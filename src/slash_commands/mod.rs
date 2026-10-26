use crate::get_user_id;

pub enum SlashCommand {
    Kick(serenity::all::UserId),
    Ban(serenity::all::UserId),
    BanWithReason((serenity::all::UserId, String)),
    Disconnect(serenity::all::UserId),
    Timeout((serenity::all::UserId, serenity::all::Timestamp)),
    DM((serenity::all::UserId, String))
}

#[derive(Debug)]
pub enum SlashCommandError {
    NotFound,
    NoPrefix,
    UserId,
    ParsingDuration,
}

impl SlashCommand {
    pub const PREFIX: &'static str = "/";

    pub async fn try_from(
        value: &str,
        guild_id: &serenity::all::GuildId,
        ctx: &serenity::all::Context
    ) -> Result<Self, SlashCommandError> {    
        let command = value.strip_prefix(Self::PREFIX).ok_or(SlashCommandError::NoPrefix)?;
    
        let words = command.split_ascii_whitespace().collect::<Vec<_>>();
        match words.as_slice() {
            ["dm", username, message @ ..] => {
                let message = message.join(" ");
                let user_id = get_user_id(guild_id, *username, ctx).await.ok_or(SlashCommandError::UserId)?;
                Ok(Self::DM((user_id, message.to_string())))
            },
            ["kick", username] => {
                let user_id = get_user_id(guild_id, *username, ctx).await.ok_or(SlashCommandError::UserId)?;
                Ok(Self::Kick(user_id))
            },
            ["ban", username] => {
                let user_id = get_user_id(guild_id, *username, ctx).await.ok_or(SlashCommandError::UserId)?;
                Ok(Self::Ban(user_id))
            },
            ["ban", username, reason] => {
                let user_id = get_user_id(guild_id, *username, ctx).await.ok_or(SlashCommandError::UserId)?;
                Ok(Self::BanWithReason((user_id, reason.to_string())))
            },
            ["disconnect", username] => {
                let user_id = get_user_id(guild_id, *username, ctx).await.ok_or(SlashCommandError::UserId)?;
                Ok(Self::Disconnect(user_id))
            },
            ["timeout", username, duration] => {
                // parse last character as 's' 'm' 'h' 'd'
                let duration = duration.parse::<u64>().map_err(|_| SlashCommandError::ParsingDuration)?;
                let user_id = get_user_id(guild_id, *username, ctx).await.ok_or(SlashCommandError::UserId)?;
                let now = serenity::all::Timestamp::now();
                let duration = now.checked_add_signed(chrono::TimeDelta::seconds(duration.try_into().unwrap())).unwrap();
                Ok(Self::Timeout((user_id, duration.into())))
            },
            _ => Err(SlashCommandError::NotFound)
        }
    }
}

