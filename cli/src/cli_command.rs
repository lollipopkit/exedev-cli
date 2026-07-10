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
            if cmd.long {
                words.push("-l".into());
            }
            push_flag_value(&mut words, "--group", cmd.group.as_ref());
            push_opt(&mut words, cmd.pattern.as_ref());
        }
        Commands::New(cmd) => {
            words.push("new".into());
            push_flag_value(&mut words, "--command", cmd.command.as_ref());
            push_flag_value(&mut words, "--comment", cmd.comment.as_ref());
            push_flag_value(&mut words, "--cpu", cmd.cpu.as_ref());
            push_flag_value(&mut words, "--disk", cmd.disk.as_ref());
            for env in &cmd.envs {
                push_flag_value(&mut words, "--env", Some(env));
            }
            push_flag_value(&mut words, "--image", cmd.image.as_ref());
            for integration in &cmd.integration {
                push_flag_value(&mut words, "--integration", Some(integration));
            }
            push_flag_value(&mut words, "--memory", cmd.memory.as_ref());
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
            push_flag_value(&mut words, "--registry-auth", cmd.registry_auth.as_ref());
            push_flag_value(&mut words, "--setup-script", cmd.setup_script.as_ref());
            for tag in &cmd.tags {
                push_flag_value(&mut words, "--tag", Some(tag));
            }
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
            words.push(cmd.vm.clone());
            words.extend(cmd.tag_names.clone());
        }
        Commands::Comment(cmd) => {
            words.extend(["comment".into(), cmd.vm.clone(), cmd.text.clone()]);
        }
        Commands::Stat(cmd) => {
            words.extend(["stat".into(), cmd.vm_name.clone()]);
            push_flag_value(&mut words, "--range", cmd.range.as_ref());
        }
        Commands::Cp(cmd) => {
            words.extend(["cp".into(), cmd.source_vm.clone()]);
            push_opt(&mut words, cmd.new_name.as_ref());
            push_flag_value(&mut words, "--copy-tags", cmd.copy_tags.as_ref());
            push_flag_value(&mut words, "--cpu", cmd.cpu.as_ref());
            push_flag_value(&mut words, "--disk", cmd.disk.as_ref());
            push_flag_value(&mut words, "--memory", cmd.memory.as_ref());
        }
        Commands::Resize(cmd) => {
            if cmd.cpu.is_none() && cmd.disk.is_none() && cmd.memory.is_none() {
                bail!("resize requires at least one of --memory, --cpu, --disk");
            }
            words.extend(["resize".into(), cmd.vmname.clone()]);
            push_flag_value(&mut words, "--cpu", cmd.cpu.as_ref());
            push_flag_value(&mut words, "--disk", cmd.disk.as_ref());
            push_flag_value(&mut words, "--memory", cmd.memory.as_ref());
        }
        Commands::Share(cmd) => build_share_command(&mut words, &cmd.command),
        Commands::Domain(cmd) => build_domain_command(&mut words, &cmd.command)?,
        Commands::Team(cmd) => build_team_command(&mut words, cmd.command.as_ref()),
        Commands::Invite(cmd) => build_invite_command(&mut words, &cmd.command),
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

fn build_domain_command(words: &mut Vec<String>, command: &DomainSubcommand) -> Result<()> {
    words.push("domain".into());
    match command {
        DomainSubcommand::Add(cmd) => {
            words.push("add".into());
            if cmd.wildcard {
                words.push("--wildcard".into());
            }
            words.extend([cmd.vm.clone(), cmd.domain.clone()]);
        }
        DomainSubcommand::Ls(cmd) => {
            match (cmd.all, cmd.vm.as_ref()) {
                (false, None) => bail!("domain ls requires either <vm> or -a"),
                (true, Some(_)) => bail!("domain ls accepts either <vm> or -a, not both"),
                _ => {}
            }
            words.push("ls".into());
            if cmd.all {
                words.push("-a".into());
            }
            push_opt(words, cmd.vm.as_ref());
        }
        DomainSubcommand::Rm(cmd) => {
            words.extend(["rm".into(), cmd.vm.clone(), cmd.domain.clone()]);
        }
    }
    Ok(())
}

fn build_team_command(words: &mut Vec<String>, command: Option<&TeamSubcommand>) {
    words.push("team".into());
    let Some(command) = command else {
        return;
    };
    match command {
        TeamSubcommand::Disable => {
            // /exec has no pty, so the server-side confirmation prompt cannot
            // be answered; the local dangerous-command guard already confirmed.
            words.extend(["disable".into(), "--yes".into()]);
        }
        TeamSubcommand::Members => words.push("members".into()),
        TeamSubcommand::Add(cmd) => words.extend(["add".into(), cmd.email.clone()]),
        TeamSubcommand::Remove(cmd) => words.extend(["remove".into(), cmd.email.clone()]),
        TeamSubcommand::Role(cmd) => {
            words.extend(["role".into(), cmd.email.clone(), cmd.role.clone()]);
        }
        TeamSubcommand::Rename(cmd) => words.extend(["rename".into(), cmd.name.clone()]),
        TeamSubcommand::Billing(cmd) => {
            words.push("billing".into());
            if let Some(TeamBillingSubcommand::Update(update)) = &cmd.command {
                words.push("update".into());
                push_flag_value(words, "--name", update.name.as_ref());
                push_flag_value(words, "--business-name", update.business_name.as_ref());
                push_flag_value(words, "--phone", update.phone.as_ref());
                push_flag_value(words, "--address-line1", update.address_line1.as_ref());
                push_flag_value(words, "--address-line2", update.address_line2.as_ref());
                push_flag_value(words, "--address-city", update.address_city.as_ref());
                push_flag_value(words, "--address-state", update.address_state.as_ref());
                push_flag_value(
                    words,
                    "--address-postal-code",
                    update.address_postal_code.as_ref(),
                );
                push_flag_value(words, "--address-country", update.address_country.as_ref());
            }
        }
        TeamSubcommand::Transfer(cmd) => {
            words.extend([
                "transfer".into(),
                cmd.vm_name.clone(),
                cmd.target_email.clone(),
            ]);
        }
        TeamSubcommand::Auth(cmd) => {
            words.push("auth".into());
            if let Some(TeamAuthSubcommand::Set(set)) = &cmd.command {
                words.extend(["set".into(), set.provider.clone()]);
                push_flag_value(words, "--issuer-url", set.issuer_url.as_ref());
                push_flag_value(words, "--client-id", set.client_id.as_ref());
                push_flag_value(words, "--client-secret", set.client_secret.as_ref());
                push_flag_value(words, "--display-name", set.display_name.as_ref());
            }
        }
        TeamSubcommand::Settings(cmd) => {
            words.push("settings".into());
            if let Some(TeamSettingsSubcommand::VmSharing(sharing)) = &cmd.command {
                words.extend(["vm-sharing".into(), sharing.value.clone()]);
            }
        }
        TeamSubcommand::Vm(cmd) => {
            words.push("vm".into());
            if let Some(TeamVmSubcommand::Ls(ls)) = &cmd.command {
                words.push("ls".into());
                if ls.long {
                    words.push("-l".into());
                }
                push_flag_value(words, "--group", ls.group.as_ref());
                push_opt(words, ls.pattern.as_ref());
            }
        }
    }
}

fn build_invite_command(words: &mut Vec<String>, command: &InviteSubcommand) {
    words.push("invite".into());
    match command {
        InviteSubcommand::Show => words.push("show".into()),
        InviteSubcommand::Link => words.push("link".into()),
        InviteSubcommand::Rewards => words.push("rewards".into()),
        InviteSubcommand::SetReward(cmd) => {
            words.extend(["set-reward".into(), cmd.reward.clone()]);
        }
        InviteSubcommand::Activity => words.push("activity".into()),
        InviteSubcommand::Request => words.push("request".into()),
        InviteSubcommand::Manage => words.push("manage".into()),
    }
}

fn build_ssh_key_command(words: &mut Vec<String>, command: &SshKeySubcommand) {
    words.push("ssh-key".into());
    match command {
        SshKeySubcommand::List => words.push("list".into()),
        SshKeySubcommand::Add(cmd) => {
            words.push("add".into());
            push_flag_value(words, "--tag", cmd.tag.as_ref());
            words.push(cmd.public_key.clone());
        }
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
            push_flag_value(words, "--name", cmd.name.as_ref());
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
            if cmd.act_as_user {
                words.push("--act-as-user".into());
            }
            for attach in &cmd.attach {
                push_flag_value(words, "--attach", Some(attach));
            }
            push_flag_value(words, "--bearer", cmd.bearer.as_ref());
            push_flag_value(words, "--comment", cmd.comment.as_ref());
            push_flag_value(words, "--fields", cmd.fields.as_ref());
            for header in &cmd.header {
                push_flag_value(words, "--header", Some(header));
            }
            if cmd.no_auth {
                words.push("--no-auth".into());
            }
            if cmd.peer {
                words.push("--peer".into());
            }
            push_flag_value(words, "--repository", cmd.repository.as_ref());
            push_flag_value(words, "--target", cmd.target.as_ref());
            words.extend(cmd.args.clone());
        }
        IntegrationsSubcommand::Edit(cmd) => {
            words.extend(["edit".into(), cmd.name.clone()]);
            if cmd.team {
                words.push("--team".into());
            }
            if cmd.act_as_user {
                words.push("--act-as-user".into());
            }
            push_flag_value(words, "--bearer", cmd.bearer.as_ref());
            if cmd.clear_header {
                words.push("--clear-header".into());
            }
            push_flag_value(words, "--comment", cmd.comment.as_ref());
            push_flag_value(words, "--fields", cmd.fields.as_ref());
            for header in &cmd.header {
                push_flag_value(words, "--header", Some(header));
            }
            if cmd.no_auth {
                words.push("--no-auth".into());
            }
            push_flag_value(words, "--repository", cmd.repository.as_ref());
            push_flag_value(words, "--target", cmd.target.as_ref());
            push_flag_value(words, "--webhook-url", cmd.webhook_url.as_ref());
            words.extend(cmd.args.clone());
        }
        IntegrationsSubcommand::Remove(cmd) => {
            words.extend(["remove".into(), cmd.name.clone()]);
            if cmd.team {
                words.push("--team".into());
            }
        }
        IntegrationsSubcommand::Attach(cmd) => {
            words.extend(["attach".into(), cmd.name.clone(), cmd.spec.clone()]);
            if cmd.team {
                words.push("--team".into());
            }
        }
        IntegrationsSubcommand::Detach(cmd) => {
            words.extend(["detach".into(), cmd.name.clone(), cmd.spec.clone()]);
            if cmd.team {
                words.push("--team".into());
            }
        }
        IntegrationsSubcommand::Rename(cmd) => {
            words.extend(["rename".into(), cmd.name.clone(), cmd.new_name.clone()]);
            if cmd.team {
                words.push("--team".into());
            }
        }
    }
}

fn build_billing_command(words: &mut Vec<String>, command: &BillingSubcommand) {
    words.push("billing".into());
    match command {
        BillingSubcommand::Plan => words.push("plan".into()),
        BillingSubcommand::Usage(cmd) => {
            words.push("usage".into());
            push_flag_value(words, "--range", cmd.range.as_ref());
        }
        BillingSubcommand::Credits => words.push("credits".into()),
        BillingSubcommand::Rewards => words.push("rewards".into()),
        BillingSubcommand::Capacity => words.push("capacity".into()),
        BillingSubcommand::Manage => words.push("manage".into()),
        BillingSubcommand::Invoices => words.push("invoices".into()),
        BillingSubcommand::Receipts => words.push("receipts".into()),
    }
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
    fn builds_domain_commands() {
        let built = command_from(&["exedev-ctl", "domain", "add", "mybox", "app.example.com"]);
        assert_eq!(shell_join(&built.words), "domain add mybox app.example.com");

        let built = command_from(&["exedev-ctl", "domain", "ls", "mybox"]);
        assert_eq!(shell_join(&built.words), "domain ls mybox");

        let built = command_from(&["exedev-ctl", "domain", "ls", "-a"]);
        assert_eq!(shell_join(&built.words), "domain ls -a");

        let built = command_from(&["exedev-ctl", "domain", "rm", "mybox", "app.example.com"]);
        assert_eq!(shell_join(&built.words), "domain rm mybox app.example.com");
    }

    #[test]
    fn rejects_invalid_domain_ls_args() {
        let cli = Cli::parse_from(["exedev-ctl", "domain", "ls"]);
        let err = build_command(&cli.command).unwrap_err();
        assert_eq!(err.to_string(), "domain ls requires either <vm> or -a");

        let cli = Cli::parse_from(["exedev-ctl", "domain", "ls", "-a", "mybox"]);
        let err = build_command(&cli.command).unwrap_err();
        assert_eq!(
            err.to_string(),
            "domain ls accepts either <vm> or -a, not both"
        );
    }

    #[test]
    fn builds_raw_exec_command() {
        let built = command_from(&["exedev-ctl", "exec", "--", "whoami"]);
        assert_eq!(shell_join(&built.words), "whoami");
    }

    #[test]
    fn builds_ls_with_group() {
        let built = command_from(&["exedev-ctl", "ls", "-l", "--group", "tag", "p1-*"]);
        assert_eq!(shell_join(&built.words), "ls -l --group tag 'p1-*'");
    }

    #[test]
    fn builds_new_command_with_resources_and_tags() {
        let built = command_from(&[
            "exedev-ctl",
            "new",
            "--name",
            "p1-a-1",
            "--cpu",
            "4",
            "--memory",
            "16GB",
            "--tag",
            "prod",
            "--tag",
            "web",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "new --cpu 4 --memory 16GB --name p1-a-1 --tag prod --tag web"
        );
    }

    #[test]
    fn builds_comment_command() {
        let built = command_from(&["exedev-ctl", "comment", "mybox", "staging copy"]);
        assert_eq!(shell_join(&built.words), "comment mybox 'staging copy'");

        let built = command_from(&["exedev-ctl", "comment", "mybox", ""]);
        assert_eq!(shell_join(&built.words), "comment mybox ''");
    }

    #[test]
    fn builds_tag_command_with_multiple_tags() {
        let built = command_from(&["exedev-ctl", "tag", "-d", "mybox", "prod", "web"]);
        assert_eq!(shell_join(&built.words), "tag -d mybox prod web");
    }

    #[test]
    fn builds_resize_command() {
        let built = command_from(&["exedev-ctl", "resize", "mybox", "--memory", "8GB"]);
        assert_eq!(shell_join(&built.words), "resize mybox --memory 8GB");
    }

    #[test]
    fn rejects_resize_without_flags() {
        let cli = Cli::parse_from(["exedev-ctl", "resize", "mybox"]);
        let err = build_command(&cli.command).unwrap_err();
        assert_eq!(
            err.to_string(),
            "resize requires at least one of --memory, --cpu, --disk"
        );
    }

    #[test]
    fn builds_domain_add_wildcard() {
        let built = command_from(&[
            "exedev-ctl",
            "domain",
            "add",
            "--wildcard",
            "mybox",
            "app.example.com",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "domain add --wildcard mybox app.example.com"
        );
    }

    #[test]
    fn builds_ssh_key_add_with_tag() {
        let built = command_from(&[
            "exedev-ctl",
            "ssh-key",
            "add",
            "--tag",
            "prod",
            "ssh-ed25519 AAAA key",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "ssh-key add --tag prod 'ssh-ed25519 AAAA key'"
        );
    }

    #[test]
    fn builds_bare_team_command() {
        let built = command_from(&["exedev-ctl", "team"]);
        assert_eq!(shell_join(&built.words), "team");
    }

    #[test]
    fn team_disable_forwards_yes() {
        let built = command_from(&["exedev-ctl", "team", "disable"]);
        assert_eq!(shell_join(&built.words), "team disable --yes");
    }

    #[test]
    fn builds_team_commands() {
        let built = command_from(&["exedev-ctl", "team", "role", "a@b.c", "admin"]);
        assert_eq!(shell_join(&built.words), "team role a@b.c admin");

        let built = command_from(&["exedev-ctl", "team", "transfer", "mybox", "a@b.c"]);
        assert_eq!(shell_join(&built.words), "team transfer mybox a@b.c");

        let built = command_from(&[
            "exedev-ctl",
            "team",
            "billing",
            "update",
            "--name",
            "ACME",
            "--address-city",
            "Berlin",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "team billing update --name ACME --address-city Berlin"
        );

        let built = command_from(&[
            "exedev-ctl",
            "team",
            "auth",
            "set",
            "oidc",
            "--issuer-url",
            "https://accounts.google.com",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "team auth set oidc --issuer-url https://accounts.google.com"
        );

        let built = command_from(&[
            "exedev-ctl",
            "team",
            "settings",
            "vm-sharing",
            "all-members",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "team settings vm-sharing all-members"
        );

        let built = command_from(&["exedev-ctl", "team", "vm", "ls", "-l", "--group", "user"]);
        assert_eq!(shell_join(&built.words), "team vm ls -l --group user");
    }

    #[test]
    fn builds_invite_commands() {
        let built = command_from(&["exedev-ctl", "invite", "show"]);
        assert_eq!(shell_join(&built.words), "invite show");

        let built = command_from(&["exedev-ctl", "invite", "set-reward", "bonus-credits"]);
        assert_eq!(shell_join(&built.words), "invite set-reward bonus-credits");
    }

    #[test]
    fn builds_billing_commands() {
        let built = command_from(&["exedev-ctl", "billing", "usage", "--range", "cycle"]);
        assert_eq!(shell_join(&built.words), "billing usage --range cycle");

        let built = command_from(&["exedev-ctl", "billing", "capacity"]);
        assert_eq!(shell_join(&built.words), "billing capacity");
    }

    #[test]
    fn builds_integrations_edit_command() {
        let built = command_from(&[
            "exedev-ctl",
            "integrations",
            "edit",
            "myproxy",
            "--team",
            "--target",
            "http://localhost:9000",
            "--clear-header",
        ]);
        assert_eq!(
            shell_join(&built.words),
            "integrations edit myproxy --team --clear-header --target http://localhost:9000"
        );

        let built = command_from(&["exedev-ctl", "integrations", "remove", "myproxy", "--team"]);
        assert_eq!(
            shell_join(&built.words),
            "integrations remove myproxy --team"
        );
    }
}
