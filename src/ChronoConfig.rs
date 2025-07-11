use crate::ChronoConfig::Error::{GetEnvVar, GetPasswordFromEntry};
use crate::Favorite;
use keyring::Entry;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};

static CONFIG_FILE_LOCATION_OVERRIDE: &str = "CHRONOS_CONFIG_FILE";
static CONFIG_FILE_LOCATION_DEFAULT: &str = "~/.chronos/config.json";

#[derive(Snafu, Debug)]
pub enum Error {
    #[snafu(display("Could not read config file {path}"))]
    ReadConfigFile {
        source: std::io::Error,
        path: String,
    },
    #[snafu(display("error parsing config file: {source}"))]
    ParseConfigFile { source: serde_json::Error },
    #[snafu(display("error reading config file location env var: {source}"))]
    GetEnvVar { source: std::env::VarError },
    #[snafu(display("error getting password for user {user} from keyring: {source}"))]
    GetPassword {
        source: keyring::Error,
        user: String,
    },
    #[snafu(display("error getting password from keyring response: {source}"))]
    GetPasswordFromEntry {
        source: keyring::Error,
    },
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub password: String,
    pub user_name: String,
    pub favorites: Option<Vec<Favorite>>,
}

#[derive(Deserialize, Debug)]
struct InternalConfig {
    pub user_name: String,
    pub favorites: Option<Vec<Favorite>>,
}

impl Config {
    pub fn new() -> Result<Self, Error> {
        let config_location = std::env::var(CONFIG_FILE_LOCATION_OVERRIDE)
            .unwrap_or(CONFIG_FILE_LOCATION_DEFAULT.to_string());

        let config_from_file: InternalConfig = serde_json::from_str(
            &std::fs::read_to_string(&config_location).context(ReadConfigFileSnafu {
                path: &config_location,
            })?,
        )
        .context(ParseConfigFileSnafu)?;

        let test = Entry::new("tech.stackable.chronos", &config_from_file.user_name).context(
            GetPasswordSnafu {
                user: &config_from_file.user_name,
            },
        )?;
        Ok(Self {
            password: test.get_password().context(GetPasswordFromEntrySnafu {})?,
            user_name: config_from_file.user_name,
            favorites: config_from_file.favorites,
        })
    }
}
