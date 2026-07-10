use anyhow::{Context, Result, bail};
use dialoguer::Confirm;

pub fn shell_join(words: &[String]) -> String {
    words
        .iter()
        .map(|word| shell_quote(word))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "_./:@=-".contains(ch))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

pub fn guard_dangerous_command(command: &str, yes: bool) -> Result<()> {
    if yes || !is_dangerous(command) {
        return Ok(());
    }
    let proceed = Confirm::new()
        .with_prompt(format!(
            "About to run dangerous command `{command}`. Continue?"
        ))
        .default(false)
        .interact()
        .context("failed to read confirmation")?;
    if !proceed {
        bail!("operation cancelled");
    }
    Ok(())
}

fn is_dangerous(command: &str) -> bool {
    let normalized = command.trim();
    let prefixes = [
        "rm ",
        "share set-public ",
        "share add-link ",
        "share add-share-link ",
        "grant-support-root ",
        "ssh-key remove ",
        "integrations remove ",
        "integrations setup ",
        "integrations detach ",
        "integrations edit ",
        "team remove ",
        "team role ",
        "team transfer ",
        "team disable",
        "domain rm ",
        "billing capacity",
    ];
    prefixes
        .iter()
        .any(|prefix| normalized == prefix.trim_end() || normalized.starts_with(prefix))
        || normalized.starts_with("tag -d ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_keeps_safe_words_unquoted() {
        assert_eq!(shell_quote("vm-1"), "vm-1");
        assert_eq!(shell_quote("user@example.com"), "user@example.com");
        assert_eq!(shell_quote("--name=a"), "--name=a");
    }

    #[test]
    fn quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn detects_dangerous_commands() {
        assert!(is_dangerous("rm vm1"));
        assert!(is_dangerous("share set-public vm1"));
        assert!(is_dangerous("share add-link vm1"));
        assert!(is_dangerous("ssh-key remove abc"));
        assert!(is_dangerous("team disable --yes"));
        assert!(is_dangerous("team transfer vm1 a@b.c"));
        assert!(is_dangerous("domain rm vm1 app.example.com"));
        assert!(is_dangerous("integrations edit myproxy --target x"));
        assert!(is_dangerous("billing capacity"));
        assert!(!is_dangerous("ls"));
        assert!(!is_dangerous("team members"));
        assert!(!is_dangerous("domain ls -a"));
    }
}
