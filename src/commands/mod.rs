pub enum Commands {
    Ping,
}

pub enum CommandError {
    NoPrefix,
    NotFound
}

impl TryFrom<String> for Commands {
    type Error = CommandError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let command = value.strip_prefix("!").ok_or(CommandError::NoPrefix)?;

        match command {
            "ping" => Ok(Self::Ping),
            _ => Err(CommandError::NotFound)
        }
    }
}