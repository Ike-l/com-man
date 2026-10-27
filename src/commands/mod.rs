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
            "
            `Note: Prefix all commands with {}`\n\
            * `ping`\n\
            * `list`\n\
            * set accept threshold `number` # Req: Admin \n\
            * set duration `number` (seconds) # Req: Admin \n\
            * echo `message`
            ",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;
    use std::time::Duration;
    use tracing_subscriber::{fmt, EnvFilter};

    static INIT: Once = Once::new();

    fn init_test_logging() {
        INIT.call_once(|| {
            let _ = fmt()
                .with_env_filter(EnvFilter::new(
                    std::env::var("TEST_LOG_LEVEL").unwrap_or_else(|_| "trace".to_string())
                ))
                .with_test_writer()
                .try_init();
        });
    }

    #[test]
    fn parse_ping() {
        init_test_logging();
        let cmd = Commands::try_from("!ping     ".to_string()).unwrap();
        matches!(cmd, Commands::Ping);
    }

    #[test]
    fn parse_list() {
        let cmd = Commands::try_from("!list        ".to_string()).unwrap();
        matches!(cmd, Commands::List);
    }

    #[test]
    fn parse_echo() {
        let cmd = Commands::try_from("!echo       hello      world       ".to_string()).unwrap();
        match cmd {
            Commands::Echo(s) => assert_eq!(s, "hello world"),
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parse_set_duration() {
        let cmd = Commands::try_from("!set      duration         30        ".to_string()).unwrap();
        match cmd {
            Commands::SetDuration(d) => assert_eq!(d, Duration::from_secs(30)),
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parse_set_accept_threshold() {
        let cmd = Commands::try_from("!set     accept      threshold      5          ".to_string()).unwrap();
        match cmd {
            Commands::SetAcceptVoteThreshold(t) => assert_eq!(t, 5),
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn parse_set_deny_threshold() {
        let cmd = Commands::try_from("!set     deny       threshold     2      ".to_string()).unwrap();
        match cmd {
            Commands::SetDenyVoteThreshold(t) => assert_eq!(t, 2),
            _ => panic!("wrong command"),
        }
    }

    #[test]
    fn missing_prefix() {
        let err = Commands::try_from("ping    ".to_string()).unwrap_err();
        matches!(err, CommandError::NoPrefix);
    }

    #[test]
    fn unknown_command() {
        let err = Commands::try_from("!unknown    ".to_string()).unwrap_err();
        matches!(err, CommandError::NotFound);
    }

    #[test]
    fn empty() {
        let err = Commands::try_from("".to_string()).unwrap_err();
        matches!(err, CommandError::NoPrefix);
    }

    #[test]
    fn spaced() {
        let err = Commands::try_from(" ".to_string()).unwrap_err();
        matches!(err, CommandError::NoPrefix);
    }
}