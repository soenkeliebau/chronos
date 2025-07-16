use crate::ChronoConfig::Error::{GetEnvVar, GetPasswordFromEntry, NoPassword};
use crate::Favorite;
use inquire::{Password, PasswordDisplayMode};
use keyring::Entry;
use keyring::Error::NoEntry;
use secrecy::SecretString;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};

static PASSWORD_ENV_VAR: &str = "CHRONOS_PASSWORD";
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
    GetPasswordFromEntry { source: keyring::Error },
    #[snafu(display("error storing password into keyring : {source}"))]
    SetPasswordFromEntry { source: keyring::Error },
    #[snafu(display("No password provided, please either set one via env var, the config, or use `login` to store one in the systems keyring."))]
    NoPassword { },
    

}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub password: Option<SecretString>,
    pub user_name: String,
    pub favorites: Option<Vec<Favorite>>,
}

impl Config {
    pub fn new() -> Result<Self, Error> {
        let config_location = std::env::var(CONFIG_FILE_LOCATION_OVERRIDE)
            .unwrap_or(CONFIG_FILE_LOCATION_DEFAULT.to_string());

        let mut config_from_file: Config = serde_json::from_str(
            &std::fs::read_to_string(&config_location).context(ReadConfigFileSnafu {
                path: &config_location,
            })?,
        )
        .context(ParseConfigFileSnafu)?;

        // Priority for password config is:
        //   1. env var
        //   2. config file
        //   3. keyring
        // So we check if the env var is set, if yes we overwrite whatever was loaded from the file,
        // if no env var is present we check if the password from the file is Some(..) - if yes we
        // use that, if not we fall back to keyring
        if let Some(password_from_env) = std::env::var(PASSWORD_ENV_VAR).ok() {
            config_from_file.password = Some(SecretString::from(password_from_env));
        } else {
            // No password from env var, so check if we got one from the file
            if config_from_file.password.is_none() {
                // no password in the file, read from keyring
                let password =
                    match Entry::new("tech.stackable.chronos", &config_from_file.user_name) {
                        Ok(password_entry) => {
                            if let Ok(password) =
                                password_entry.get_password().context(GetPasswordSnafu {
                                    user: &config_from_file.user_name,
                                })
                            {
                                config_from_file.password = Some(SecretString::from(password));
                            } else {
                                return Err(NoPassword {});
                                /*
                                println!("No stored password found.");
                                let new_password = Password::new(&format!(
                                    "Enter password for user {}: ",
                                    &config_from_file.user_name
                                ))
                                .with_display_toggle_enabled()
                                .without_confirmation()
                                .with_display_mode(PasswordDisplayMode::Hidden)
                                .with_formatter(&|_| String::from("Input received"))
                                .prompt()
                                .context(ObtainPasswordFromUserSnafu {})?;
                                password_entry
                                    .set_password(&new_password)
                                    .context(SetPasswordFromEntrySnafu {})?;
                                */
                            };
                        }

                        Err(e) => match e {
                            NoEntry => {
                                println!("No password entry found!")
                            }
                            _ => {
                                println!("Error obtaining password!")
                            }
                        },
                    };
            }
        }
        Ok(config_from_file)
    }
    
    pub fn save_password(&mut self, password: &str) -> Result<(), Error> {
        match Entry::new("tech.stackable.chronos", &self.user_name) {
            Ok(entry) => {
                entry.set_password(password).context(SetPasswordFromEntrySnafu)?;
                self.password = Some(SecretString::from(password));
            }
            Err(_) => {}
        }
        Ok(())
    }
}
