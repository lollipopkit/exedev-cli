use clap::{Args, Parser, Subcommand};
use exedev_core::DEFAULT_ENDPOINT;

#[derive(Debug, Parser)]
#[command(name = "exedevctl")]
#[command(about = "Rust CLI for exe.dev over HTTPS /exec")]
#[command(version)]
#[command(disable_help_subcommand = true)]
pub(crate) struct Cli {
    #[arg(long, global = true, default_value = DEFAULT_ENDPOINT)]
    pub(crate) endpoint: String,

    #[arg(long, global = true)]
    pub(crate) json: bool,

    #[arg(long, global = true)]
    pub(crate) yes: bool,

    #[command(subcommand)]
    pub(crate) command: Commands,
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
    /// Show VM metrics.
    Stat(StatCmd),
    /// Copy an existing VM.
    Cp(CpCmd),
    /// Resize a VM disk.
    Resize(ResizeCmd),
    /// Share HTTPS VM access.
    Share(ShareCmd),
    /// View and manage your team.
    Team(TeamCmd),
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
    #[arg(short = 'a', long = "a")]
    pub(crate) all: bool,
    #[arg(short = 'l', long = "l")]
    pub(crate) long: bool,
    pub(crate) pattern: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct NewCmd {
    #[arg(long)]
    pub(crate) command: Option<String>,
    #[arg(long)]
    pub(crate) disk: Option<String>,
    #[arg(long = "env")]
    pub(crate) envs: Vec<String>,
    #[arg(long)]
    pub(crate) image: Option<String>,
    #[arg(long)]
    pub(crate) integration: Vec<String>,
    #[arg(long)]
    pub(crate) name: Option<String>,
    #[arg(long)]
    pub(crate) no_email: bool,
    #[arg(long)]
    pub(crate) prompt: Option<String>,
    #[arg(long)]
    pub(crate) setup_script: Option<String>,
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
    pub(crate) tag_name: String,
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
    pub(crate) disk: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct ResizeCmd {
    pub(crate) vmname: String,
    #[arg(long)]
    pub(crate) disk: String,
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
pub(crate) struct TeamCmd {
    #[command(subcommand)]
    pub(crate) command: TeamSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TeamSubcommand {
    Members,
    Add(EmailCmd),
    Remove(EmailCmd),
}

#[derive(Debug, Args)]
pub(crate) struct EmailCmd {
    pub(crate) email: String,
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
    pub(crate) attach: Vec<String>,
    #[arg(long)]
    pub(crate) bearer: Option<String>,
    #[arg(long)]
    pub(crate) header: Vec<String>,
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
pub(crate) struct NameCmd {
    pub(crate) name: String,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationAttachCmd {
    pub(crate) name: String,
    pub(crate) spec: String,
}

#[derive(Debug, Args)]
pub(crate) struct IntegrationRenameCmd {
    pub(crate) name: String,
    pub(crate) new_name: String,
}

#[derive(Debug, Args)]
pub(crate) struct BillingCmd {
    #[command(subcommand)]
    pub(crate) command: BillingSubcommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BillingSubcommand {
    Plan,
    Update,
    Invoices,
    Receipts,
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
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
