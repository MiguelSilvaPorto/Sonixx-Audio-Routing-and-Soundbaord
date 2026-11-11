use crate::git::{self, Git};
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PREFIXES: [&str; 7] = ["feat", "fix", "docs", "refactor", "chore", "test", "perf"];

pub fn normalize_branch(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("branch name cannot be empty");
    }
    let branch = if name.contains('/') {
        name.to_string()
    } else {
        format!("feat/{name}")
    };
    let (prefix, rest) = branch
        .split_once('/')
        .with_context(|| format!("invalid branch name: {branch}"))?;
    if !PREFIXES.contains(&prefix) {
        bail!(
            "invalid prefix '{}' in '{}'. Use one of: {}",
            prefix,
            branch,
            PREFIXES.join(", ")
        );
    }
    if rest.is_empty()
        || !rest
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
        || rest.starts_with('.')
        || rest.starts_with('-')
        || rest.ends_with('.')
        || rest.ends_with('-')
        || rest.contains("..")
    {
        bail!(
            "invalid branch name '{}': use lowercase letters, digits, '.', '_' or '-', without leading/trailing separators",
            branch
        );
    }
    Ok(branch)
}

pub fn slug(branch: &str) -> String {
    branch.replace('/', "-")
}

fn ensure_clean(git: &Git) -> Result<()> {
    let status = git.stdout(&["status", "--porcelain"])?;
    let dirty: Vec<&str> = status
        .lines()
        .filter(|line| {
            let line = line.trim_end();
            !line.is_empty()
                && !line.starts_with("?? .worktrees/")
                && !line.starts_with("?? .worktrees\\")
        })
        .collect();
    if dirty.is_empty() {
        Ok(())
    } else {
        bail!(
            "the main copy has uncommitted changes. Resolve them first (a worktree is born from a clean main):\n{}",
            dirty.join("\n")
        )
    }
}

pub fn open(name: &str, no_fetch: bool, reopen: bool) -> Result<()> {
    let repo = git::discover()?;
    if git::is_inside_worktrees(&repo) {
        bail!(
            "you are INSIDE a worktree ({}). Work there, or run this command outside it.",
            repo.display()
        );
    }
    let principal = git::principal_of(&repo);
    let git = Git::new(&principal);
    let branch = normalize_branch(name)?;
    let dest = principal.join(".worktrees").join(slug(&branch));
    if dest.exists() {
        bail!("a worktree already exists at: {}", dest.display());
    }

    let protected = git::protected_branch(&git)?;
    ensure_clean(&git)?;

    if !no_fetch {
        println!("Fetching origin/{protected}...");
        git.run(&["fetch", "origin", protected.as_str(), "--quiet"])?;
    }
    let base = format!("origin/{protected}");

    fs::create_dir_all(principal.join(".worktrees"))
        .context("failed to create .worktrees folder")?;

    let refname = format!("refs/heads/{branch}");
    let branch_exists = git
        .try_run(&["rev-parse", "--verify", "--quiet", refname.as_str()])?
        .is_some();

    let dest_text = dest.to_string_lossy().to_string();
    if reopen {
        if !branch_exists {
            println!("Branch '{branch}' does not exist locally; fetching from origin...");
            let fetch_ref = format!("{branch}:refs/heads/{branch}");
            git.run(&["fetch", "origin", fetch_ref.as_str(), "--quiet"])?;
        }
        git.run(&["worktree", "add", dest_text.as_str(), branch.as_str()])?;
    } else {
        if branch_exists {
            bail!("branch '{branch}' already exists locally. Use --reopen to attach it to a worktree.");
        }
        let remote = git.run(&["ls-remote", "--heads", "origin", branch.as_str()])?;
        if !remote.trim().is_empty() {
            bail!(
                "branch '{branch}' already exists on origin. Choose another name or use --reopen."
            );
        }
        git.run(&[
            "worktree",
            "add",
            "-b",
            branch.as_str(),
            dest_text.as_str(),
            base.as_str(),
        ])?;
        let _ = git.try_run(&["branch", "--unset-upstream", branch.as_str()])?;
    }

    let commit = git.run(&["rev-parse", "--short", base.as_str()])?;
    println!();
    println!("OK - worktree ready:");
    println!("  branch : {branch}");
    println!("  folder : {}", dest.display());
    println!("  base   : {base} (commit {commit})");
    println!();
    println!("Work INSIDE that folder (never in the main copy).");
    println!("Validate: cargo fmt --all -- --check ; cargo clippy --all-targets -- -D warnings ; cargo test");
    println!("To close the task, from inside the worktree:");
    println!("  cargo run -p sonixx-dev -- worktree close -m \"feat(scope): description\"");
    Ok(())
}

fn run_cargo(dir: &Path, args: &[&str]) -> Result<()> {
    println!("  cargo {}", args.join(" "));
    let out = Command::new("cargo")
        .current_dir(dir)
        .args(args)
        .output()
        .context("failed to run cargo")?;
    if !out.status.success() {
        bail!(
            "cargo {} failed:\n{}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

fn conventional(message: &str) -> bool {
    let Some((prefix, rest)) = message.split_once(':') else {
        return false;
    };
    let scope_ok = if let Some((kind, scope)) = prefix.split_once('(') {
        let Some(scope) = scope.strip_suffix(')') else {
            return false;
        };
        PREFIXES.contains(&kind)
            && !scope.is_empty()
            && scope.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            })
    } else {
        PREFIXES.contains(&prefix)
    };
    scope_ok && !rest.trim().is_empty()
}

pub fn close(
    message: Option<String>,
    no_push: bool,
    keep: bool,
    skip_validation: bool,
) -> Result<()> {
    let root = git::discover()?;
    if !git::is_inside_worktrees(&root) {
        bail!(
            "this command runs FROM INSIDE a worktree (.worktrees/...). Current folder: {}",
            root.display()
        );
    }
    let principal = git::principal_of(&root);
    let git = Git::new(&root);
    let principal_git = Git::new(&principal);
    let branch = git::current_branch(&git)?;
    let protected = git::protected_branch(&principal_git)?;
    if branch == protected {
        bail!("the current branch is '{protected}' - that must not happen inside a worktree");
    }

    let conflicts = git.run(&["diff", "--name-only", "--diff-filter=U"])?;
    let conflicts = conflicts.trim();
    if !conflicts.is_empty() {
        bail!(
            "there are UNRESOLVED conflicts in this worktree:\n{}\nResolve them, run 'git add', then run close again.",
            conflicts
        );
    }

    if !skip_validation && root.join("Cargo.toml").exists() {
        println!("Validating Rust (cargo fmt, clippy, test)...");
        run_cargo(&root, &["fmt", "--all", "--", "--check"])?;
        run_cargo(
            &root,
            &[
                "clippy",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
        )?;
        run_cargo(&root, &["test", "--all-targets"])?;
        println!("Rust validation passed.");
    }

    let status = git.stdout(&["status", "--porcelain"])?;
    let merge_in_progress = git
        .try_run(&["rev-parse", "--verify", "--quiet", "MERGE_HEAD"])?
        .is_some();

    if !status.trim().is_empty() {
        let msg = match message {
            Some(message) => message,
            None if merge_in_progress => format!("merge: integra a {protected}"),
            None => bail!(
                "there are uncommitted changes. Run again with a message:\n  worktree close -m \"feat(scope): description\""
            ),
        };
        if !conventional(&msg) {
            eprintln!(
                "warning: message does not follow Conventional Commits (committing anyway): {msg}"
            );
        }
        git.run(&["add", "-A"])?;
        git.run(&["commit", "-m", msg.as_str()])?;
        println!("Commit created: {msg}");
    } else {
        println!("Nothing pending to commit.");
    }

    let origin_ref = format!("origin/{protected}");
    let mut published = false;
    for attempt in 1..=2 {
        git.run(&["fetch", "origin", protected.as_str(), "--quiet"])?;

        let (is_ancestor, _, _) =
            git.allow_fail(&["merge-base", "--is-ancestor", origin_ref.as_str(), "HEAD"])?;
        if !is_ancestor {
            println!("{origin_ref} moved - integrating (git merge {origin_ref})...");
            let merge_msg = format!("merge: integra a {protected}");
            let (merged, _, stderr) =
                git.allow_fail(&["merge", origin_ref.as_str(), "-m", merge_msg.as_str()])?;
            if !merged {
                let conflicts = git
                    .run(&["diff", "--name-only", "--diff-filter=U"])
                    .unwrap_or_default();
                bail!(
                    "CONFLICT while integrating {origin_ref} - the merge is IN PROGRESS in this worktree.\nConflicted files:\n{}\nResolve them, run 'git add', then run close again. Nothing was published.\n{}",
                    conflicts.trim(),
                    stderr.trim()
                );
            }
            println!("{origin_ref} integrated.");
        }

        if no_push {
            println!("Push skipped (--no-push).");
            published = true;
            break;
        }

        let push_ref = format!("HEAD:{protected}");
        let (pushed, stdout, stderr) = git.allow_fail(&["push", "origin", push_ref.as_str()])?;
        if pushed {
            published = true;
            println!("Commits published to {origin_ref}.");
            for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
                println!("  {line}");
            }
            break;
        }
        if attempt == 2 {
            bail!("git push to {origin_ref} failed:\n{}{}", stdout, stderr);
        }
        println!("Push rejected ({origin_ref} moved again) - integrating and publishing again...");
    }

    println!();
    println!("Last commits of the task:");
    for line in git.run(&["log", "--oneline", "-3"])?.lines() {
        println!("  {line}");
    }

    if !no_push {
        println!();
        println!("=== NEXT STEP (HUMAN): Pull Request ===");
        println!("  The push went to {origin_ref} - no new remote branch was created.");
        println!("  If a PR from your fork's {protected} already exists, these commits are already in it.");
        println!("  Otherwise open it when you want: cargo run -p sonixx-dev -- worktree pr");
        println!("  (the AI NEVER opens the PR)");
    }

    println!();
    if published && !no_push && !keep {
        match git::installed_devtool(&principal) {
            Ok(exe) if exe.exists() => {
                spawn_cleanup(&exe, &principal, &branch)?;
            }
            _ => {
                println!("Automatic cleanup is not installed (run 'sonixx-dev init' from the main copy).");
                println!("To clean the worktree manually:");
                println!("  cargo run -p sonixx-dev -- worktree clean {branch}");
            }
        }
    } else {
        println!("To clean the worktree (from the main repository):");
        println!("  cargo run -p sonixx-dev -- worktree clean {branch}");
    }
    Ok(())
}

fn spawn_cleanup(exe: &Path, principal: &Path, branch: &str) -> Result<()> {
    let mut cmd = Command::new(exe);
    cmd.arg("worktree")
        .arg("clean")
        .arg("--repo")
        .arg(principal)
        .arg("--wait")
        .arg("--delete-branch")
        .arg(branch)
        .current_dir(principal);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    let child = cmd
        .spawn()
        .context("failed to start the background cleanup process")?;
    println!(
        "Cleanup scheduled in the background (pid {}). Log: .worktrees/limpeza-automatica.log",
        child.id()
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct WorktreeEntry {
    path: PathBuf,
    branch: Option<String>,
}

fn parse_worktrees(git: &Git) -> Result<Vec<WorktreeEntry>> {
    let text = git.stdout(&["worktree", "list", "--porcelain"])?;
    let mut entries = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(previous) = current_path.take() {
                entries.push(WorktreeEntry {
                    path: previous,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(PathBuf::from(path.trim()));
            current_branch = None;
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = Some(branch.trim().trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(path) = current_path {
        entries.push(WorktreeEntry {
            path,
            branch: current_branch,
        });
    }
    Ok(entries)
}

fn resolve_destination(principal: &Path, name: &str) -> PathBuf {
    let root = principal.join(".worktrees");
    if name.contains('/') {
        return root.join(slug(name));
    }
    let exact = root.join(name);
    if exact.exists() {
        exact
    } else {
        root.join(format!("feat-{name}"))
    }
}

fn resolve_branch_for_path(git: &Git, path: &Path) -> Result<Option<String>> {
    for entry in parse_worktrees(git)? {
        if git::same_path(&entry.path, path) {
            return Ok(entry.branch);
        }
    }
    Ok(None)
}

fn remove_reparse_points(dir: &Path) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !git::is_reparse_point(&path) {
            continue;
        }
        let is_dir = fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
        if is_dir {
            let _ = fs::remove_dir(&path);
        } else {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

fn log_cleanup(principal: &Path, message: &str) {
    let dir = principal.join(".worktrees");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("limpeza-automatica.log");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{stamp}] {message}");
    }
}

pub fn clean(
    name: &str,
    force: bool,
    delete_branch: bool,
    wait: bool,
    repo: Option<PathBuf>,
) -> Result<()> {
    let repo = match repo {
        Some(path) => path,
        None => git::discover()?,
    };
    let principal = git::principal_of(&repo);
    let git = Git::new(&principal);
    let dest = resolve_destination(&principal, name);

    if !dest.exists() {
        let _ = git.try_run(&["worktree", "prune"])?;
        println!("No worktree at: {}", dest.display());
        return Ok(());
    }

    let cwd = std::env::current_dir().context("cannot read the current folder")?;
    if git::is_inside(&cwd, &dest) {
        bail!(
            "cannot remove a worktree while the current folder is inside it ({}).",
            dest.display()
        );
    }

    let branch = resolve_branch_for_path(&git, &dest)?.or_else(|| {
        if name.contains('/') {
            Some(name.to_string())
        } else {
            None
        }
    });
    let attempts = if wait { 60 } else { 1 };
    let dest_text = dest.to_string_lossy().to_string();
    let mut last_error = String::new();

    for attempt in 1..=attempts {
        remove_reparse_points(&dest)?;
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(dest_text.as_str());
        let (removed, stdout, stderr) = git.allow_fail(&args)?;
        if removed {
            println!("Worktree removed: {}", dest.display());
            if delete_branch {
                if let Some(branch) = branch.as_deref() {
                    let mut deleted = false;
                    let mut last_error = String::new();
                    for attempt in 1..=10 {
                        let (ok, _, err) = git.allow_fail(&["branch", "-D", branch])?;
                        if ok {
                            deleted = true;
                            break;
                        }
                        last_error = err;
                        if attempt < 10 {
                            sleep(Duration::from_millis(300));
                        }
                    }
                    if deleted {
                        println!("Local branch deleted: {branch}");
                        log_cleanup(&principal, &format!("OK {branch}: local branch deleted"));
                    } else {
                        println!("Could not delete local branch '{branch}': {last_error}");
                        log_cleanup(
                            &principal,
                            &format!(
                                "PENDING {branch}: could not delete local branch - {last_error}"
                            ),
                        );
                    }
                }
            }
            log_cleanup(&principal, &format!("OK {name}: worktree removed"));
            return Ok(());
        }
        last_error = format!("{}{}", stdout.trim(), stderr.trim());
        if attempt < attempts {
            sleep(Duration::from_secs(2));
        }
    }

    log_cleanup(
        &principal,
        &format!("PENDING {name}: could not remove worktree - {last_error}"),
    );
    bail!(
        "could not remove {} after {attempts} attempt(s). A terminal/editor may be holding the folder.\n{}",
        dest.display(),
        last_error.trim()
    )
}

pub fn list() -> Result<()> {
    let repo = git::discover()?;
    let principal = git::principal_of(&repo);
    let git = Git::new(&principal);
    println!("{}", git.run(&["worktree", "list"])?);
    Ok(())
}

pub fn status() -> Result<()> {
    let repo = git::discover()?;
    let principal = git::principal_of(&repo);
    let git = Git::new(&principal);
    let protected = git::current_branch(&git)
        .ok()
        .filter(|branch| branch == "master" || branch == "main")
        .unwrap_or_else(|| "master".to_string());

    println!("== Worktrees ==");
    let entries = parse_worktrees(&git)?;
    for entry in &entries {
        println!(
            "  {:<12} {}",
            entry.branch.as_deref().unwrap_or("(detached)"),
            entry.path.display()
        );
    }

    println!();
    println!("== origin/{protected} vs upstream/{protected} ==");
    let _ = git.allow_fail(&["fetch", "origin", "--quiet"])?;
    let has_upstream = git.try_run(&["remote", "get-url", "upstream"])?.is_some();
    if has_upstream {
        let _ = git.allow_fail(&["fetch", "upstream", "--quiet"])?;
        let range = format!("upstream/{protected}...origin/{protected}");
        if let Some(counts) =
            git.try_run(&["rev-list", "--left-right", "--count", range.as_str()])?
        {
            let parts: Vec<&str> = counts.split_whitespace().collect();
            if parts.len() == 2 {
                println!(
                    "  origin/{protected} is {} commit(s) ahead and {} behind upstream/{protected}",
                    parts[1], parts[0]
                );
                println!("  (the PR to upstream comes from this branch; a human opens/merges it)");
            }
        }
    } else {
        println!("  (no upstream remote configured)");
    }

    println!();
    println!("== Local task branches ==");
    let branches = git.stdout(&[
        "branch",
        "--list",
        "feat/*",
        "fix/*",
        "docs/*",
        "refactor/*",
        "chore/*",
        "test/*",
        "perf/*",
    ])?;
    if branches.trim().is_empty() {
        println!("  (none)");
    } else {
        for line in branches.lines().filter(|line| !line.trim().is_empty()) {
            println!("  {line}");
        }
        println!("  (local worktree branches - they do not exist on origin; pushes go to origin/{protected})");
    }

    println!();
    println!("== Pending cleanup ==");
    let origin_ref = format!("origin/{protected}");
    let mut pending = Vec::new();
    for entry in &entries {
        if git::same_path(&entry.path, &principal) || !entry.path.exists() {
            continue;
        }
        let worktree_git = Git::new(&entry.path);
        let dirty = worktree_git
            .stdout(&["status", "--porcelain"])
            .unwrap_or_default();
        let ancestor = worktree_git
            .allow_fail(&["merge-base", "--is-ancestor", "HEAD", origin_ref.as_str()])
            .map(|(ok, _, _)| ok)
            .unwrap_or(false);
        if dirty.trim().is_empty() && ancestor {
            pending.push(format!(
                "  CAN CLEAN : {:<24} (published and clean)",
                entry.branch.as_deref().unwrap_or("(detached)")
            ));
        }
    }
    if let Ok(entries_fs) = fs::read_dir(principal.join(".worktrees")) {
        for entry_fs in entries_fs.flatten() {
            let path = entry_fs.path();
            if !path.is_dir() {
                continue;
            }
            let registered = entries
                .iter()
                .any(|entry| git::same_path(&entry.path, &path));
            if !registered {
                pending.push(format!(
                    "  GHOST     : {:<24} (folder without a git worktree record)",
                    entry_fs.file_name().to_string_lossy()
                ));
            }
        }
    }
    if pending.is_empty() {
        println!("  (nothing pending - everything clean)");
    } else {
        for line in pending {
            println!("{line}");
        }
    }
    Ok(())
}

fn github_slug(remote: &str) -> Option<(String, String)> {
    let remote = remote.trim().trim_end_matches('/');
    let rest = remote
        .strip_prefix("https://github.com/")
        .or_else(|| remote.strip_prefix("http://github.com/"))
        .or_else(|| remote.strip_prefix("git@github.com:"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repo) = rest.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

pub fn pr() -> Result<()> {
    let repo = git::discover()?;
    let principal = git::principal_of(&repo);
    let git = Git::new(&principal);
    let protected = git::protected_branch(&git)?;
    let origin = git
        .try_run(&["remote", "get-url", "origin"])?
        .unwrap_or_default();
    let upstream = git
        .try_run(&["remote", "get-url", "upstream"])?
        .unwrap_or_default();
    let (origin_owner, _) =
        github_slug(&origin).with_context(|| format!("origin is not a GitHub remote: {origin}"))?;
    let (upstream_owner, upstream_repo) = github_slug(&upstream)
        .with_context(|| format!("upstream is not a GitHub remote: {upstream}"))?;
    println!(
        "https://github.com/{upstream_owner}/{upstream_repo}/compare/{protected}...{origin_owner}:{protected}?expand=1"
    );
    println!();
    println!("Open this URL in a browser as a HUMAN action. The AI never opens or merges the PR.");
    Ok(())
}
