use std::{fmt::Display, time::Duration};

use thiserror::Error;

#[derive(Debug)]
pub enum Commands {
    Ping,
    List,
    Echo(String),
    SetDuration(Duration),
    SetAcceptVoteThreshold(u64),
    SetDenyVoteThreshold(u64),
}

impl Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let command = match self {
            Commands::Ping => format!("ping"),
            Commands::List => format!("list"),
            Commands::Echo(message) => format!("echo {message}"),
            Commands::SetDuration(duration) => format!("set duration {}s", duration.as_secs()),
            Commands::SetAcceptVoteThreshold(threshold) => format!("set accept threshold {threshold}"),
            Commands::SetDenyVoteThreshold(threshold) => format!("set deny threshold {threshold}"),
        };

        write!(f, "{command}")
    }
}

impl Commands {
    pub const PREFIX: &'static str = "!";

    pub fn list() -> String {
        format!(
            "`Note: Prefix all commands with {}`\n\
            * `ping`\n\
            * `list`\n\
            * set accept threshold `number` # Req: Admin \n\
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
    #[error("Command Not Found. Expected one of: \n{}", Commands::list())]
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

        let words = command.split_whitespace().collect::<Vec<_>>();
        match words.as_slice() {
            ["list"] => Ok(Self::List),
            ["ping"] => Ok(Self::Ping),
            ["set", "duration", duration] => Ok(Self::SetDuration(Duration::from_secs(duration.parse().map_err(|_| CommandError::ParsingDuration)?))),
            ["set", "accept", "threshold", threshold] => Ok(Self::SetAcceptVoteThreshold(threshold.parse().map_err(|_| CommandError::ParsingThreshold)?)),
            ["set", "deny", "threshold", threshold] => Ok(Self::SetDenyVoteThreshold(threshold.parse().map_err(|_| CommandError::ParsingThreshold)?)),
            ["echo", message @ ..] => {
                let message = message.join(" ");
                Ok(Self::Echo(message))
            },
            _ => Err(CommandError::NotFound)
        }
    }
}