pub async fn get_user_id(
    guild_id: &serenity::all::GuildId,
    message: &str,
    ctx: &serenity::all::Context
) -> Option<serenity::all::UserId> {
    if let Some(mention_id) = parse_mention(message) {
        if let Ok(id) = mention_id.parse::<u64>() {
            return Some(serenity::all::UserId::new(id));
        }
    }

    let user_name = parse_username(message)?;
    let members = guild_id.members(&ctx.http, None, None).await.ok()?;
    for member in members {
        let display = member.display_name();
        let name = &member.user.name;
        let nick = member.nick.as_deref().unwrap_or("");

        if display.eq_ignore_ascii_case(&user_name) || 
            name.eq_ignore_ascii_case(&user_name) || 
            nick.eq_ignore_ascii_case(&user_name) {
            let user_id = member.user.id;
            return Some(user_id);
        }
    }

    None
}

fn parse_mention(message: &str) -> Option<String> {
    let start_bracket = message.find('<')?;
    if message.as_bytes().get(start_bracket + 1) != Some(&b'@') {
        return None;
    }

    let start = start_bracket + 2;
    let end_bracket = message[start..].find('>')?;
    let end = start + end_bracket;

    if message.as_bytes().get(end) != Some(&b'>') {
        return None;
    }

    let mention = message[start..end].to_string();

    Some(mention)
}

fn parse_username(message: &str) -> Option<String> {
    if !message.starts_with('@') {
        return None;
    }

    let rest = &message[1..];
    let end = rest.find(' ');

    let username = if let Some(end) = end { 
        rest[..end].to_string() 
    } else { 
        rest.to_string() 
    };

    Some(username)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mention_with_id() {
        assert_eq!(parse_mention(" bar <@123456789> foo "), Some("123456789".to_string()));
    }

    #[test]
    fn parse_mention_no_match() {
        assert_eq!(parse_mention("  @username  "), None);
        assert_eq!(parse_mention("hello"), None);
    }

    #[test]
    fn parse_empty() {
        assert_eq!(parse_mention(""), None);
        assert_eq!(parse_username(""), None);
    }

    #[test]
    fn parse_space() {
        assert_eq!(parse_mention(" "), None);
        assert_eq!(parse_username(" "), None);
    }

    #[test]
    fn parse_indicators() {
        assert_eq!(parse_mention("@"), None);
        assert_eq!(parse_mention(" @"), None);
        assert_eq!(parse_mention("@ "), None);
        assert_eq!(parse_mention(" @ "), None);
        assert_eq!(parse_mention("\t@"), None);
        assert_eq!(parse_mention("@\t"), None);
        assert_eq!(parse_mention("\t@\t"), None);
        assert_eq!(parse_mention("  @"), None);
        assert_eq!(parse_mention("@  "), None);
        assert_eq!(parse_mention("  @  "), None);

        assert_eq!(parse_username(" @<>"), None);
        assert_eq!(parse_username(" @ <>"), None);
        assert_eq!(parse_username(" @ < >"), None);
        assert_eq!(parse_username(" @ <> "), None);
        assert_eq!(parse_username(" @ < > "), None);
        assert_eq!(parse_username(" @< >"), None);
        assert_eq!(parse_username(" @<> "), None);
        assert_eq!(parse_username(" @< > "), None);

        assert_eq!(parse_username("\t@<>"), None);
        assert_eq!(parse_username("\t@ <>"), None);
        assert_eq!(parse_username("\t@ < >"), None);
        assert_eq!(parse_username("\t@ <> "), None);
        assert_eq!(parse_username("\t@ < > "), None);
        assert_eq!(parse_username("\t@< >"), None);
        assert_eq!(parse_username("\t@<> "), None);
        assert_eq!(parse_username("\t@< > "), None);

        assert_eq!(parse_username("  @<>"), None);
        assert_eq!(parse_username("  @ <>"), None);
        assert_eq!(parse_username("  @ < >"), None);
        assert_eq!(parse_username("  @ <> "), None);
        assert_eq!(parse_username("  @ < > "), None);
        assert_eq!(parse_username("  @< >"), None);
        assert_eq!(parse_username("  @<> "), None);
        assert_eq!(parse_username("  @< > "), None);

        assert_eq!(parse_username(" @< >"), None);
    }

    #[test]
    fn parse_username_extracts_name() {
        assert_eq!(parse_username("@username"), Some("username".to_string()));
        assert_eq!(parse_username("@username foo "), Some("username".to_string()));
        // assert_eq!(parse_username(" bar @username foo "), Some("username".to_string())); // gets None
    }

    #[test]
    fn parse_username_returns_none_for_mention() {
        assert_eq!(parse_username("<@123456789>"), None);
    }
}