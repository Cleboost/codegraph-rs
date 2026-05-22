use anyhow::Result;
use clap::{Parser, Subcommand};

/// codegraph — local-first code intelligence
#[derive(Parser, Debug)]
#[command(name = "codegraph", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Interactive multi-agent installer (default when run with no args).
    Install,
    /// Initialize .codegraph/ in the current directory and build the index.
    Init {
        #[arg(short, long)]
        index: bool,
    },
    /// Remove the .codegraph/ directory.
    Uninit,
    /// Full re-index.
    Index,
    /// Incremental sync of changed files.
    Sync,
    /// Show index health, backend, sizes.
    Status,
    /// Search nodes by name / signature / docstring.
    Query { query: String },
    /// List indexed files under a path.
    Files { path: Option<String> },
    /// Build context for a symbol or topic.
    Context { target: String },
    /// Show impact radius for a node.
    Affected { node: String },
    /// Run as MCP server over stdio.
    Serve {
        #[arg(long)]
        mcp: bool,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("codegraph=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Install) {
        Cmd::Install => todo!("installer"),
        Cmd::Init { .. } => todo!("init"),
        Cmd::Uninit => todo!("uninit"),
        Cmd::Index => todo!("index"),
        Cmd::Sync => todo!("sync"),
        Cmd::Status => todo!("status"),
        Cmd::Query { .. } => todo!("query"),
        Cmd::Files { .. } => todo!("files"),
        Cmd::Context { .. } => todo!("context"),
        Cmd::Affected { .. } => todo!("affected"),
        Cmd::Serve { .. } => todo!("mcp serve"),
    }
}
