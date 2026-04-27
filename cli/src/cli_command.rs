use crate::cli::*;
use anyhow::{Result, bail};

#[derive(Debug)]
pub(crate) struct BuiltCommand {
    pub(crate) words: Vec<String>,
    pub(crate) fallback_ssh: bool,
}

pub(crate) fn build_command(command: &Commands) -> Result<BuiltCommand> {
    let mut words = Vec::new();
    let mut fallback_ssh = false;

    match command {
        Commands::Help(cmd) => {
            words.push("help".into());
            words.extend(cmd.command.clone());
        }
        Commands::Doc(cmd) => {
            words.push("doc".into());
            push_opt(&mut words, cmd.slug.as_ref());
        }
        Commands::Ls(cmd) => {
            words.push("ls".into());
            if cmd.all {
                words.push("-a".into());
            }
            if cmd.long {
                words.push("-l".into());
            }
            push_opt(&mut words, cmd.pattern.as_ref());
        }
        Commands::New(cmd) => {
            words.push("new".into());
            push_flag_value(&mut words, "--command", cmd.command.as_ref());
            push_flag_value(&mut words, "--disk", cmd.disk.as_ref());
            for env in &cmd.envs {
                push_flag_value(&mut words, "--env", Some(env));
            }
            push_flag_value(&mut words, "--image", cmd.image.as_ref());
            for integration in &cmd.integration {
                push_flag_value(&mut words, "--integration", Some(integration));
            }
            push_flag_value(&mut words, "--name", cmd.name.as_ref());
            if cmd.no_email {
                words.push("--no-email".into());
            }
            if cmd.prompt.as_deref() == Some("/dev/stdin")
                || cmd.setup_script.as_deref() == Some("/dev/stdin")
            {
                fallback_ssh = true;
            }
            push_flag_value(&mut words, "--prompt", cmd.prompt.as_ref());
            push_flag_value(&mut words, "--setup-script", cmd.setup_script.as_ref());
        }
        Commands::Rm(cmd) => {
            ensure_non_empty(&cmd.vmnames, "rm requires at least one VM name")?;
            words.push("rm".into());
            words.extend(cmd.vmnames.clone());
        }
        Commands::Restart(cmd) => words.extend(["restart".into(), cmd.vmname.clone()]),
        Commands::Rename(cmd) => {
            words.extend(["rename".into(), cmd.oldname.clone(), cmd.newname.clone()])
        }
        Commands::Tag(cmd) => {
            words.push("tag".into());
            if cmd.delete {
                words.push("-d".into());
            }
            words.extend([cmd.vm.clone(), cmd.tag_name.clone()]);
        }
        Commands::Stat(cmd) => {
            words.extend(["stat".into(), cmd.vm_name.clone()]);
            push_flag_value(&mut words, "--range", cmd.range.as_ref());
        }
        Commands::Cp(cmd) => {
            words.extend(["cp".into(), cmd.source_vm.clone()]);
            push_opt(&mut words, cmd.new_name.as_ref());
            push_flag_value(&mut words, "--copy-tags", cmd.copy_tags.as_ref());
            push_flag_value(&mut words, "--disk", cmd.disk.as_ref());
        }
        Commands::Resize(cmd) => words.extend([
            "resize".into(),
            cmd.vmname.clone(),
            format!("--disk={}", cmd.disk),
        ]),
        Commands::Share(cmd) => build_share_command(&mut words, &cmd.command),
        Commands::Team(cmd) => build_team_command(&mut words, &cmd.command),
        Commands::Whoami => words.push("whoami".into()),
        Commands::SshKey(cmd) => build_ssh_key_command(&mut words, &cmd.command),
        Commands::SetRegion(cmd) => {
            words.extend(["set-region".into(), cmd.region_code.clone()]);
        }
        Commands::Integrations(cmd) => build_integrations_command(&mut words, &cmd.command),
        Commands::Billing(cmd) => build_billing_command(&mut words, &cmd.command),
        Commands::Shelley(cmd) => build_shelley_command(&mut words, &cmd.command),
        Commands::Browser(cmd) => {
            words.push("browser".into());
            if cmd.qr {
                words.push("--qr".into());
            }
        }
        Commands::Ssh(cmd) => {
            fallback_ssh = true;
            words.push("ssh".into());
            if let Some(user) = &cmd.user {
                words.extend(["-l".into(), user.clone()]);
            }
            words.push(cmd.target.clone());
            words.extend(cmd.command.clone());
        }
        Commands::GrantSupportRoot(cmd) => {
            words.extend([
                "grant-support-root".into(),
                cmd.vmname.clone(),
                cmd.state.clone(),
            ]);
        }
        Commands::Exit => words.push("exit".into()),
        Commands::Exec(cmd) => words.extend(cmd.command.clone()),
    }

    Ok(BuiltCommand {
        words,
        fallback_ssh,
    })
}

fn build_share_command(words: &mut Vec<String>, command: &ShareSubcommand) {
    words.push("share".into());
    match command {
        ShareSubcommand::Show(cmd) => {
            words.extend(["show".into(), cmd.vm.clone()]);
            if cmd.qr {
                words.push("--qr".into());
            }
        }
        ShareSubcommand::Port(cmd) => {
            words.extend(["port".into(), cmd.vm.clone()]);
            push_opt(words, cmd.port.as_ref());
        }
        ShareSubcommand::SetPublic(cmd) => words.extend(["set-public".into(), cmd.vm.clone()]),
        ShareSubcommand::SetPrivate(cmd) => words.extend(["set-private".into(), cmd.vm.clone()]),
        ShareSubcommand::Add(cmd) => {
            words.extend(["add".into(), cmd.vm.clone(), cmd.target.clone()]);
            push_flag_value(words, "--message", cmd.message.as_ref());
            if cmd.qr {
                words.push("--qr".into());
            }
        }
        ShareSubcommand::Remove(cmd) => {
            words.extend(["remove".into(), cmd.vm.clone(), cmd.target.clone()]);
        }
        ShareSubcommand::AddLink(cmd) => {
            words.extend(["add-link".into(), cmd.vm.clone()]);
            if cmd.qr {
                words.push("--qr".into());
            }
        }
        ShareSubcommand::RemoveLink(cmd) => {
            words.extend(["remove-link".into(), cmd.vm.clone(), cmd.token.clone()]);
        }
        ShareSubcommand::ReceiveEmail(cmd) => {
            words.extend(["receive-email".into(), cmd.vm.clone()]);
            push_opt(words, cmd.state.as_ref());
        }
        ShareSubcommand::Access(cmd) => {
            words.extend(["access".into(), cmd.action.clone(), cmd.vm.clone()]);
        }
    }
}

fn build_team_command(words: &mut Vec<String>, command: &TeamSubcommand) {
    words.push("team".into());
    match command {
        TeamSubcommand::Members => words.push("members".into()),
        TeamSubcommand::Add(cmd) => words.extend(["add".into(), cmd.email.clone()]),
        TeamSubcommand::Remove(cmd) => words.extend(["remove".into(), cmd.email.clone()]),
    }
}

fn build_ssh_key_command(words: &mut Vec<String>, command: &SshKeySubcommand) {
    words.push("ssh-key".into());
    match command {
        SshKeySubcommand::List => words.push("list".into()),
        SshKeySubcommand::Add(cmd) => words.extend(["add".into(), cmd.public_key.clone()]),
        SshKeySubcommand::Remove(cmd) => words.extend(["remove".into(), cmd.key.clone()]),
        SshKeySubcommand::Rename(cmd) => {
            words.extend(["rename".into(), cmd.old_name.clone(), cmd.new_name.clone()]);
        }
        SshKeySubcommand::GenerateApiKey(cmd) => {
            words.push("generate-api-key".into());
            push_flag_value(words, "--label", cmd.label.as_ref());
            push_flag_value(words, "--vm", cmd.vm.as_ref());
            push_flag_value(words, "--cmds", cmd.cmds.as_ref());
            push_flag_value(words, "--exp", cmd.exp.as_ref());
        }
    }
}

fn build_integrations_command(words: &mut Vec<String>, command: &IntegrationsSubcommand) {
    words.push("integrations".into());
    match command {
        IntegrationsSubcommand::List => words.push("list".into()),
        IntegrationsSubcommand::Setup(cmd) => {
            words.extend(["setup".into(), cmd.integration_type.clone()]);
            if cmd.disconnect {
                words.push("-d".into());
            }
            if cmd.delete {
                words.push("--delete".into());
            }
            if cmd.list {
                words.push("--list".into());
            }
            if cmd.verify {
                words.push("--verify".into());
            }
        }
        IntegrationsSubcommand::Add(cmd) => {
            words.extend(["add".into(), cmd.integration_type.clone()]);
            push_flag_value(words, "--name", Some(&cmd.name));
            if cmd.team {
                words.push("--team".into());
            }
            for attach in &cmd.attach {
                push_flag_value(words, "--attach", Some(attach));
            }
            push_flag_value(words, "--bearer", cmd.bearer.as_ref());
            for header in &cmd.header {
                push_flag_value(words, "--header", Some(header));
            }
            if cmd.peer {
                words.push("--peer".into());
            }
            push_flag_value(words, "--repository", cmd.repository.as_ref());
            push_flag_value(words, "--target", cmd.target.as_ref());
            words.extend(cmd.args.clone());
        }
        IntegrationsSubcommand::Remove(cmd) => words.extend(["remove".into(), cmd.name.clone()]),
        IntegrationsSubcommand::Attach(cmd) => {
            words.extend(["attach".into(), cmd.name.clone(), cmd.spec.clone()]);
        }
        IntegrationsSubcommand::Detach(cmd) => {
            words.extend(["detach".into(), cmd.name.clone(), cmd.spec.clone()]);
        }
        IntegrationsSubcommand::Rename(cmd) => {
            words.extend(["rename".into(), cmd.name.clone(), cmd.new_name.clone()]);
        }
    }
}

fn build_billing_command(words: &mut Vec<String>, command: &BillingSubcommand) {
    words.push("billing".into());
    words.push(
        match command {
            BillingSubcommand::Plan => "plan",
            BillingSubcommand::Update => "update",
            BillingSubcommand::Invoices => "invoices",
            BillingSubcommand::Receipts => "receipts",
        }
        .into(),
    );
}

fn build_shelley_command(words: &mut Vec<String>, command: &ShelleySubcommand) {
    words.push("shelley".into());
    match command {
        ShelleySubcommand::Install(cmd) => words.extend(["install".into(), cmd.vm.clone()]),
        ShelleySubcommand::Prompt(cmd) => {
            words.extend(["prompt".into(), cmd.vm.clone(), cmd.prompt.clone()]);
        }
    }
}

fn push_opt(words: &mut Vec<String>, value: Option<&String>) {
    if let Some(value) = value {
        words.push(value.clone());
    }
}

fn push_flag_value(words: &mut Vec<String>, flag: &str, value: Option<&String>) {
    if let Some(value) = value {
        words.push(flag.into());
        words.push(value.clone());
    }
}

fn ensure_non_empty<T>(items: &[T], message: &str) -> Result<()> {
    if items.is_empty() {
        bail!(message.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;
    use exedev_core::shell::shell_join;

    fn command_from(args: &[&str]) -> BuiltCommand {
        let cli = Cli::parse_from(args);
        build_command(&cli.command).unwrap()
    }

    #[test]
    fn builds_new_command() {
        let built = command_from(&[
            "exedev-ctl",
            "new",
            "--name",
            "p1-a-1",
            "--image",
            "ubuntu:22.04",
            "--env",
            "FOO=bar",
            "--no-email",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "new --env FOO=bar --image ubuntu:22.04 --name p1-a-1 --no-email"
        );
        assert!(!built.fallback_ssh);
    }

    #[test]
    fn new_stdin_prompt_uses_ssh_fallback() {
        let built = command_from(&["exedev-ctl", "new", "--prompt", "/dev/stdin"]);
        assert!(built.fallback_ssh);
    }

    #[test]
    fn builds_share_command() {
        let built = command_from(&[
            "exedev-ctl",
            "share",
            "add",
            "mybox",
            "user@example.com",
            "--message",
            "check this",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "share add mybox user@example.com --message 'check this'"
        );
    }

    #[test]
    fn builds_raw_exec_command() {
        let built = command_from(&["exedev-ctl", "exec", "--", "whoami"]);
        assert_eq!(shell_join(&built.words), "whoami");
    }
}
