use clap::{Args, Parser, Subcommand, ValueEnum};
use exedev_core::DEFAULT_ENDPOINT;

#[derive(Debug, Parser)]
#[command(name = "exedev-ctl")]
#[command(about = "Rust CLI for exe.dev over SSH by default, with optional HTTPS /exec transport")]
#[command(version)]
#[command(disable_help_subcommand = true)]
pub(crate) struct Cli {
    #[arg(long, global = true, default_value = DEFAULT_ENDPOINT, help = "HTTPS /exec endpoint used only with --transport http")]
    pub(crate) endpoint: String,

    #[arg(long, global = true, value_enum, default_value_t = Transport::Ssh)]
    pub(crate) transport: Transport,

    #[arg(long, global = true)]
    pub(crate) json: bool,

    #[arg(long, global = true)]
    pub(crate) yes: bool,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Transport {
    Ssh,
    Http,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    /// Show help information from exe.dev.
    Help(HelpCmd),
    /// Browse documentation.
    Doc(DocCmd),
    /// List your VMs.
    Ls(LsCmd),
    /// Create a new VM.
    New(NewCmd),
    /// Delete one or more VMs.
    Rm(RmCmd),
    /// Restart a VM.
    Restart(RestartCmd),
    /// Rename a VM.
    Rename(RenameCmd),
    /// Add or remove a tag on a VM.
    Tag(TagCmd),
    /// Set or clear a short comment on a VM.
    Comment(CommentCmd),
    /// Show VM metrics.
    Stat(StatCmd),
    /// Copy an existing VM.
    Cp(CpCmd),
    /// Resize a VM's resources (memory, CPU, disk).
    Resize(ResizeCmd),
    /// Share HTTPS VM access.
    Share(ShareCmd),
    /// Manage custom domains.
    Domain(DomainCmd),
    /// View and manage your team.
    Team(TeamCmd),
    /// Manage your invite link and rewards.
    Invite(InviteCmd),
    /// Show current user information.
    Whoami,
    /// Manage SSH keys.
    #[command(name = "ssh-key")]
    SshKey(SshKeyCmd),
    /// Set preferred region for new VMs.
    #[command(name = "set-region")]
    SetRegion(SetRegionCmd),
    /// Manage integrations.
    #[command(alias = "int")]
    Integrations(IntegrationsCmd),
    /// View and manage billing.
    Billing(BillingCmd),
    /// Manage Shelley agent on VMs.
    Shelley(ShelleyCmd),
    /// Generate a browser magic link.
    Browser(BrowserCmd),
    /// SSH into a VM. Falls back to local ssh because HTTPS /exec has no pty.
    Ssh(SshCmd),
    /// Allow exe.dev support root access.
    #[command(name = "grant-support-root")]
    GrantSupportRoot(GrantSupportRootCmd),
    /// REPL compatibility no-op.
    Exit,
    /// Send a raw exe.dev command to /exec.
    Exec(ExecCmd),
}

#[derive(Debug, Args)]
pub(crate) struct HelpCmd {
    pub(crate) command: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct DocCmd {
    pub(crate) slug: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct LsCmd {
    #[arg(short = 'l', long = "l")]
    pub(crate) long: bool,
    #[arg(long)]
    pub(crate) group: Option<String>,
    pub(crate) pattern: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct NewCmd {
    #[arg(long)]
    pub(crate) command: Option<String>,
    #[arg(long)]
    pub(crate) comment: Option<String>,
    #[arg(long)]
    pub(crate) cpu: Option<String>,
    #[arg(long)]
    pub(crate) disk: Option<String>,
    #[arg(long = "env")]
    pub(crate) envs: Vec<String>,
    #[arg(long)]
    pub(crate) image: Option<String>,
    #[arg(long)]
    pub(crate) integration: Vec<String>,
    #[arg(long)]
    pub(crate) memory: Option<String>,
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long)]
    pub(crate) no_email: bool,
    #[arg(long)]
    pub(crate) prompt: Option<String>,
    #[arg(long)]
    pub(crate) registry_auth: Option<String>,
    #[arg(long)]
    pub(crate) setup_script: Option<String>,
    #[arg(long = "tag")]
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct RmCmd {
    pub(crate) vmnames: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct RestartCmd {
    pub(crate) vmname: String,
}

#[derive(Debug, Args)]
pub(crate) struct RenameCmd {
    pub(crate) oldname: String,
    pub(crate) newname: String,
}

#[derive(Debug, Args)]
pub(crate) struct TagCmd {
    #[arg(short = 'd', long = "d")]
    pub(crate) delete: bool,
    pub(crate) vm: String,
    #[arg(required = true, num_args = 1..)]
    pub(crate) tag_names: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct CommentCmd {
    pub(crate) vm: String,
    /// Comment text; pass an empty string to clear the comment.
    pub(crate) text: String,
}

#[derive(Debug, Args)]
pub(crate) struct StatCmd {
    pub(crate) vm_name: String,
    #[arg(long)]
    pub(crate) range: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct CpCmd {
    pub(crate) source_vm: String,
    pub(crate) new_name: Option<String>,
    #[arg(long)]
    pub(crate) copy_tags: Option<String>,
    #[arg(long)]
    pub(crate) cpu: Option<String>,
    #[arg(long)]
    pub(crate) disk: Option<String>,
    #[arg(long)]
    pub(crate) memory: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ResizeCmd {
    pub(crate) vmname: String,
    #[arg(long)]
    pub(crate) cpu: Option<String>,
    #[arg(long)]
    pub(crate) disk: Option<String>,
    #[arg(long)]
    pub(crate) memory: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ShareCmd {
    #[command(subcommand)]
    pub(crate) command: ShareSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ShareSubcommand {
    Show(ShareShowCmd),
    Port(SharePortCmd),
    #[command(name = "set-public")]
    SetPublic(ShareVmCmd),
    #[command(name = "set-private")]
    SetPrivate(ShareVmCmd),
    Add(ShareAddCmd),
    Remove(ShareRemoveCmd),
    #[command(name = "add-link", alias = "add-share-link")]
    AddLink(ShareShowCmd),
    #[command(name = "remove-link", alias = "remove-share-link")]
    RemoveLink(ShareRemoveLinkCmd),
    #[command(name = "receive-email")]
    ReceiveEmail(ShareReceiveEmailCmd),
    Access(ShareAccessCmd),
}

#[derive(Debug, Args)]
pub(crate) struct ShareShowCmd {
    pub(crate) vm: String,
    #[arg(long)]
    pub(crate) qr: bool,
}

#[derive(Debug, Args)]
pub(crate) struct SharePortCmd {
    pub(crate) vm: String,
    pub(crate) port: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ShareVmCmd {
    pub(crate) vm: String,
}

#[derive(Debug, Args)]
pub(crate) struct ShareAddCmd {
    pub(crate) vm: String,
    pub(crate) target: String,
    #[arg(long)]
    pub(crate) message: Option<String>,
    #[arg(long)]
    pub(crate) qr: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ShareRemoveCmd {
    pub(crate) vm: String,
    pub(crate) target: String,
}

#[derive(Debug, Args)]
pub(crate) struct ShareRemoveLinkCmd {
    pub(crate) vm: String,
    pub(crate) token: String,
}

#[derive(Debug, Args)]
pub(crate) struct ShareReceiveEmailCmd {
    pub(crate) vm: String,
    pub(crate) state: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ShareAccessCmd {
    pub(crate) action: String,
    pub(crate) vm: String,
}

#[derive(Debug, Args)]
pub(crate) struct DomainCmd {
    #[command(subcommand)]
    pub(crate) command: DomainSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DomainSubcommand {
    /// Register a custom domain for a VM.
    Add(DomainAddCmd),
    /// List registered custom domains.
    Ls(DomainLsCmd),
    /// Remove a custom domain from a VM.
    Rm(DomainVmDomainCmd),
}

#[derive(Debug, Args)]
pub(crate) struct DomainAddCmd {
    /// Issue a wildcard (*.<parent>) certificate via DNS-01 delegation.
    #[arg(long)]
    pub(crate) wildcard: bool,
    pub(crate) vm: String,
    pub(crate) domain: String,
}

#[derive(Debug, Args)]
pub(crate) struct DomainVmDomainCmd {
    pub(crate) vm: String,
    pub(crate) domain: String,
}

#[derive(Debug, Args)]
pub(crate) struct DomainLsCmd {
    #[arg(short = 'a', long = "all")]
    pub(crate) all: bool,
    pub(crate) vm: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct TeamCmd {
    #[command(subcommand)]
    pub(crate) command: Option<TeamSubcommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamSubcommand {
    /// Disband your team.
    Disable,
    /// List team members.
    Members,
    /// Add a team member.
    Add(EmailCmd),
    /// Remove a team member.
    Remove(EmailCmd),
    /// Change a team member's role.
    Role(TeamRoleCmd),
    /// Rename your team.
    Rename(TeamRenameCmd),
    /// View and manage team billing information.
    Billing(TeamBillingCmd),
    /// Transfer a VM to another team member.
    Transfer(TeamTransferCmd),
    /// View and manage team auth settings.
    Auth(TeamAuthCmd),
    /// View and manage team settings.
    Settings(TeamSettingsCmd),
    /// View team members' VMs.
    Vm(TeamVmCmd),
}

#[derive(Debug, Args)]
pub(crate) struct EmailCmd {
    pub(crate) email: String,
}

#[derive(Debug, Args)]
pub(crate) struct TeamRoleCmd {
    pub(crate) email: String,
    /// One of user, admin, billing_owner.
    pub(crate) role: String,
}

#[derive(Debug, Args)]
pub(crate) struct TeamRenameCmd {
    pub(crate) name: String,
}

#[derive(Debug, Args)]
pub(crate) struct TeamBillingCmd {
    #[command(subcommand)]
    pub(crate) command: Option<TeamBillingSubcommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamBillingSubcommand {
    /// Update team billing information.
    Update(TeamBillingUpdateCmd),
}

#[derive(Debug, Args)]
pub(crate) struct TeamBillingUpdateCmd {
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long)]
    pub(crate) business_name: Option<String>,
    #[arg(long)]
    pub(crate) phone: Option<String>,
    #[arg(long)]
    pub(crate) address_line1: Option<String>,
    #[arg(long)]
    pub(crate) address_line2: Option<String>,
    #[arg(long)]
    pub(crate) address_city: Option<String>,
    #[arg(long)]
    pub(crate) address_state: Option<String>,
    #[arg(long)]
    pub(crate) address_postal_code: Option<String>,
    #[arg(long)]
    pub(crate) address_country: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct TeamTransferCmd {
    pub(crate) vm_name: String,
    pub(crate) target_email: String,
}

#[derive(Debug, Args)]
pub(crate) struct TeamAuthCmd {
    #[command(subcommand)]
    pub(crate) command: Option<TeamAuthSubcommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamAuthSubcommand {
    /// Set the team auth provider.
    Set(TeamAuthSetCmd),
}

#[derive(Debug, Args)]
pub(crate) struct TeamAuthSetCmd {
    /// One of default, google, oidc.
    pub(crate) provider: String,
    #[arg(long)]
    pub(crate) issuer_url: Option<String>,
    #[arg(long)]
    pub(crate) client_id: Option<String>,
    #[arg(long)]
    pub(crate) client_secret: Option<String>,
    #[arg(long)]
    pub(crate) display_name: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct TeamSettingsCmd {
    #[command(subcommand)]
    pub(crate) command: Option<TeamSettingsSubcommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamSettingsSubcommand {
    /// Set who can share team VMs.
    #[command(name = "vm-sharing")]
    VmSharing(TeamVmSharingCmd),
}

#[derive(Debug, Args)]
pub(crate) struct TeamVmSharingCmd {
    /// One of admins-only, all-members.
    pub(crate) value: String,
}

#[derive(Debug, Args)]
pub(crate) struct TeamVmCmd {
    #[command(subcommand)]
    pub(crate) command: Option<TeamVmSubcommand>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamVmSubcommand {
    /// List all VMs across your team.
    #[command(alias = "list")]
    Ls(TeamVmLsCmd),
}

#[derive(Debug, Args)]
pub(crate) struct TeamVmLsCmd {
    #[arg(short = 'l', long = "l")]
    pub(crate) long: bool,
    #[arg(long)]
    pub(crate) group: Option<String>,
    pub(crate) pattern: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct InviteCmd {
    #[command(subcommand)]
    pub(crate) command: InviteSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum InviteSubcommand {
    /// Show your active invite link and reward.
    Show,
    /// Print only your active invite link.
    Link,
    /// List invite rewards you can use.
    Rewards,
    /// Choose the reward for your invite link.
    #[command(name = "set-reward")]
    SetReward(InviteSetRewardCmd),
    /// Show signups, upgrades, and reward status.
    Activity,
    /// Request more trial invites.
    Request,
    /// Open the invites page.
    Manage,
}

#[derive(Debug, Args)]
pub(crate) struct InviteSetRewardCmd {
    /// One of standard, bonus-credits, extra-memory, extra-disk.
    pub(crate) reward: String,
}

#[derive(Debug, Args)]
pub(crate) struct SshKeyCmd {
    #[command(subcommand)]
    pub(crate) command: SshKeySubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SshKeySubcommand {
    List,
    Add(SshKeyAddCmd),
    Remove(SshKeyRemoveCmd),
    Rename(SshKeyRenameCmd),
    #[command(name = "generate-api-key")]
    GenerateApiKey(SshKeyGenerateApiKeyCmd),
}

#[derive(Debug, Args)]
pub(crate) struct SshKeyAddCmd {
    /// Scope the key to VMs with this tag.
    #[arg(long)]
    pub(crate) tag: Option<String>,
    pub(crate) public_key: String,
}

#[derive(Debug, Args)]
pub(crate) struct SshKeyRemoveCmd {
    pub(crate) key: String,
}

#[derive(Debug, Args)]
pub(crate) struct SshKeyRenameCmd {
    pub(crate) old_name: String,
    pub(crate) new_name: String,
}

#[derive(Debug, Args)]
pub(crate) struct SshKeyGenerateApiKeyCmd {
    #[arg(long)]
    pub(crate) label: Option<String>,
    #[arg(long)]
    pub(crate) vm: Option<String>,
    #[arg(long)]
    pub(crate) cmds: Option<String>,
    #[arg(long)]
    pub(crate) exp: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SetRegionCmd {
    pub(crate) region_code: String,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationsCmd {
    #[command(subcommand)]
    pub(crate) command: IntegrationsSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum IntegrationsSubcommand {
    List,
    Setup(IntegrationSetupCmd),
    Add(IntegrationAddCmd),
    Edit(IntegrationEditCmd),
    Remove(NameCmd),
    Attach(IntegrationAttachCmd),
    Detach(IntegrationAttachCmd),
    Rename(IntegrationRenameCmd),
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationSetupCmd {
    pub(crate) integration_type: String,
    #[arg(short = 'd', long = "d")]
    pub(crate) disconnect: bool,
    #[arg(long)]
    pub(crate) delete: bool,
    #[arg(long)]
    pub(crate) list: bool,
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long)]
    pub(crate) verify: bool,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationAddCmd {
    pub(crate) integration_type: String,
    #[arg(long)]
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) team: bool,
    #[arg(long)]
    pub(crate) act_as_user: bool,
    #[arg(long)]
    pub(crate) attach: Vec<String>,
    #[arg(long)]
    pub(crate) bearer: Option<String>,
    #[arg(long)]
    pub(crate) comment: Option<String>,
    #[arg(long)]
    pub(crate) fields: Option<String>,
    #[arg(long)]
    pub(crate) header: Vec<String>,
    #[arg(long)]
    pub(crate) no_auth: bool,
    #[arg(long)]
    pub(crate) peer: bool,
    #[arg(long)]
    pub(crate) repository: Option<String>,
    #[arg(long)]
    pub(crate) target: Option<String>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationEditCmd {
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) team: bool,
    #[arg(long)]
    pub(crate) act_as_user: bool,
    #[arg(long)]
    pub(crate) bearer: Option<String>,
    #[arg(long)]
    pub(crate) clear_header: bool,
    #[arg(long)]
    pub(crate) comment: Option<String>,
    #[arg(long)]
    pub(crate) fields: Option<String>,
    #[arg(long)]
    pub(crate) header: Vec<String>,
    #[arg(long)]
    pub(crate) no_auth: bool,
    #[arg(long)]
    pub(crate) repository: Option<String>,
    #[arg(long)]
    pub(crate) target: Option<String>,
    #[arg(long)]
    pub(crate) webhook_url: Option<String>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct NameCmd {
    pub(crate) name: String,
    #[arg(long)]
    pub(crate) team: bool,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationAttachCmd {
    pub(crate) name: String,
    pub(crate) spec: String,
    #[arg(long)]
    pub(crate) team: bool,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationRenameCmd {
    pub(crate) name: String,
    pub(crate) new_name: String,
    #[arg(long)]
    pub(crate) team: bool,
}

#[derive(Debug, Args)]
pub(crate) struct BillingCmd {
    #[command(subcommand)]
    pub(crate) command: BillingSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BillingSubcommand {
    /// Show your current plan and resource limits.
    Plan,
    /// Show resource usage against your plan.
    Usage(BillingUsageCmd),
    /// Show Shelley credit balances.
    Credits,
    /// Show invite rewards you've earned.
    Rewards,
    /// Change your subscription capacity.
    Capacity,
    /// Open the billing page.
    Manage,
    /// Show invoices.
    Invoices,
    /// Show receipts for credit purchases.
    Receipts,
}

#[derive(Debug, Args)]
pub(crate) struct BillingUsageCmd {
    /// Time range: cycle, 24h, 7d, or 30d.
    #[arg(long)]
    pub(crate) range: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ShelleyCmd {
    #[command(subcommand)]
    pub(crate) command: ShelleySubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ShelleySubcommand {
    Install(ShareVmCmd),
    Prompt(ShelleyPromptCmd),
}

#[derive(Debug, Args)]
pub(crate) struct ShelleyPromptCmd {
    pub(crate) vm: String,
    pub(crate) prompt: String,
}

#[derive(Debug, Args)]
pub(crate) struct BrowserCmd {
    #[arg(long)]
    pub(crate) qr: bool,
}

#[derive(Debug, Args)]
pub(crate) struct SshCmd {
    #[arg(short = 'l')]
    pub(crate) user: Option<String>,
    pub(crate) target: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub(crate) command: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct GrantSupportRootCmd {
    pub(crate) vmname: String,
    pub(crate) state: String,
}

#[derive(Debug, Args)]
pub(crate) struct ExecCmd {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub(crate) command: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn transport_defaults_to_ssh() {
        let cli = Cli::parse_from(["exedev-ctl", "ls"]);
        assert_eq!(cli.transport, Transport::Ssh);
    }

    #[test]
    fn parses_http_transport() {
        let cli = Cli::parse_from(["exedev-ctl", "--transport", "http", "ls"]);
        assert_eq!(cli.transport, Transport::Http);
    }

    #[test]
    fn parses_ssh_transport() {
        let cli = Cli::parse_from(["exedev-ctl", "--transport", "ssh", "ls"]);
        assert_eq!(cli.transport, Transport::Ssh);
    }

    #[test]
    fn rejects_unknown_transport() {
        let result = Cli::try_parse_from(["exedev-ctl", "--transport", "auto", "ls"]);
        assert!(result.is_err());
    }
}
