use std::{fmt::Display, time::Duration};

use thiserror::Error;

pub enum Commands {
    Ping,
    List,
    Echo(String),
    SetDuration(Duration),
    SetVoteThreshold(u64),
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let command = match self {
            Commands::Ping => format!("ping"),
            Commands::List => format!("list"),
            Commands::Echo(message) => format!("echo {message}"),
            Commands::SetDuration(duration) => format!("set duration {}s", duration.as_secs()),
            Commands::SetVoteThreshold(threshold) => format!("set threshold {threshold}"),
        };

        write!(f, "{command}")
    }
}

impl Commands {
    pub const PREFIX: &'static str = "!";

    pub fn list() -> String {
        format!(
            "`note: Prefix all commands with {}`\n\
            * ping\n\
            * list\n\
            * set threshold `number` # Req: Admin \n\
            * set duration `number` (seconds) # Req: Admin \n\
            * echo `message`",
            Self::PREFIX
        )
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("No Prefix Found. Expected: {}", Commands::PREFIX)]
    NoPrefix,
    #[error("Command Not Found. Expected One of: {}", Commands::list())]
    NotFound,
    #[error("Failed to parse Threshold value. Expected valid u64")]
    ParsingThreshold,
    #[error("Failed to parse Duration value. Expected valid u64 in seconds")]
    ParsingDuration,
}

impl TryFrom<String> for Commands {
    type Error = CommandError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let command = value.strip_prefix(Self::PREFIX).ok_or(CommandError::NoPrefix)?;

        let words = command.split(' ').collect::<Vec<_>>();
        match words.as_slice() {
            ["list"] => Ok(Self::List),
            ["ping"] => Ok(Self::Ping),
            ["set", "duration", duration] => Ok(Self::SetDuration(Duration::from_secs(duration.parse().map_err(|_| CommandError::ParsingDuration)?))),
            ["set", "threshold", threshold] => Ok(Self::SetVoteThreshold(threshold.parse().map_err(|_| CommandError::ParsingThreshold)?)),
            ["echo", message @ ..] => {
                let message = message.join(" ");
                Ok(Self::Echo(message))
            },
            _ => Err(CommandError::NotFound)
        }
    }
}