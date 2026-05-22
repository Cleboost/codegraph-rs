use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use codegraph_db::Db;
use codegraph_extract::Orchestrator;
use codegraph_mcp::McpServer;
use std::sync::Arc;

mod watcher;
mod bin_install;

const CODEGRAPH_DIR: &str = ".codegraph";
const DB_FILE: &str = "db.sqlite";

#[derive(Parser, Debug)]
#[command(name = "codegraph", version, about = "Local-first code intelligence")]
struct Cli {
    /// Workspace root (default: current dir).
    #[arg(long, global = true)]
    path: Option<Utf8PathBuf>,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Initialize .codegraph/ in the current directory.
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
    /// Show index health.
    Status,
    /// Search nodes (FTS).
    Query {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// List indexed files under a path prefix.
    Files { path: Option<String> },
    /// Build markdown context for a symbol.
    Context {
        target: String,
        #[arg(long, default_value_t = 1)]
        depth: u32,
        #[arg(long)]
        source: bool,
    },
    /// Run as MCP server over stdio.
    Serve {
        #[arg(long)]
        mcp: bool,
    },
    /// Multi-agent installer (placeholder).
    Install,
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
    let root = match &cli.path {
        Some(p) => p.clone(),
        None => Utf8PathBuf::from_path_buf(std::env::current_dir()?)
            .map_err(|p| anyhow!("non-UTF8 cwd: {}", p.display()))?,
    };

    match cli.cmd.unwrap_or(Cmd::Install) {
        Cmd::Init { index } => cmd_init(&root, index),
        Cmd::Uninit => cmd_uninit(&root),
        Cmd::Index => cmd_index(&root),
        Cmd::Sync => cmd_sync(&root),
        Cmd::Status => cmd_status(&root),
        Cmd::Query { query, limit } => cmd_query(&root, &query, limit),
        Cmd::Files { path } => cmd_files(&root, path.as_deref()),
        Cmd::Context {
            target,
            depth,
            source,
        } => cmd_context(&root, &target, depth, source),
        Cmd::Serve { mcp } => cmd_serve(&root, mcp),
        Cmd::Install => cmd_install(&root),
    }
}

fn cmd_install(root: &Utf8Path) -> Result<()> {
    use codegraph_installer::{registry, InstallOpts, InstallReport};
    
    // Self-install the binary first
    let final_bin = bin_install::ensure_installed()?;
    
    let opts = InstallOpts {
        project_root: Some(root.to_path_buf()),
        global: false,
        binary_path: final_bin,
    };
    for target in registry() {
        let status = target.detect(&opts);
        eprintln!("[{}] detected: {:?}", target.id(), status);
        let report = target.install(&opts)?;
        match report {
            InstallReport::Installed(p) | InstallReport::Updated(p) => {
                for f in p {
                    eprintln!("  wrote {}", f);
                }
            }
            InstallReport::Unchanged => eprintln!("  unchanged"),
            InstallReport::Skipped(r) => eprintln!("  skipped: {}", r),
        }
    }
    Ok(())
}

fn db_path(root: &Utf8Path) -> Utf8PathBuf {
    root.join(CODEGRAPH_DIR).join(DB_FILE)
}

fn ensure_initialized(root: &Utf8Path) -> Result<()> {
    if !db_path(root).exists() {
        return Err(anyhow!("not initialized: run `codegraph init` in {}", root));
    }
    Ok(())
}

fn cmd_init(root: &Utf8Path, do_index: bool) -> Result<()> {
    let dir = root.join(CODEGRAPH_DIR);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(".gitignore"), "*\n")?;
    std::fs::write(dir.join("version"), env!("CARGO_PKG_VERSION"))?;
    let db = Db::open(&db_path(root))?;
    eprintln!("initialized {}", dir);
    if do_index {
        let stats = Orchestrator::with_registry().index_all(root, &db)?;
        eprintln!(
            "indexed {} files, {} nodes, {} edges",
            stats.files, stats.nodes, stats.edges
        );
    }
    Ok(())
}

fn cmd_uninit(root: &Utf8Path) -> Result<()> {
    let dir = root.join(CODEGRAPH_DIR);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
        eprintln!("removed {}", dir);
    }
    Ok(())
}

fn cmd_index(root: &Utf8Path) -> Result<()> {
    ensure_initialized(root)?;
    let db = Db::open(&db_path(root))?;
    let stats = Orchestrator::with_registry().index_all(root, &db)?;
    eprintln!(
        "indexed {} files, {} nodes, {} edges (skipped {})",
        stats.files, stats.nodes, stats.edges, stats.skipped
    );
    Ok(())
}

fn cmd_sync(root: &Utf8Path) -> Result<()> {
    ensure_initialized(root)?;
    let db = Db::open(&db_path(root))?;
    let stats = Orchestrator::with_registry().sync(root, &db)?;
    eprintln!(
        "synced {} files (skipped {}), nodes={} edges={}",
        stats.files, stats.skipped, stats.nodes, stats.edges
    );
    Ok(())
}

fn cmd_status(root: &Utf8Path) -> Result<()> {
    ensure_initialized(root)?;
    let db = Db::open(&db_path(root))?;
    let s = db.stats()?;
    println!("schema: v{}", s.schema_version);
    println!("files:  {}", s.files);
    println!("nodes:  {}", s.nodes);
    println!("edges:  {}", s.edges);
    println!("size:   {} bytes", s.size_bytes);
    Ok(())
}

fn cmd_query(root: &Utf8Path, q: &str, limit: u32) -> Result<()> {
    ensure_initialized(root)?;
    let db = Db::open(&db_path(root))?;
    let hits = db.search_nodes(q, limit)?;
    for h in hits {
        println!(
            "[{}] {}  {}  {}:{}",
            h.id,
            h.kind.as_str(),
            h.name,
            h.file,
            h.start_line
        );
    }
    Ok(())
}

fn cmd_files(root: &Utf8Path, prefix: Option<&str>) -> Result<()> {
    ensure_initialized(root)?;
    let db = Db::open(&db_path(root))?;
    for f in db.files_under(prefix.unwrap_or(""))? {
        println!("{}  ({})", f.path, f.language);
    }
    Ok(())
}

fn cmd_context(root: &Utf8Path, target: &str, depth: u32, include_source: bool) -> Result<()> {
    ensure_initialized(root)?;
    let db = Db::open(&db_path(root))?;
    let req = codegraph_context::ContextRequest {
        query: target.into(),
        depth,
        include_source,
        limit: 5,
        format: codegraph_context::Format::Markdown,
    };
    print!("{}", codegraph_context::build(&db, &req)?);
    Ok(())
}

fn cmd_serve(root: &Utf8Path, mcp: bool) -> Result<()> {
    if !mcp {
        return Err(anyhow!("only --mcp transport supported"));
    }
    ensure_initialized(root).context("init the index before serving")?;
    let db = Arc::new(Db::open(&db_path(root))?);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        watcher::spawn(root.to_path_buf(), db.clone());
        McpServer::new(db).run_stdio().await
    })?;
    Ok(())
}
