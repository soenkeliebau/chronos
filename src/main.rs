mod ChronoConfig;

use ChronoConfig::Config;
use chrono::{Duration, NaiveDate, Weekday};
use clap::Parser;
use clap::parser::ValueSource;
use inquire::validator::{ErrorMessage, StringValidator, Validation};
use inquire::{
    CustomType, CustomUserError, DateSelect, Editor, Password, PasswordDisplayMode, Select, Text,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use stackable_serious_business::coffeecup::client::CoffeeCup;
use stackable_serious_business::coffeecup::structs::Project;
use stackable_serious_business::easybill::structs::Customer;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io;
use std::io::{BufReader, BufWriter, Write};
use std::num::ParseIntError;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Snafu, Debug)]
pub enum ChronosError {
    #[snafu(display("Error reading configuration: {source}"))]
    Config { source: crate::ChronoConfig::Error },
    #[snafu(display("failed to read password input from user : {source}"))]
    ObtainPasswordFromUser { source: inquire::InquireError },
    #[snafu(display("Error in communication with CoffeeCup [{action}]: {source}"))]
    CoffeeCup {
        source: stackable_serious_business::coffeecup::client::Error,
        action: String,
    },
    #[snafu(display("failed to open projects state file [{filename}] for writing: {source}"))]
    OpenProjectsFile { source: io::Error, filename: String },
    #[snafu(display("Failed to write projects to file {filename}: {source}"))]
    WriteProjectsFile {
        source: serde_json::Error,
        filename: String,
    },
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
enum Command {
    Login,
    Run,
    Book {
        #[arg(long)]
        template: Option<String>,
        #[arg(short, long)]
        project: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        duration: Option<usize>,
        #[arg(short, long)]
        reference: Option<String>,
        #[arg(short, long)]
        comment: Option<String>,
        #[arg(long)]
        date: Option<NaiveDate>,
    },
    Sync,
    Template,
}

#[derive(Parser, Debug, Deserialize)]
pub struct Favorite {
    project: usize,
    task: usize,
    duration: Option<usize>,
    comment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectTask {
    pub display: String,
    pub project: usize,
    pub task: usize,
}

impl Display for ProjectTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct TimeEntryDraft {
    pub date: Option<String>,
    pub description: Option<String>,
    pub duration: Option<usize>,
    pub target: Option<BookingTarget>,
    pub reference: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
struct BookingTarget {
    pub task: usize,
    pub project: usize,
}

#[tokio::main]
async fn main() -> Result<(), ChronosError> {
    let args = Args::parse();

    let mut config = Config::new().context(ConfigSnafu {})?;

    match args.command {
        Command::Login => {
            let new_password =
                Password::new(&format!("Enter password for user {}: ", &config.user_name))
                    .with_display_toggle_enabled()
                    .without_confirmation()
                    .with_display_mode(PasswordDisplayMode::Hidden)
                    .with_formatter(&|_| String::from("Input received"))
                    .prompt()
                    .context(ObtainPasswordFromUserSnafu {})?;
            config
                .save_password(&new_password)
                .context(ConfigSnafu {})?;
            println!("{}", config.password.unwrap().expose_secret());
        }
        Command::Run => {}
        Command::Book {
            template,
            project,
            task,
            duration,
            reference,
            comment,
            date,
        } => {
            // If a template is specified initialize the draft from this template first
            // otherwise start with an empty draft and ask for _everything_
            let mut draft = match template {
                None => TimeEntryDraft::default(),
                Some(template_name) => {
                    let templates = load_templates()?;
                    match templates.get(&template_name) {
                        None => TimeEntryDraft::default(),
                        Some(entry) => entry.clone(),
                    }
                }
            };

            let file = File::open("./config/projects.json").context(OpenProjectsFileSnafu {
                filename: "./config/projects.json",
            })?;
            let reader = BufReader::new(file);
            let project_tasks: Vec<ProjectTask> =
                serde_json::from_reader(reader).context(WriteProjectsFileSnafu {
                    filename: "./config/projects.json",
                })?;

            if draft.target.is_none() {
                let project_task = Select::new("Welches Projekt?", project_tasks)
                    .prompt()
                    .unwrap();
                draft.target = Some(BookingTarget {
                    task: project_task.task,
                    project: project_task.task,
                });
            }

            if draft.date.is_none() {
                let booking_date = DateSelect::new("Date?")
                    .with_week_start(Weekday::Mon)
                    .prompt();
            }

            if draft.duration.is_none() {
                let duration = CustomType::<usize>::new("How long did you work?")
                    .prompt()
                    .unwrap();

                draft.duration = Some(duration);
            }

            if draft.description.is_none() {
                let description = Editor::new("What did you do?").prompt().unwrap();
                draft.description = Some(description);
            }
            println!("Booking this: {:?}", draft);
        }
        Command::Sync => {
            let mut coffeecup_client = CoffeeCup::new_with_password(
                &config.user_name,
                config.password.as_ref().unwrap().expose_secret(),
            )
            .await
            .context(CoffeeCupSnafu {
                action: "create client".to_string(),
            })?;
            let projects = coffeecup_client
                .get_my_projects()
                .await
                .context(CoffeeCupSnafu {
                    action: "list projects",
                })?;

            let customers = coffeecup_client
                .list_customer()
                .await
                .context(CoffeeCupSnafu {
                    action: "list customers",
                })?
                .into_iter()
                .map(|customer| (customer.id, customer))
                .collect::<BTreeMap<_, _>>();

            let mut project_tasks: Vec<ProjectTask> = Vec::new();

            for project in projects {
                let client = match project.client {
                    None => "Internal",
                    Some(client_id) => match customers.get(&client_id) {
                        None => "Missing Customer",
                        Some(customer) => &customer.name,
                    },
                };

                if let Some(tasks) = project.tasks {
                    for task in tasks {
                        project_tasks.push(ProjectTask {
                            display: format!("{} / {} / {}", client, project.name, task.label),
                            project: project.id,
                            task: task.id,
                        })
                    }
                }
            }

            let file = File::create("./config/projects.json").context(OpenProjectsFileSnafu {
                filename: "./config/projects.json",
            })?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer(&mut writer, &project_tasks).context(WriteProjectsFileSnafu {
                filename: "./config/projects.json",
            })?;
            writer.flush().context(OpenProjectsFileSnafu {
                filename: "./config/projects.json",
            })?;
        }
        Command::Template => {}
    }
    Ok(())
}

#[derive(Default, Clone, Debug)]
struct DurationValidator {}

impl StringValidator for DurationValidator {
    fn validate(&self, input: &str) -> Result<Validation, CustomUserError> {
        match usize::from_str(input) {
            Ok(_) => Ok(Validation::Valid),
            Err(e) => Ok(Validation::Invalid(ErrorMessage::Custom(
                "Not a number!".to_string(),
            ))),
        }
    }
}

fn load_templates() -> Result<BTreeMap<String, TimeEntryDraft>, ChronosError> {
    let file = File::open("./config/templates.json").context(OpenProjectsFileSnafu {
        filename: "./config/templates.json",
    })?;

    let reader = BufReader::new(file);
    serde_json::from_reader(reader).context(WriteProjectsFileSnafu {
        filename: "./config/templates.json",
    })
}
