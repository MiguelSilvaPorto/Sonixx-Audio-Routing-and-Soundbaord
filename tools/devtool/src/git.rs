use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Small wrapper around `git -C <repo>`.
pub struct Git {
    repo: PathBuf,
}

impl Git {
    pub fn new(repo: impl Into<PathBuf>) -> Self {
        Self { repo: repo.into() }
    }

    pub fn repo(&self) -> &Path {
        &self.repo
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&self.repo);
        cmd
    }

    pub fn output(&self, args: &[&str]) -> Result<Output> {
        self.command()
            .args(args)
            .output()
            .with_context(|| format!("failed to run git {:?}", args))
    }

    /// Runs git, fails on a non-zero exit code, returns trimmed stdout.
    pub fn run(&self, args: &[&str]) -> Result<String> {
        let out = self.output(args)?;
        if !out.status.success() {
            bail!(
                "git {:?} failed:\n{}{}",
                args,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Runs git and returns stdout without trimming (useful for porcelain output).
    pub fn stdout(&self, args: &[&str]) -> Result<String> {
        let out = self.output(args)?;
        if !out.status.success() {
            bail!(
                "git {:?} failed:\n{}{}",
                args,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// Runs git; returns `None` when it fails (used for "does this ref exist?").
    pub fn try_run(&self, args: &[&str]) -> Result<Option<String>> {
        let out = self.output(args)?;
        if out.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&out.stdout).trim().to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    /// Runs git without failing; returns (success, stdout, stderr).
    pub fn allow_fail(&self, args: &[&str]) -> Result<(bool, String, String)> {
        let out = self.output(args)?;
        Ok((
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        ))
    }
}

pub fn discover() -> Result<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git (is Git installed and on PATH?)")?;
    if !out.status.success() {
        bail!(
            "not inside a git repository:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}

/// `A:\repo\.worktrees\feat-x` -> `A:\repo`
pub fn principal_of(repo: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in repo.components() {
        if component.as_os_str().to_string_lossy() == ".worktrees" {
            break;
        }
        out.push(component.as_os_str());
    }
    if out.as_os_str().is_empty() {
        repo.to_path_buf()
    } else {
        out
    }
}

pub fn is_inside_worktrees(repo: &Path) -> bool {
    repo.components()
        .any(|c| c.as_os_str().to_string_lossy() == ".worktrees")
}

pub fn current_branch(git: &Git) -> Result<String> {
    match git.try_run(&["symbolic-ref", "--short", "HEAD"])? {
        Some(branch) if !branch.is_empty() => Ok(branch),
        _ => bail!("HEAD is detached; the worktree flow needs a branch"),
    }
}

/// The main copy must stay on master (or main). Returns that branch name.
pub fn protected_branch(git: &Git) -> Result<String> {
    let branch = current_branch(git)?;
    if branch == "master" || branch == "main" {
        Ok(branch)
    } else {
        bail!(
            "the main copy is on branch '{}' - it must stay on master/main. Fix that first.",
            branch
        )
    }
}

pub fn git_common_dir(git: &Git) -> Result<PathBuf> {
    let raw = git.run(&["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(git.repo().join(path))
    }
}

/// Installed copy of this CLI, kept outside worktrees so cleanup can run after
/// the worktree folder is removed.
pub fn installed_devtool(principal: &Path) -> Result<PathBuf> {
    let common = git_common_dir(&Git::new(principal))?;
    let name = if cfg!(windows) {
        "sonixx-dev.exe"
    } else {
        "sonixx-dev"
    };
    Ok(common.join(name))
}

pub fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    path_key(a) == path_key(b)
}

pub fn is_reparse_point(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// True when `path` is inside `root` (or equal to it).
pub fn is_inside(path: &Path, root: &Path) -> bool {
    let a = path_key(path);
    let b = path_key(root);
    a == b || a.starts_with(&(b + "/"))
}
