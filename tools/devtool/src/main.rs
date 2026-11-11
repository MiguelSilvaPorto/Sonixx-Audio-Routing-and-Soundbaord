mod git;
mod hook;
mod worktree;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sonixx-dev",
    version,
    about = "Sonixx development workflow: worktrees, protected branches and cleanup"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Install the pre-commit hook that protects master/main.
    Init {
        /// Replace an existing pre-commit hook that was not created by sonixx-dev.
        #[arg(long)]
        force: bool,
    },
    /// Git hook entry points (called by the installed hook).
    Hook {
        #[command(subcommand)]
        command: HookCommand,
    },
    /// Worktree workflow.
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommand,
    },
}

#[derive(Subcommand)]
enum HookCommand {
    /// Block direct commits on master/main.
    PreCommit,
}

#[derive(Subcommand)]
enum WorktreeCommand {
    /// Create an isolated worktree for a task.
    Open {
        name: String,
        #[arg(long)]
        no_fetch: bool,
        #[arg(long)]
        reopen: bool,
    },
    /// Commit, integrate origin and publish the task.
    Close {
        #[arg(short, long)]
        message: Option<String>,
        #[arg(long)]
        no_push: bool,
        #[arg(long)]
        keep: bool,
        #[arg(long)]
        skip_validation: bool,
    },
    /// Remove a worktree (and optionally its local branch).
    Clean {
        name: String,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        delete_branch: bool,
        #[arg(long)]
        wait: bool,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// Show worktrees, origin/upstream position and pending cleanup.
    Status,
    /// List git worktrees.
    List,
    /// Print the Pull Request URL (human action).
    Pr,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { force } => hook::init(force),
        Commands::Hook { command } => match command {
            HookCommand::PreCommit => {
                if hook::pre_commit()? {
                    Ok(())
                } else {
                    std::process::exit(1);
                }
            }
        },
        Commands::Worktree { command } => match command {
            WorktreeCommand::Open {
                name,
                no_fetch,
                reopen,
            } => worktree::open(&name, no_fetch, reopen),
            WorktreeCommand::Close {
                message,
                no_push,
                keep,
                skip_validation,
            } => worktree::close(message, no_push, keep, skip_validation),
            WorktreeCommand::Clean {
                name,
                force,
                delete_branch,
                wait,
                repo,
            } => worktree::clean(&name, force, delete_branch, wait, repo),
            WorktreeCommand::Status => worktree::status(),
            WorktreeCommand::List => worktree::list(),
            WorktreeCommand::Pr => worktree::pr(),
        },
    }
}
