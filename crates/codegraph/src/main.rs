use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, Subcommand};
use codegraph_db::Db;
use codegraph_extract::Orchestrator;
use codegraph_mcp::McpServer;
use std::sync::Arc;

mod watcher;

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
    /// Initialize .codegraph/ in the current directory and index immediately.
    /// Pass --no-index to skip indexing.
    Init {
        #[arg(long)]
        no_index: bool,
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

    let cmd = cli.cmd.ok_or_else(|| anyhow!("no subcommand. Try `codegraph init`"))?;
    match cmd {
        Cmd::Init { no_index } => cmd_init(&root, !no_index),
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
    }
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
    use codegraph_installer::{project_registry, DetectStatus, InstallOpts, InstallReport};
    use console::style;
    use dialoguer::{theme::ColorfulTheme, MultiSelect};

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

    // Agent setup
    let bin = std::env::current_exe()?;
    let bin = Utf8PathBuf::from_path_buf(bin)
        .map_err(|p| anyhow!("non-UTF8 bin path: {}", p.display()))?;
    let opts = InstallOpts {
        project_root: Some(root.to_path_buf()),
        global: false,
        binary_path: bin,
        home_dir: None,
    };

    let all_targets = project_registry();
    let statuses: Vec<DetectStatus> = all_targets.iter().map(|t| t.detect(&opts)).collect();

    let found_indices: Vec<usize> = statuses
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s, DetectStatus::Found))
        .map(|(i, _)| i)
        .collect();

    let already_indices: Vec<usize> = statuses
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s, DetectStatus::AlreadyConfigured))
        .map(|(i, _)| i)
        .collect();

    let not_found_indices: Vec<usize> = statuses
        .iter()
        .enumerate()
        .filter(|(_, s)| matches!(s, DetectStatus::NotFound))
        .map(|(i, _)| i)
        .collect();

    eprintln!();
    if !already_indices.is_empty() {
        eprintln!("{}", style("Already configured:").blue());
        for i in &already_indices {
            eprintln!("  {}", style(all_targets[*i].label()).blue());
        }
        eprintln!();
    }

    if !not_found_indices.is_empty() {
        eprintln!("{}", style("Not detected:").dim());
        for i in &not_found_indices {
            eprintln!("  {}", style(all_targets[*i].label()).dim());
        }
        eprintln!();
    }

    if found_indices.is_empty() {
        return Ok(());
    }

    let labels: Vec<String> = found_indices
        .iter()
        .map(|&i| all_targets[i].label().to_string())
        .collect();

    let chosen = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agents to configure (space = toggle, enter = confirm)")
        .items(&labels)
        .defaults(&vec![false; found_indices.len()])
        .interact()?;

    if chosen.is_empty() {
        return Ok(());
    }

    eprintln!();
    for pos in chosen {
        let target = &all_targets[found_indices[pos]];
        let report = target.install(&opts)?;
        match report {
            InstallReport::Installed(p) | InstallReport::Updated(p) => {
                for f in &p {
                    eprintln!("[{}] wrote {}", target.id(), f);
                }
            }
            InstallReport::Unchanged => eprintln!("[{}] unchanged", target.id()),
            InstallReport::Skipped(r) => eprintln!("[{}] skipped: {}", target.id(), r),
        }
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
