//! Observe, enqueue, or land a pull request through GitHub's native merge queue (`cargo xtask land`).

use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::repo_root;

/// Every check branch protection requires before the merge queue takes a PR.
/// Looking at only the first let `land` queue a PR whose second check had never
/// run, and GitHub's refusal did not say why.
const REQUIRED_CHECKS: &[&str] = &["compiler-check", "workspace-clippy"];
const CHECK_POLL_SECONDS: u64 = 15;
/// `land` waits this long for the merge, so "queued" is not read as "landed".
const LAND_WAIT_SECONDS: u64 = 1800;

/// The verdict for a commit from its check runs: `green`, `red`, `pending`
/// (a required check is still running) or `missing` (one never started).
fn required_checks_verdict(nodes: &[Value], head: &str) -> (String, String) {
    let mut running = Vec::new();
    let mut missing = Vec::new();
    for required in REQUIRED_CHECKS {
        let latest = nodes
            .iter()
            .filter(|n| {
                n["__typename"].as_str() == Some("CheckRun") && n["name"].as_str() == Some(required)
            })
            .max_by_key(|n| n["startedAt"].as_str().unwrap_or(""));
        let Some(run) = latest else {
            missing.push(*required);
            continue;
        };
        let status = run["status"].as_str().unwrap_or("");
        if status != "COMPLETED" {
            running.push(format!("{required} is {}", status.to_ascii_lowercase()));
            continue;
        }
        let conclusion = run["conclusion"].as_str().unwrap_or("");
        if conclusion != "SUCCESS" {
            return (
                "red".to_owned(),
                format!("{required} concluded {}", conclusion.to_ascii_lowercase()),
            );
        }
    }
    if !missing.is_empty() {
        return (
            "missing".to_owned(),
            format!(
                "{} has not run on {head}. Both checks start on a push to the PR branch and when the base changes to main; a description edit starts only `compiler-check`. If one of those happened a moment ago, run this again in a minute; otherwise push the branch again.",
                missing.join(" and ")
            ),
        );
    }
    if !running.is_empty() {
        return ("pending".to_owned(), running.join("; "));
    }
    (
        "green".to_owned(),
        "every required check succeeded".to_owned(),
    )
}

fn run_cmd(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new(args[0])
        .args(&args[1..])
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", args[0]))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let out_str = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        return Err(if !err.is_empty() {
            err
        } else if !out_str.is_empty() {
            out_str
        } else {
            format!("{} exited {}", args[0], out.status.code().unwrap_or(1))
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn repository_slug(root: &Path, remote: &str) -> Result<(String, String), String> {
    let out = run_cmd(
        root,
        &["git", "remote", "get-url", "--all", "--push", remote],
    )?;
    let urls: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    if urls.len() != 1 {
        return Err("Landing requires one explicit GitHub push destination.".to_owned());
    }
    let url = urls[0]
        .strip_prefix("https://github.com/")
        .or_else(|| urls[0].strip_prefix("git@github.com:"))
        .ok_or_else(|| {
            "The push destination must be an HTTPS or SSH github.com repository.".to_owned()
        })?;
    let slug = url.trim_end_matches('/').trim_end_matches(".git");
    let (owner, name) = slug
        .split_once('/')
        .ok_or_else(|| "Invalid GitHub repository slug.".to_owned())?;
    Ok((owner.to_owned(), name.to_owned()))
}

fn graphql(root: &Path, query: &str, vars: &[(&str, &str, bool)]) -> Result<Value, String> {
    let mut cmd_args = vec![
        "gh".to_owned(),
        "api".to_owned(),
        "graphql".to_owned(),
        "-f".to_owned(),
        format!("query={query}"),
    ];
    for (k, v, is_int) in vars {
        cmd_args.push(if *is_int { "-F" } else { "-f" }.to_owned());
        cmd_args.push(format!("{k}={v}"));
    }
    let refs: Vec<&str> = cmd_args.iter().map(String::as_str).collect();
    let out = run_cmd(root, &refs)?;
    let parsed: Value = serde_json::from_str(&out).map_err(|e| e.to_string())?;
    if let Some(errors) = parsed.get("errors") {
        return Err(errors.to_string());
    }
    parsed
        .get("data")
        .cloned()
        .ok_or_else(|| "GraphQL response missing data".to_owned())
}

fn pull_request(root: &Path, remote: &str, number: u64) -> Result<Value, String> {
    let (owner, name) = repository_slug(root, remote)?;
    let num_str = number.to_string();
    let query = "query($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){pullRequest(number:$number){id number url state isDraft baseRefName headRefOid mergeStateStatus mergeable mergeCommit{oid} mergeQueueEntry{position state} autoMergeRequest{enabledAt}}}}";
    let data = graphql(
        root,
        query,
        &[
            ("owner", &owner, false),
            ("name", &name, false),
            ("number", &num_str, true),
        ],
    )?;
    let pr = data["repository"]["pullRequest"].clone();
    if pr.is_null() {
        return Err(format!("Pull request {number} does not exist."));
    }
    Ok(pr)
}

fn queue_status(root: &Path, remote: &str) -> Result<Value, String> {
    let (owner, name) = repository_slug(root, remote)?;
    let query = "query($owner:String!,$name:String!){repository(owner:$owner,name:$name){mergeQueue(branch:\"main\"){entries(first:100){pageInfo{hasNextPage} nodes{position state pullRequest{number url title headRefOid}}}}}}";
    let data = graphql(
        root,
        query,
        &[("owner", &owner, false), ("name", &name, false)],
    )?;
    Ok(data["repository"]["mergeQueue"].clone())
}

fn head_check(root: &Path, remote: &str, head: &str) -> Result<(String, String), String> {
    let (owner, name) = repository_slug(root, remote)?;
    let query = "query($owner:String!,$name:String!,$head:GitObjectID!){repository(owner:$owner,name:$name){object(oid:$head){... on Commit{statusCheckRollup{contexts(first:100){nodes{__typename ... on CheckRun{name status conclusion startedAt}}}}}}}}";
    let data = graphql(
        root,
        query,
        &[
            ("owner", &owner, false),
            ("name", &name, false),
            ("head", head, false),
        ],
    )?;
    let nodes = data["repository"]["object"]["statusCheckRollup"]["contexts"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(required_checks_verdict(&nodes, head))
}

fn validate_head(pr: &Value, expected: &str) -> Result<(), String> {
    if pr["baseRefName"].as_str() != Some("main") {
        return Err("This command only submits PRs targeting main.".to_owned());
    }
    if expected.len() != 40
        || !expected.bytes().all(|b| b.is_ascii_hexdigit())
        || pr["headRefOid"].as_str() != Some(expected)
    {
        return Err(
            "PR source head differs from --expected-head; inspect the actual revision.".to_owned(),
        );
    }
    Ok(())
}

fn enqueue(root: &Path, remote: &str, number: u64, expected: &str) -> Result<Value, String> {
    let pr = pull_request(root, remote, number)?;
    validate_head(&pr, expected)?;
    if pr["state"].as_str() == Some("MERGED") || !pr["mergeQueueEntry"].is_null() {
        return Ok(pr);
    }
    if pr["state"].as_str() != Some("OPEN") {
        return Err("Cannot enqueue a closed PR.".to_owned());
    }
    let pr_id = pr["id"].as_str().unwrap_or("");
    let query = "mutation($pullRequest:ID!,$head:GitObjectID!){enqueuePullRequest(input:{pullRequestId:$pullRequest,expectedHeadOid:$head}){mergeQueueEntry{position state}}}";
    graphql(
        root,
        query,
        &[("pullRequest", pr_id, false), ("head", expected, false)],
    )
}

fn arm(root: &Path, remote: &str, number: u64, expected: &str) -> Result<Value, String> {
    let pr = pull_request(root, remote, number)?;
    validate_head(&pr, expected)?;
    if pr["state"].as_str() == Some("MERGED") || !pr["mergeQueueEntry"].is_null() {
        return Ok(pr);
    }
    if pr["state"].as_str() != Some("OPEN") {
        return Err(format!("PR {number} was closed without merging."));
    }
    if pr["mergeStateStatus"].as_str() == Some("DIRTY")
        || pr["mergeable"].as_str() == Some("CONFLICTING")
    {
        return Err(format!(
            "PR {number} has a merge conflict with main; resolve it and push."
        ));
    }
    let (verdict, description) = head_check(root, remote, expected)?;
    if verdict == "red" {
        return Err(format!(
            "{description} on {expected}; inspect the failed run before changing source."
        ));
    }
    // Arming auto-merge here would wait forever for a check nothing will start.
    if verdict == "missing" {
        return Err(description);
    }
    if verdict == "pending" && !pr["autoMergeRequest"].is_null() {
        return Ok(pr);
    }
    if pr["isDraft"].as_bool() == Some(true) {
        let (owner, name) = repository_slug(root, remote)?;
        let slug = format!("{owner}/{name}");
        let num_str = number.to_string();
        run_cmd(root, &["gh", "pr", "ready", &num_str, "--repo", &slug])?;
    }
    if verdict == "green" {
        let _ = enqueue(root, remote, number, expected)?;
    } else {
        let pr_id = pr["id"].as_str().unwrap_or("");
        let query = "mutation($pullRequest:ID!,$head:GitObjectID!){enablePullRequestAutoMerge(input:{pullRequestId:$pullRequest,expectedHeadOid:$head}){pullRequest{autoMergeRequest{enabledAt} mergeQueueEntry{position state}}}}";
        if graphql(
            root,
            query,
            &[("pullRequest", pr_id, false), ("head", expected, false)],
        )
        .is_err()
        {
            if head_check(root, remote, expected)?.0 == "green" {
                let _ = enqueue(root, remote, number, expected)?;
            } else {
                return Err("Failed to arm auto-merge on PR.".to_owned());
            }
        }
    }
    pull_request(root, remote, number)
}

/// The subcommand and the index of its first argument. `AGENTS.md` and the
/// workflow skills spell landing as `cargo xtask land <pr> --expected-head <sha>`,
/// so a leading pull request number selects `land` and is itself the first
/// argument.
fn subcommand_and_first_argument(args: &[String]) -> (&str, usize) {
    match args.first() {
        Some(first) if first.parse::<u64>().is_ok() => ("land", 0),
        Some(first) => (first.as_str(), 1),
        None => ("", 0),
    }
}

pub fn run(args: &[String]) -> Result<(), u8> {
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help") {
        println!(
            "usage: cargo xtask land <pr> --expected-head <sha> [--wait-seconds <secs>]
         Marks the PR ready, queues it once every required check is green, waits
         for the merge (default {LAND_WAIT_SECONDS} s) and names the merge commit.
         Exit 2 means queued but not merged yet. `--wait-seconds 0` returns at once.
       cargo xtask land status [pr]
         Prints one PR, or the merge queue.
       cargo xtask land wait <pr> --expected-head <sha> [--wait-seconds <secs>]
         Waits for a PR that is already queued to merge. It does not wait for checks.
       cargo xtask land enqueue <pr> --expected-head <sha>
         Queues a PR whose checks are already green, without waiting."
        );
        return Ok(());
    }
    let (sub, mut i) = subcommand_and_first_argument(args);
    let mut pr_number: Option<u64> = None;
    let mut expected_head: Option<String> = None;
    let mut wait_seconds: Option<u64> = None;
    let mut remote = "origin".to_owned();

    while i < args.len() {
        match args[i].as_str() {
            "--expected-head" => {
                i += 1;
                expected_head = args.get(i).cloned();
            }
            "--wait-seconds" => {
                i += 1;
                wait_seconds = args.get(i).and_then(|s| s.parse().ok());
            }
            "--remote" => {
                i += 1;
                if let Some(r) = args.get(i) {
                    remote = r.clone();
                }
            }
            other => {
                if pr_number.is_none()
                    && let Ok(n) = other.parse::<u64>()
                {
                    pr_number = Some(n);
                } else {
                    eprintln!("land: unknown argument `{other}`");
                    return Err(2);
                }
            }
        }
        i += 1;
    }

    let root = repo_root();
    let run_op = || -> Result<(Value, u8), String> {
        match sub {
            "status" => {
                let val = match pr_number {
                    Some(n) => pull_request(&root, &remote, n)?,
                    None => queue_status(&root, &remote)?,
                };
                Ok((val, 0))
            }
            "enqueue" => {
                let (Some(n), Some(head)) = (pr_number, expected_head.as_deref()) else {
                    return Err("Supply a PR number and --expected-head FULL_SHA.".to_owned());
                };
                Ok((enqueue(&root, &remote, n, head)?, 0))
            }
            "land" | "wait" => {
                let (Some(n), Some(head)) = (pr_number, expected_head.as_deref()) else {
                    return Err("Supply a PR number and --expected-head FULL_SHA.".to_owned());
                };
                let pr = if sub == "land" {
                    arm(&root, &remote, n, head)?
                } else {
                    pull_request(&root, &remote, n)?
                };
                let secs =
                    wait_seconds.unwrap_or(if sub == "land" { LAND_WAIT_SECONDS } else { 60 });
                if pr["state"].as_str() == Some("MERGED") || secs == 0 {
                    return Ok((pr, 0));
                }
                let deadline = Instant::now() + Duration::from_secs(secs);
                loop {
                    let cur = pull_request(&root, &remote, n)?;
                    validate_head(&cur, head)?;
                    if cur["state"].as_str() == Some("MERGED") {
                        return Ok((cur, 0));
                    }
                    if cur["state"].as_str() == Some("CLOSED")
                        || cur["mergeStateStatus"].as_str() == Some("DIRTY")
                    {
                        return Err(format!("PR {n} closed or has merge conflict."));
                    }
                    if Instant::now() >= deadline {
                        return Ok((cur, 2));
                    }
                    thread::sleep(Duration::from_secs(CHECK_POLL_SECONDS));
                }
            }
            other => Err(format!("unknown land subcommand `{other}`")),
        }
    };

    match run_op() {
        Ok((val, code)) => {
            println!("{val}");
            // The JSON above is for scripts. A person or an agent reads this
            // line, and only a merge counts as landed.
            if matches!(sub, "land" | "wait") {
                let number = val["number"].as_u64().unwrap_or_default();
                if val["state"].as_str() == Some("MERGED") {
                    eprintln!(
                        "landing: #{number} merged as {}",
                        val["mergeCommit"]["oid"]
                            .as_str()
                            .unwrap_or("a commit GitHub did not report")
                    );
                } else {
                    let queue = &val["mergeQueueEntry"];
                    let place = if queue.is_null() {
                        "not in the merge queue".to_owned()
                    } else {
                        format!("in the merge queue at position {}", queue["position"])
                    };
                    eprintln!(
                        "landing: #{number} has NOT merged yet ({place}); do not report it as landed"
                    );
                }
            }
            if code == 0 { Ok(()) } else { Err(code) }
        }
        Err(e) => {
            eprintln!("landing: {e}");
            Err(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{required_checks_verdict, subcommand_and_first_argument};

    // Branch protection requires both checks. A PR can have a green
    // `compiler-check` and no `workspace-clippy` run at all, as one retargeted
    // without a push did before `lint-when-mergeable.yml`, and that must not
    // read as green.
    #[test]
    fn a_required_check_that_never_ran_is_missing_not_green() {
        let run = |name: &str, status: &str, conclusion: &str| {
            serde_json::json!({
                "__typename": "CheckRun",
                "name": name,
                "status": status,
                "conclusion": conclusion,
                "startedAt": "2026-11-10T00:00:00Z",
            })
        };
        let both = [
            run("compiler-check", "COMPLETED", "SUCCESS"),
            run("workspace-clippy", "COMPLETED", "SUCCESS"),
        ];
        assert_eq!(required_checks_verdict(&both, "abc").0, "green");

        let (verdict, why) =
            required_checks_verdict(&[run("compiler-check", "COMPLETED", "SUCCESS")], "abc");
        assert_eq!(verdict, "missing");
        assert!(why.contains("workspace-clippy has not run on abc"));

        let running = [
            run("compiler-check", "COMPLETED", "SUCCESS"),
            run("workspace-clippy", "IN_PROGRESS", ""),
        ];
        assert_eq!(required_checks_verdict(&running, "abc").0, "pending");

        let failed = [
            run("compiler-check", "COMPLETED", "FAILURE"),
            run("workspace-clippy", "COMPLETED", "SUCCESS"),
        ];
        assert_eq!(required_checks_verdict(&failed, "abc").0, "red");
    }

    // The documented `land <pr> --expected-head <sha>` form and the explicit
    // subcommand forms must reach the same parser position for the number.
    #[test]
    fn a_leading_pull_request_number_selects_land() {
        let args = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        let documented = args(&["2236", "--expected-head", "abc"]);
        assert_eq!(subcommand_and_first_argument(&documented), ("land", 0));
        let explicit = args(&["land", "2236", "--expected-head", "abc"]);
        assert_eq!(subcommand_and_first_argument(&explicit), ("land", 1));
        let status = args(&["status"]);
        assert_eq!(subcommand_and_first_argument(&status), ("status", 1));
    }
}
