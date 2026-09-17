use clap::Parser;
use clap_ext::prelude::*;

#[derive(Debug, Parser)]
#[command(name = "basic", version, about = "Example CLI using clap-ext")]
struct Cli {
    #[command(flatten)]
    verbosity: Verbosity,

    #[command(flatten)]
    config: ConfigArg,

    #[arg(long, value_enum, default_value_t)]
    output: OutputFormat,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, clap::Subcommand)]
enum Cmd {
    /// Initialize a new project
    Init(InitCmd),
    /// Validate a config
    Validate(ValidateCmd),
    /// Print version
    Version(VersionCmd),
    /// Run a custom command
    Run {
        /// Custom input
        input: String,
    },
}

fn main() -> CliResult<()> {
    let cli = Cli::parse();
    setup_tracing(cli.verbosity.to_filter());

    match cli.cmd {
        Cmd::Init(c) => {
            tracing::info!(
                "init: path={:?} force={} template={}",
                c.path,
                c.force,
                c.template
            );
        }
        Cmd::Validate(c) => {
            tracing::info!("validate: path={:?} strict={}", c.path, c.strict);
        }
        Cmd::Version(_) => {
            println!("basic v{}", env!("CARGO_PKG_VERSION"));
        }
        Cmd::Run { input } => {
            println!("run: input={} output={}", input, cli.output);
        }
    }
    Ok(())
}
