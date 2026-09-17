//! Shared subcommand structs that 60+ CLIs re-implement.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// `init` subcommand: scaffold a new project.
#[derive(Debug, Args)]
pub struct InitCmd {
    /// Path to initialize (defaults to cwd)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Force overwrite if path exists
    #[arg(short, long)]
    pub force: bool,

    /// Template to use (defaults to "default").
    ///
    /// Treated as an **identifier** (e.g. `"default"`, `"rust"`, `"python"`)
    /// by `clap-ext`. Use [`InitCmd::validate_template`] when wiring
    /// `InitCmd` to any future template loader that resolves the template
    /// against a directory; the helper rejects names that look like paths
    /// (`/`, `\`, `..`) so an attacker-controlled `--template` cannot escape
    /// the templates root.
    #[arg(short, long, default_value = "default")]
    pub template: String,
}

impl InitCmd {
    /// Defense-in-depth check for the `--template` value.
    ///
    /// `init` itself does **not** read any template file today (this
    /// crate only defines the parsed shape — file I/O is the host
    /// binary's responsibility). Even so, treat `--template` as a
    /// **name**, never a path: a future consumer that wires
    /// `InitCmd` to a `templates/<name>/...` loader must not let the
    /// user escape that root via `..`, `/`, `\`, or an absolute path.
    ///
    /// Returns `Ok(())` for valid identifiers like `"default"`,
    /// `"rust-cli"`, `"python.project"`. Returns `Err(reason)` for
    /// anything that smells like a path or traversal segment.
    pub fn validate_template(&self) -> Result<(), String> {
        let t = self.template.trim();
        if t.is_empty() {
            return Err("template name must not be empty".to_string());
        }
        if t.contains('/') || t.contains('\\') {
            return Err(format!(
                "template name must not contain path separators (got {:?})",
                self.template
            ));
        }
        if t == ".." || t.starts_with("../") || t.ends_with("/..") || t.contains("/../") {
            return Err(format!(
                "template name must not contain '..' traversal (got {:?})",
                self.template
            ));
        }
        // Reject NUL and other control characters outright.
        if t.chars().any(|c| c.is_control()) {
            return Err(format!(
                "template name must not contain control characters (got {:?})",
                self.template
            ));
        }
        Ok(())
    }
}

/// `validate` subcommand: check a config or project file.
#[derive(Debug, Args)]
pub struct ValidateCmd {
    /// Path to validate
    pub path: PathBuf,

    /// Strict mode: warnings are errors
    #[arg(long)]
    pub strict: bool,
}

/// `version` subcommand: print version + build info.
#[derive(Debug, Args)]
pub struct VersionCmd {}

/// Helper: register all 3 common subcommands on a `clap::Command`.
///
/// This is a convenience for CLIs that build their `clap::Command`
/// imperatively (not via `#[derive(Subcommand)]`). For derived
/// subcommands, use `common_subcommands::InitCmd` etc. as variants.
pub fn add_common_subcommands(cmd: clap::Command) -> clap::Command {
    cmd.subcommand(
        clap::Command::new("init")
            .about("Initialize a new project")
            .arg(
                clap::arg!([path] "Path to initialize")
                    .default_value(".")
                    .value_parser(clap::value_parser!(PathBuf)),
            )
            .arg(clap::arg!(-f --force "Force overwrite"))
            .arg(clap::arg!(-t --template <TEMPLATE> "Template to use").default_value("default")),
    )
    .subcommand(
        clap::Command::new("validate")
            .about("Validate a config or project")
            .arg(clap::arg!([path] "Path to validate").value_parser(clap::value_parser!(PathBuf)))
            .arg(clap::arg!(--strict "Strict mode: warnings are errors")),
    )
    .subcommand(clap::Command::new("version").about("Print version + build info"))
}

/// Trait alias: CLIs with common subcommands can implement this to
/// get a uniform match arm signature.
pub trait CommonSubcommand {}
impl CommonSubcommand for InitCmd {}
impl CommonSubcommand for ValidateCmd {}
impl CommonSubcommand for VersionCmd {}

/// Convenience subcommand enum that bundles all 3 common subcommands
/// plus a free-form `Other` variant for app-specific commands.
///
/// Use this when a CLI has no app-specific subcommands and just wants
/// the 3 common ones uniformly.
#[derive(Debug, Subcommand)]
pub enum CommonCommands {
    /// Initialize a new project
    Init(InitCmd),
    /// Validate a config or project
    Validate(ValidateCmd),
    /// Print version + build info
    Version(VersionCmd),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init(template: &str) -> InitCmd {
        InitCmd {
            path: PathBuf::from("."),
            force: false,
            template: template.to_string(),
        }
    }

    #[test]
    fn init_template_accepts_safe_identifiers() {
        for t in [
            "default",
            "rust",
            "python",
            "rust-cli",
            "python.project",
            "a",
        ] {
            let cmd = init(t);
            assert!(
                cmd.validate_template().is_ok(),
                "expected {:?} to be a safe template name; got {:?}",
                t,
                cmd.validate_template()
            );
        }
    }

    #[test]
    fn init_template_rejects_path_separators() {
        // Forward slash
        assert!(init("../etc/passwd").validate_template().is_err());
        assert!(init("foo/bar").validate_template().is_err());
        assert!(init("foo/bar/baz").validate_template().is_err());
        assert!(init("/etc/passwd").validate_template().is_err());
        // Backslash (Windows-style)
        assert!(init("foo\\bar").validate_template().is_err());
        assert!(init("..\\windows\\system32").validate_template().is_err());
    }

    #[test]
    fn init_template_rejects_dotdot_traversal() {
        assert!(init("..").validate_template().is_err());
        assert!(init("../foo").validate_template().is_err());
        assert!(init("foo/..").validate_template().is_err());
        assert!(init("foo/../bar").validate_template().is_err());
    }

    #[test]
    fn init_template_rejects_empty_and_control_chars() {
        assert!(init("").validate_template().is_err());
        assert!(
            init("   ").validate_template().is_err(),
            "whitespace-only is empty after trim"
        );
        assert!(
            init("foo\nbar").validate_template().is_err(),
            "newline rejected"
        );
        assert!(
            init("foo\0bar").validate_template().is_err(),
            "NUL rejected"
        );
    }
}
