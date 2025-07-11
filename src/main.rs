mod ChronoConfig;

use clap::Parser;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};
use ChronoConfig::Config;

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("Error reading configuration: {source}"))]
    Config {
        source: crate::ChronoConfig::Error,
    },
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,

    /// Number of times to greet
    #[arg(short, long, default_value_t = 1)]
    count: u8,
}

#[derive(Parser, Debug)]
enum Command {
    Login,
    Run,
    Book,
}

#[derive(Parser, Debug, Deserialize)]
pub struct Favorite{
    project: usize,
    task: usize,
    duration: Option<usize>,
    comment: Option<String>,
}

fn main() -> Result<(), Error>{
    let args = Args::parse();

    let config = Config::new().context(ConfigSnafu {})?;
    
    match args.command {
        Command::Login => {
            println!("Logging in for user {}", config.user_name);
        }
        Command::Run => {}
        Command::Book => {}
    }
    Ok(())
}