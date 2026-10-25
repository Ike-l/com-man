use std::{fmt::Display, time::Duration};

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
            * set threshold `number` # Req: Admin,
            * set duration `number` (seconds) # Req: Admin,
            * echo `message`",
            Self::PREFIX
        )
    }
}

pub enum CommandError {
    NoPrefix,
    NotFound,
    ParsingThreshold,
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