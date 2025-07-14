mod ChronoConfig;

use ChronoConfig::Config;
use chrono::NaiveDate;
use clap::Parser;
use inquire::{Password, PasswordDisplayMode};
use secrecy::ExposeSecret;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use std::time::Duration;

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("Error reading configuration: {source}"))]
    Config { source: crate::ChronoConfig::Error },
    #[snafu(display("failed to read password input from user : {source}"))]
    ObtainPasswordFromUser { source: inquire::InquireError },
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
        #[arg(short, long)]
        template: Option<String>,
        #[arg(short, long)]
        project: Option<String>,
        #[arg(short, long)]
        task: Option<String>,
        #[arg(short, long)]
        duration: Option<Duration>,
        #[arg(short, long)]
        reference: Option<String>,
        #[arg(short, long)]
        comment: Option<String>,
        #[arg(short, long)]
        date: Option<NaiveDate>,
    },
    Sync,
}

#[derive(Parser, Debug, Deserialize)]
pub struct Favorite {
    project: usize,
    task: usize,
    duration: Option<usize>,
    comment: Option<String>,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let config = Config::new().context(ConfigSnafu {})?;

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
            config.save_password(&new_password)?;
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
        } => {}
        Command::Sync => {}
    }
    Ok(())
}
