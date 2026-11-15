//! Stale remote branch cleanup (`cargo xtask branch-cleanup`).
//!
//! The repository holds about 1,800 branches, most of them `work/*` heads of
//! pull requests that GitHub squash-merged before "Automatically delete head
//! branches" was turned on. Cloud agent sessions cannot delete refs (their Git
//! proxy refuses deletions), so `.github/workflows/branch-cleanup.yml` runs
//! this command with the workflow's own token. Without `--delete` it only
//! reports, so it is also safe to run locally.
//!
//! A branch is deleted only when every one of these holds:
//! 1. It is not protected ([`PROTECTED`], [`PROTECTED_PREFIXES`], the default
//!    branch), and no tracked file of the default-branch checkout links to it.
//! 2. No open pull request, draft or ready, uses it as its head.
//! 3. Its newest commit is older than `--min-age-hours`, unless it is one of
//!    the [`EXPLICIT`] leftovers.
//! 4. A merged or closed pull request's last head is still its tip, its tip is
//!    already in the default branch's history, it is an [`EXPLICIT`] leftover,
//!    or it has no pull request and `--include-unmerged-no-pr` was given.
//!
//! Squash merges leave a branch's commits outside main's history, so the
//! pull-request check, not ancestry, recognizes those branches. A branch that
//! gained commits after its pull request closed is kept. Each deletion carries
//! a lease on the examined commit, so a branch that moved after the scan
//! survives and is reported as a failure.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Branches that tools, workflows or GitHub depend on. The workflow's header
/// comment repeats this list; keep them in step.
const PROTECTED: &[&str] = &[
    // The default branch; every workflow and tool targets it.
    "main",
    // The claims registry written by `cargo xtask claims`.
    "work/claims",
    // The conventional Pages branch; pages.yml deploys from Actions today.
    "gh-pages",
];
/// Name prefixes GitHub manages itself.
const PROTECTED_PREFIXES: &[&str] = &["gh-readonly-queue/"];
/// Leftover probe branches at an old main commit with no new commits.
const EXPLICIT: &[&str] = &["work/claude-probe-delete-me", "claude/eager-ritchie-j2dts3"];

const USAGE: &str = "\
usage: cargo xtask branch-cleanup [--repo OWNER/NAME] [--git-dir DIR] [--checkout DIR]
         [--remote NAME] [--min-age-hours N] [--max-delete N] [--include-unmerged-no-pr]
         [--no-fetch] [--out DIR] [--delete]

Without --delete the command only reports. Reports go to DIR/summary.md and
DIR/branches.json (default DIR: branch-cleanup).";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
enum Category {
    MergedPr,
    ClosedPr,
    Ancestor,
    Explicit,
    UnmergedNoPr,
    MovedAfterPr,
    OpenPr,
    TooNew,
    Protected,
}

impl Category {
    const ALL: [Category; 9] = [
        Category::MergedPr,
        Category::ClosedPr,
        Category::Ancestor,
        Category::Explicit,
        Category::UnmergedNoPr,
        Category::MovedAfterPr,
        Category::OpenPr,
        Category::TooNew,
        Category::Protected,
    ];

    fn title(self) -> &'static str {
        match self {
            Category::MergedPr => "Merged pull request",
            Category::ClosedPr => "Closed (unmerged) pull request",
            Category::Ancestor => "Tip already reachable from the default branch",
            Category::Explicit => "Explicit leftover probe branch",
            Category::UnmergedNoPr => "Unmerged commits, no pull request",
            Category::MovedAfterPr => "Kept: commits after its closed pull request",
            Category::OpenPr => "Kept: open pull request",
            Category::TooNew => "Kept: newest commit younger than the age limit",
            Category::Protected => "Kept: protected or linked from repository files",
        }
    }
}

#[derive(Debug, Serialize)]
struct Branch {
    name: String,
    sha: String,
    commit_time: i64,
    category: Category,
    note: String,
    delete: bool,
    over_cap: bool,
    result: String,
}

/// Pull requests whose head is one branch name of this repository.
#[derive(Debug, Default)]
struct Pulls {
    open: bool,
    /// (number, merged, last head commit) of every closed pull request.
    finished: Vec<(u64, bool, String)>,
}

struct Options {
    repo: String,
    git_dir: PathBuf,
    checkout: Option<PathBuf>,
    remote: String,
    min_age_hours: f64,
    max_delete: usize,
    include_unmerged: bool,
    fetch: bool,
    out: PathBuf,
    delete: bool,
    batch: usize,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        repo: std::env::var("GITHUB_REPOSITORY").unwrap_or_else(|_| "CathedralOS/Omega".into()),
        git_dir: PathBuf::from("."),
        checkout: None,
        remote: "origin".into(),
        min_age_hours: 48.0,
        max_delete: 2000,
        include_unmerged: false,
        fetch: true,
        out: PathBuf::from("branch-cleanup"),
        delete: false,
        batch: 50,
    };
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let mut value = || {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--repo" => options.repo = value()?,
            "--git-dir" => options.git_dir = value()?.into(),
            "--checkout" => options.checkout = Some(value()?.into()),
            "--remote" => options.remote = value()?,
            "--min-age-hours" => {
                options.min_age_hours = value()?.parse().map_err(|_| "bad --min-age-hours")?
            }
            "--max-delete" => {
                options.max_delete = value()?.parse().map_err(|_| "bad --max-delete")?
            }
            "--include-unmerged-no-pr" => options.include_unmerged = true,
            "--no-fetch" => options.fetch = false,
            "--out" => options.out = value()?.into(),
            "--delete" => options.delete = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    Ok(options)
}

fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git(options: &Options, args: &[&str]) -> Result<String, String> {
    let dir = options.git_dir.to_string_lossy().into_owned();
    let mut full = vec!["-C", dir.as_str()];
    full.extend_from_slice(args);
    command("git", &full)
}

fn gh_json(path: &str) -> Result<serde_json::Value, String> {
    let text = command("gh", &["api", path])?;
    serde_json::from_str(&text).map_err(|error| format!("gh api {path}: bad JSON: {error}"))
}

fn remote_prefix(options: &Options) -> String {
    format!("refs/remotes/{}/", options.remote)
}

fn list_heads(options: &Options) -> Result<Vec<(String, String, i64)>, String> {
    let prefix = remote_prefix(options);
    let format = "--format=%(refname)%09%(objectname)%09%(committerdate:unix)";
    let text = git(options, &["for-each-ref", format, &prefix])?;
    let mut heads = Vec::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let (Some(reference), Some(sha), Some(when)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("unexpected for-each-ref line: {line}"));
        };
        let name = reference.strip_prefix(&prefix).unwrap_or(reference);
        if name != "HEAD" {
            heads.push((name.to_owned(), sha.to_owned(), when.parse().unwrap_or(0)));
        }
    }
    Ok(heads)
}

fn merged_into_default(options: &Options, default: &str) -> Result<BTreeSet<String>, String> {
    let prefix = remote_prefix(options);
    let base = format!("{prefix}{default}");
    let text = git(
        options,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "--merged",
            &base,
            &prefix,
        ],
    )?;
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .map(str::to_owned)
        .collect())
}

/// Branches that a tracked file of the default-branch checkout links to.
fn linked_branches(
    checkout: &str,
    repo: &str,
    names: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let pattern = format!(
        "github\\.com/{}/(tree|blob|commits|compare)/[^)\"' <>#?]+",
        repo.replace('.', "\\.")
    );
    // `git grep` exits 1 when nothing matches.
    let output = Command::new("git")
        .args(["-C", checkout, "grep", "-hoiIE", &pattern])
        .output()
        .map_err(|error| format!("cannot run git grep: {error}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let lowered_repo = format!("github.com/{}/", repo.to_ascii_lowercase());
    let mut linked = BTreeSet::new();
    for line in text.lines() {
        let Some(start) = line.to_ascii_lowercase().find(&lowered_repo) else {
            continue;
        };
        // Skip "<repo>/<kind>/" to reach "<branch>[/<path>]".
        let tail = &line[start + lowered_repo.len()..];
        let Some((_, after_kind)) = tail.split_once('/') else {
            continue;
        };
        let parts: Vec<&str> = after_kind.split('/').collect();
        // The branch is the longest leading run of components that names one.
        if let Some(name) = (1..=parts.len())
            .rev()
            .map(|end| parts[..end].join("/"))
            .find(|candidate| names.contains(candidate))
        {
            linked.insert(name);
        }
    }
    Ok(linked)
}

fn pull_requests(repo: &str) -> Result<BTreeMap<String, Pulls>, String> {
    let mut pulls: BTreeMap<String, Pulls> = BTreeMap::new();
    // Pages by number: the `Link` headers name `repositories/<id>/` paths,
    // which the cloud session proxy refuses.
    for page in 1.. {
        let path = format!("repos/{repo}/pulls?state=all&per_page=100&page={page}");
        let batch = gh_json(&path)?;
        let items = batch
            .as_array()
            .ok_or_else(|| format!("{path}: not a list"))?;
        for pr in items {
            let head = &pr["head"];
            // Fork pull requests do not own a branch here.
            let head_repo = head["repo"]["full_name"].as_str().unwrap_or("");
            if !head_repo.eq_ignore_ascii_case(repo) {
                continue;
            }
            let (Some(name), Some(sha), Some(number)) = (
                head["ref"].as_str(),
                head["sha"].as_str(),
                pr["number"].as_u64(),
            ) else {
                return Err(format!(
                    "{path}: pull request without head ref, sha or number"
                ));
            };
            let entry = pulls.entry(name.to_owned()).or_default();
            if pr["state"] == "open" {
                entry.open = true;
            } else {
                entry
                    .finished
                    .push((number, !pr["merged_at"].is_null(), sha.to_owned()));
            }
        }
        if items.len() < 100 {
            break;
        }
    }
    Ok(pulls)
}

struct Facts<'a> {
    protected: &'a BTreeSet<String>,
    merged: &'a BTreeSet<String>,
    pulls: &'a BTreeMap<String, Pulls>,
    cutoff: i64,
    include_unmerged: bool,
}

/// Decide one branch's category and whether it is deleted, with its note.
fn classify(name: &str, sha: &str, commit_time: i64, facts: &Facts) -> (Category, bool, String) {
    if facts.protected.contains(name) || PROTECTED_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return (Category::Protected, false, String::new());
    }
    let none = Pulls::default();
    let pulls = facts.pulls.get(name).unwrap_or(&none);
    if pulls.open {
        return (Category::OpenPr, false, String::new());
    }
    if EXPLICIT.contains(&name) {
        return (Category::Explicit, true, String::new());
    }
    if commit_time > facts.cutoff {
        return (Category::TooNew, false, String::new());
    }
    // The newest closed pull request whose last head is still this tip.
    let matching = pulls
        .finished
        .iter()
        .filter(|pr| pr.2 == sha)
        .max_by_key(|pr| pr.0);
    if let Some((number, merged, _)) = matching {
        let category = if *merged {
            Category::MergedPr
        } else {
            Category::ClosedPr
        };
        return (category, true, format!("#{number}"));
    }
    if facts.merged.contains(name) {
        return (Category::Ancestor, true, String::new());
    }
    if !pulls.finished.is_empty() {
        let mut numbers: Vec<u64> = pulls.finished.iter().map(|pr| pr.0).collect();
        numbers.sort_unstable_by(|a, b| b.cmp(a));
        let note = numbers
            .iter()
            .map(|n| format!("#{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        return (Category::MovedAfterPr, false, note);
    }
    (
        Category::UnmergedNoPr,
        facts.include_unmerged,
        String::new(),
    )
}

/// Delete in batches; each ref only if it still points at the examined commit.
fn delete_branches(options: &Options, doomed: &mut [&mut Branch]) {
    for (index, chunk) in doomed.chunks_mut(options.batch).enumerate() {
        let mut args = vec![
            "push".to_owned(),
            "--porcelain".into(),
            "--no-verify".into(),
        ];
        for branch in chunk.iter() {
            args.push(format!(
                "--force-with-lease=refs/heads/{}:{}",
                branch.name, branch.sha
            ));
        }
        args.push(options.remote.clone());
        for branch in chunk.iter() {
            args.push(format!(":refs/heads/{}", branch.name));
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(&options.git_dir)
            .args(&args)
            .output();
        let (stdout, stderr) = match &output {
            Ok(output) => (
                String::from_utf8_lossy(&output.stdout).into_owned(),
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ),
            Err(error) => (String::new(), format!("cannot run git push: {error}")),
        };
        // Porcelain lines: "<flag>\t(delete):refs/heads/<name>\t<summary>".
        let mut status = BTreeMap::new();
        for line in stdout.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            if let [flag, refs, summary, ..] = fields[..]
                && let Some((_, name)) = refs.split_once(":refs/heads/")
            {
                status.insert(name.to_owned(), (flag.to_owned(), summary.to_owned()));
            }
        }
        let fallback = stderr
            .trim()
            .lines()
            .last()
            .unwrap_or("no result")
            .to_owned();
        for branch in chunk.iter_mut() {
            branch.result = match status.get(&branch.name) {
                Some((flag, _)) if flag == "-" => "deleted".into(),
                Some((_, summary)) => format!("failed: {summary}"),
                None => format!("failed: {fallback}"),
            };
        }
        let deleted = chunk.iter().filter(|b| b.result == "deleted").count();
        println!("deleted batch {}: {deleted}/{}", index + 1, chunk.len());
    }
}

fn render(branches: &[Branch], options: &Options, default: &str, now: i64) -> String {
    let mut text = String::new();
    let mode = if options.delete {
        "deletion run"
    } else {
        "dry run"
    };
    let doomed: Vec<&Branch> = branches.iter().filter(|b| b.delete).collect();
    let capped = branches.iter().filter(|b| b.over_cap).count();
    let _ = writeln!(text, "# Branch cleanup ({mode})\n");
    let _ = writeln!(
        text,
        "Repository `{}`, default branch `{default}`, {} branches examined, minimum age {} h, \
         unmerged no-PR branches {}.\n",
        options.repo,
        branches.len(),
        options.min_age_hours,
        if options.include_unmerged {
            "included"
        } else {
            "kept"
        }
    );
    if options.delete {
        let ok = doomed.iter().filter(|b| b.result == "deleted").count();
        let failed = doomed.len() - ok;
        let _ = writeln!(
            text,
            "**Deleted {ok} of {} branches; {failed} failed.**",
            doomed.len()
        );
    } else {
        let _ = writeln!(
            text,
            "**Would delete {} branches.** Nothing was deleted.",
            doomed.len()
        );
    }
    if capped > 0 {
        let _ = writeln!(
            text,
            "\n{capped} further candidates exceeded max_delete={} and wait for a later run.",
            options.max_delete
        );
    }
    let _ = writeln!(
        text,
        "\n| Category | Deleted? | Count |\n| --- | --- | ---: |"
    );
    for category in Category::ALL {
        let deletes = match category {
            Category::MergedPr | Category::ClosedPr | Category::Ancestor | Category::Explicit => {
                true
            }
            Category::UnmergedNoPr => options.include_unmerged,
            _ => false,
        };
        let count = branches.iter().filter(|b| b.category == category).count();
        let verdict = if deletes { "yes" } else { "no" };
        let _ = writeln!(text, "| {} | {verdict} | {count} |", category.title());
    }
    text.push('\n');
    for category in Category::ALL {
        let group: Vec<&Branch> = branches.iter().filter(|b| b.category == category).collect();
        if group.is_empty() {
            continue;
        }
        let title = category.title();
        let _ = writeln!(
            text,
            "<details><summary>{title} ({})</summary>\n\n```text",
            group.len()
        );
        for branch in group {
            let days = (now - branch.commit_time) as f64 / 86_400.0;
            let _ = write!(text, "{}  {}  {days:.1}d", branch.name, &branch.sha[..12]);
            if !branch.note.is_empty() {
                let _ = write!(text, "  {}", branch.note);
            }
            if !branch.result.is_empty() {
                let _ = write!(text, "  [{}]", branch.result);
            }
            if branch.over_cap {
                text.push_str("  [over max_delete]");
            }
            text.push('\n');
        }
        text.push_str("```\n\n</details>\n\n");
    }
    let failures: Vec<&&Branch> = doomed
        .iter()
        .filter(|b| b.result.starts_with("failed"))
        .collect();
    if !failures.is_empty() {
        text.push_str("## Failures\n\n```text\n");
        for branch in failures {
            let _ = writeln!(text, "{}: {}", branch.name, branch.result);
        }
        text.push_str("```\n");
    }
    text
}

fn execute(options: &Options) -> Result<bool, String> {
    let default = gh_json(&format!("repos/{}", options.repo))?["default_branch"]
        .as_str()
        .ok_or("repository has no default_branch")?
        .to_owned();
    if options.fetch {
        let refspec = format!("+refs/heads/*:{}*", remote_prefix(options));
        git(
            options,
            &[
                "fetch",
                "--quiet",
                "--prune",
                "--filter=blob:none",
                &options.remote,
                &refspec,
            ],
        )?;
    }
    let heads = list_heads(options)?;
    let names: BTreeSet<String> = heads.iter().map(|head| head.0.clone()).collect();
    if !names.contains(&default) {
        return Err(format!(
            "default branch {default:?} was not fetched; refusing to classify"
        ));
    }
    let checkout = options.checkout.as_ref().unwrap_or(&options.git_dir);
    let linked = linked_branches(&checkout.to_string_lossy(), &options.repo, &names)?;
    let mut protected: BTreeSet<String> = PROTECTED.iter().map(|name| (*name).to_owned()).collect();
    protected.insert(default.clone());
    protected.extend(linked.iter().cloned());
    let merged = merged_into_default(options, &default)?;
    let pulls = pull_requests(&options.repo)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs() as i64;
    let facts = Facts {
        protected: &protected,
        merged: &merged,
        pulls: &pulls,
        cutoff: now - (options.min_age_hours * 3600.0) as i64,
        include_unmerged: options.include_unmerged,
    };
    let mut branches: Vec<Branch> = heads
        .into_iter()
        .map(|(name, sha, commit_time)| {
            let (category, delete, mut note) = classify(&name, &sha, commit_time, &facts);
            if category == Category::Protected && linked.contains(&name) {
                note = "linked from a repository file".into();
            }
            Branch {
                name,
                sha,
                commit_time,
                category,
                note,
                delete,
                over_cap: false,
                result: String::new(),
            }
        })
        .collect();
    branches.sort_by(|a, b| (a.category, &a.name).cmp(&(b.category, &b.name)));

    // Oldest first; candidates beyond the cap wait for the next run.
    let mut doomed: Vec<&mut Branch> = branches.iter_mut().filter(|b| b.delete).collect();
    doomed.sort_by_key(|b| b.commit_time);
    for branch in doomed.iter_mut().skip(options.max_delete) {
        branch.delete = false;
        branch.over_cap = true;
    }
    doomed.truncate(options.max_delete);
    if options.delete {
        delete_branches(options, &mut doomed);
    }

    fs::create_dir_all(&options.out).map_err(|error| error.to_string())?;
    let summary = render(&branches, options, &default, now);
    fs::write(options.out.join("summary.md"), &summary).map_err(|error| error.to_string())?;
    let json = serde_json::to_string_pretty(&branches).map_err(|error| error.to_string())?;
    fs::write(options.out.join("branches.json"), json).map_err(|error| error.to_string())?;
    println!("{}", summary.split("<details>").next().unwrap_or(""));
    Ok(branches.iter().any(|b| b.result.starts_with("failed")))
}

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    let options = parse(args).map_err(|message| {
        eprintln!("{message}");
        2
    })?;
    match execute(&options) {
        Ok(false) => Ok(()),
        Ok(true) => Err(1),
        Err(message) => {
            eprintln!("branch-cleanup: {message}");
            Err(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts_for<'a>(
        protected: &'a BTreeSet<String>,
        merged: &'a BTreeSet<String>,
        pulls: &'a BTreeMap<String, Pulls>,
    ) -> Facts<'a> {
        Facts {
            protected,
            merged,
            pulls,
            cutoff: 1_000,
            include_unmerged: false,
        }
    }

    #[test]
    fn classifies_each_rule_in_order() {
        let protected: BTreeSet<String> = ["main".to_owned()].into();
        let merged: BTreeSet<String> = ["work/ancestor".to_owned(), "main".to_owned()].into();
        let mut pulls = BTreeMap::new();
        pulls.insert(
            "work/open".into(),
            Pulls {
                open: true,
                finished: vec![(1, true, "a".into())],
            },
        );
        pulls.insert(
            "work/merged".into(),
            Pulls {
                open: false,
                finished: vec![(7, true, "a".into())],
            },
        );
        pulls.insert(
            "work/closed".into(),
            Pulls {
                open: false,
                finished: vec![(8, false, "a".into())],
            },
        );
        pulls.insert(
            "work/moved".into(),
            Pulls {
                open: false,
                finished: vec![(9, true, "old".into())],
            },
        );
        let facts = facts_for(&protected, &merged, &pulls);
        let case = |name: &str, time: i64| classify(name, "a", time, &facts);

        assert_eq!(case("main", 0).0, Category::Protected);
        assert_eq!(
            case("gh-readonly-queue/main/pr-1", 0).0,
            Category::Protected
        );
        assert_eq!(case("work/open", 0).0, Category::OpenPr);
        assert_eq!(
            case("work/claude-probe-delete-me", 5_000),
            (Category::Explicit, true, String::new())
        );
        assert_eq!(case("work/merged", 5_000).0, Category::TooNew);
        assert_eq!(
            case("work/merged", 0),
            (Category::MergedPr, true, "#7".into())
        );
        assert_eq!(
            case("work/closed", 0),
            (Category::ClosedPr, true, "#8".into())
        );
        assert_eq!(
            case("work/ancestor", 0),
            (Category::Ancestor, true, String::new())
        );
        // A branch reused after its pull request keeps its newer commits.
        assert_eq!(
            case("work/moved", 0),
            (Category::MovedAfterPr, false, "#9".into())
        );
        assert_eq!(
            case("work/orphan", 0),
            (Category::UnmergedNoPr, false, String::new())
        );
        let opted_in = Facts {
            include_unmerged: true,
            ..facts
        };
        assert!(classify("work/orphan", "a", 0, &opted_in).1);
    }
}
