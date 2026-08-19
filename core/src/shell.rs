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
    let commands = [
        "rm",
        "share set-public",
        "share set-private",
        "share add-link",
        "share add-share-link",
        "share remove-link",
        "share remove-share-link",
        "share remove",
        "share access allow",
        // Turns a VM's mailbox on and sets who it may write to.
        "share receive-email",
        "grant-support-root",
        // Both mint a credential that reaches VMs, so they belong with the
        // revocation the list already covers.
        "ssh-key add",
        "ssh-key generate-api-key",
        "ssh-key remove",
        "domain add",
        "domain rm",
        // `add` can carry --attach specs and `attach` mounts the credential into
        // VMs, so both hand out access just as `detach` and `edit` take it away.
        "integrations add",
        "integrations attach",
        "integrations remove",
        "integrations setup",
        "integrations detach",
        "integrations edit",
        // Everything that changes who holds authority over the team or its VMs.
        "team add",
        "team remove",
        "team role",
        "team transfer",
        "team auth set",
        "team settings vm-sharing",
        "team disable",
        "team settings auto-join on",
        // Anything that changes what the account is billed for. `resize` and `cp`
        // take effect immediately, and creating a pool reserves capacity, so they
        // belong with the subscription commands below rather than outside the
        // "spending" category the skill documents.
        "resize",
        "cp",
        "pool new",
        "pool delete",
        "billing capacity",
        "billing credits buy",
        "billing payment remove",
        "billing payment default",
        "tag -d",
    ];
    if is_read_only_integrations_setup(normalized) {
        return false;
    }
    commands
        .iter()
        .any(|name| matches_command(normalized, name))
        || grants_shell_access(normalized)
}

/// Matches a command name at a word boundary.
///
/// A plain `starts_with` would also match anything that merely spells one of
/// these names as a prefix, so a raw `exec -- team disablex` or
/// `billing capacityfoo` would prompt for a command that is not the dangerous one.
fn matches_command(command: &str, name: &str) -> bool {
    command == name
        || command
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with(' '))
}

/// `integrations setup <type> --list` and `--verify` only report what is already
/// connected, so they are exempt unless a disconnect flag is present too.
fn is_read_only_integrations_setup(command: &str) -> bool {
    if !matches_command(command, "integrations setup") {
        return false;
    }
    let mut reads = false;
    let mut mutates = false;
    for word in command.split_whitespace() {
        match word {
            "--list" | "--verify" => reads = true,
            "-d" | "--delete" => mutates = true,
            _ => {}
        }
    }
    reads && !mutates
}

/// `share add <vm> <target> --root` grants SSH, Terminal, and Shelley access,
/// which is strictly more powerful than the web-only share it looks like.
fn grants_shell_access(command: &str) -> bool {
    command.starts_with("share add ") && command.split_whitespace().any(|word| word == "--root")
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
        assert!(is_dangerous("billing credits buy 100 --yes"));
        assert!(is_dangerous("billing payment remove 4f1c2a9b8d3e"));
        assert!(is_dangerous("pool delete builders --force"));
        assert!(is_dangerous("share access allow mybox"));
        assert!(is_dangerous("team settings auto-join on"));
        assert!(is_dangerous("share add mybox a@b.c --root"));
        assert!(is_dangerous("integrations add github --name repo"));
        assert!(is_dangerous("integrations attach my-mcp auto:all"));
        assert!(is_dangerous("team add a@b.c admin"));
        assert!(is_dangerous("team auth set oidc --issuer-url https://x"));
        assert!(is_dangerous("team settings vm-sharing all-members"));
        assert!(is_dangerous("ssh-key add --tag prod 'ssh-ed25519 AAAA k'"));
        assert!(is_dangerous("ssh-key generate-api-key --exp 30d"));
        assert!(is_dangerous("share remove mybox a@b.c"));
        assert!(is_dangerous("share remove-link mybox tok"));
        assert!(is_dangerous("share set-private mybox"));
        assert!(is_dangerous("domain add mybox app.example.com"));
        assert!(is_dangerous("pool new builders --cpus 16 --region fra"));
        assert!(is_dangerous("resize mybox --cpu 64"));
        assert!(is_dangerous("cp mybox mybox-2"));
        assert!(is_dangerous("billing payment default 4f1c2a9b"));
        assert!(is_dangerous("share receive-email mybox on"));
        assert!(!is_dangerous("ls"));
        assert!(!is_dangerous("pool list"));
        assert!(!is_dangerous("billing payment list"));
    }

    #[test]
    fn danger_matching_stops_at_word_boundaries() {
        assert!(!is_dangerous("team disablex"));
        assert!(!is_dangerous("billing capacityfoo"));
        assert!(!is_dangerous("ssh-key generate-api-keyx"));
        assert!(!is_dangerous("team settings auto-join oncall"));
        assert!(!is_dangerous("rmx vm1"));
        assert!(is_dangerous("team disable --yes"));
        assert!(is_dangerous("billing capacity"));
    }

    #[test]
    fn read_only_integrations_setup_is_exempt() {
        assert!(!is_dangerous("integrations setup github --list"));
        assert!(!is_dangerous("integrations setup chatgpt --verify"));
        assert!(is_dangerous("integrations setup github"));
        assert!(is_dangerous("integrations setup github --list -d"));
        assert!(is_dangerous("integrations setup github --delete --list"));
        assert!(!is_dangerous("ssh-key list"));
        assert!(!is_dangerous("share show mybox"));
        assert!(!is_dangerous("domain ls mybox"));
        assert!(!is_dangerous("team members"));
        assert!(!is_dangerous("team settings"));
        assert!(!is_dangerous("integrations list --usage"));
        assert!(!is_dangerous("integrations catalog stripe"));
        assert!(!is_dangerous("domain ls -a"));
        assert!(!is_dangerous("share add mybox a@b.c"));
        // Revocation is covered as a deletion, so the --root downgrade is too.
        assert!(is_dangerous("share remove mybox a@b.c --root"));
        assert!(!is_dangerous("team settings auto-join off"));
        assert!(!is_dangerous("billing credits usage --group=day"));
        assert!(!is_dangerous("pool list"));
    }
}
