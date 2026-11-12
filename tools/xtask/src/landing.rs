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
const REQUIRED_CHECKS: &[(&str, &str)] = &[
    ("compiler-check", ".github/workflows/compiler.yml"),
    ("workspace-clippy", ".github/workflows/repository-lint.yml"),
];
const GITHUB_ACTIONS_APP_ID: u64 = 15368;
const CHECK_POLL_SECONDS: u64 = 15;
/// `land` waits this long for the merge, so "queued" is not read as "landed".
const LAND_WAIT_SECONDS: u64 = 1800;

/// The verdict for a commit from its check runs: `green`, `red`, `pending`
/// (a required check is still running) or `missing` (one never started).
fn required_checks_verdict(jobs: &[Value], workflows: &[Value], head: &str) -> (String, String) {
    let mut running = Vec::new();
    let mut missing = Vec::new();
    for (required, path) in REQUIRED_CHECKS {
        let sources: Vec<_> = workflows
            .iter()
            .filter(|run| run["path"] == *path)
            .collect();
        if sources.is_empty() {
            missing.push(*required);
            continue;
        }
        if sources.iter().any(|run| run["status"] != "completed") {
            running.push(format!("{required} has a pending workflow attempt"));
            continue;
        }
        let candidates: Vec<_> = jobs.iter().filter(|job| job["name"] == *required).collect();
        if candidates.len() != sources.len() {
            missing.push(*required);
            continue;
        }
        // Only current-attempt jobs reach here. Across reruns of older runs,
        // compare actual job start times, never opaque database IDs. Tied
        // times authorize queueing only when every tied job succeeded.
        let latest = candidates
            .iter()
            .map(|job| job["started_at"].as_str().unwrap_or(""))
            .max();
        for job in candidates
            .iter()
            .filter(|job| job["started_at"].as_str() == latest)
        {
            if job["status"] != "completed" {
                running.push(format!("{required} is not completed"));
            } else if job["conclusion"] != "success" {
                return (
                    "red".to_owned(),
                    format!("{required} concluded {}", job["conclusion"]),
                );
            }
        }
    }
    if !missing.is_empty() {
        return (
            "missing".to_owned(),
            format!(
                "{} has no current required job on {head}; inspect the official workflows in Actions before retrying.",
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
    head_check_with_api(&format!("{owner}/{name}"), head, |endpoint| {
        let out = run_cmd(root, &["gh", "api", endpoint])?;
        serde_json::from_str(&out).map_err(|e| format!("Invalid check response: {e}"))
    })
}

fn rest_list(
    api: &mut impl FnMut(&str) -> Result<Value, String>,
    endpoint: &str,
    field: &str,
) -> Result<Vec<Value>, String> {
    let mut records = Vec::new();
    let mut total = None;
    let separator = if endpoint.contains('?') { '&' } else { '?' };
    for page in 1.. {
        let response = api(&format!("{endpoint}{separator}per_page=100&page={page}"))?;
        let count = response["total_count"]
            .as_u64()
            .ok_or("List response has no total_count")?;
        if total.is_some_and(|previous| previous != count) {
            return Err("List changed during pagination; retry the head check.".to_owned());
        }
        total = Some(count);
        let rows = response[field]
            .as_array()
            .ok_or_else(|| format!("List response has no {field} array"))?;
        if rows.len() > 100 || records.len() as u64 + rows.len() as u64 > count {
            return Err("List response exceeds its declared page or total count.".to_owned());
        }
        records.extend_from_slice(rows);
        if records.len() as u64 == count {
            break;
        }
        if rows.len() != 100 {
            return Err("Incomplete page; refusing a partial verdict.".to_owned());
        }
    }
    if records
        .iter()
        .any(|row| row["id"].as_u64().filter(|id| *id != 0).is_none())
    {
        return Err("List contains an invalid record ID.".to_owned());
    }
    records.sort_unstable_by_key(|row| row["id"].as_u64());
    if records
        .windows(2)
        .any(|pair| pair[0]["id"] == pair[1]["id"])
    {
        return Err("Duplicate record IDs; refusing an incomplete or changing list.".to_owned());
    }
    Ok(records)
}

fn known_check_status(value: &Value) -> bool {
    matches!(
        value.as_str(),
        Some("queued" | "in_progress" | "completed" | "waiting" | "pending" | "requested")
    )
}

fn head_check_with_api(
    repository: &str,
    head: &str,
    mut api: impl FnMut(&str) -> Result<Value, String>,
) -> Result<(String, String), String> {
    // GitHub can omit the rollup while real jobs are available. Independent
    // workflow discovery also catches a new run or rerun before it has a job.
    let checks = rest_list(
        &mut api,
        &format!("repos/{repository}/commits/{head}/check-runs?filter=all"),
        "check_runs",
    )?;
    if checks
        .iter()
        .any(|run| run["head_sha"].as_str() != Some(head))
    {
        return Err("Check run has a different source head.".to_owned());
    }
    let mut current_workflows = Vec::new();
    let mut current_jobs = Vec::new();
    for (required, path) in REQUIRED_CHECKS {
        let file = path
            .rsplit('/')
            .next()
            .ok_or("Required workflow has no file name")?;
        let workflows = rest_list(
            &mut api,
            &format!("repos/{repository}/actions/workflows/{file}/runs?head_sha={head}"),
            "workflow_runs",
        )?;
        let mut identity = None;
        let mut numbers = Vec::new();
        for run in &workflows {
            let workflow_id = run["workflow_id"]
                .as_u64()
                .filter(|id| *id != 0)
                .ok_or("Workflow has no identity")?;
            if identity.is_some_and(|previous| previous != workflow_id) {
                return Err("Workflow identity changed; attempt order is ambiguous.".to_owned());
            }
            identity = Some(workflow_id);
            let number = run["run_number"]
                .as_u64()
                .filter(|n| *n != 0)
                .ok_or("Workflow has no run number")?;
            numbers.push(number);
            if run["head_sha"].as_str() != Some(head)
                || run["path"].as_str() != Some(path)
                || !run["repository"]["full_name"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(repository))
                || run["run_attempt"].as_u64().filter(|n| *n != 0).is_none()
                || !known_check_status(&run["status"])
            {
                return Err(
                    "Workflow does not match its repository, head, path or current attempt."
                        .to_owned(),
                );
            }
        }
        numbers.sort_unstable();
        if numbers.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("Duplicate workflow run numbers; attempt order is ambiguous.".to_owned());
        }
        let newest = numbers.last().copied();
        // GitHub documents run_number as increasing for new runs of one
        // workflow. An older run's first attempt is superseded; a rerun may
        // have happened later, so its current attempt must also be considered.
        for run in workflows.iter().filter(|run| {
            run["run_number"].as_u64() == newest
                || run["run_attempt"].as_u64().is_some_and(|n| n > 1)
        }) {
            current_workflows.push(run.clone());
            if run["status"] != "completed" {
                continue;
            }
            let id = run["id"].as_u64().ok_or("Workflow has no run ID")?;
            let attempt = run["run_attempt"]
                .as_u64()
                .ok_or("Workflow has no current attempt")?;
            let suite = run["check_suite_id"]
                .as_u64()
                .filter(|id| *id != 0)
                .ok_or("Workflow has no check-suite identity")?;
            let canonical_repository = run["repository"]["full_name"]
                .as_str()
                .ok_or("Workflow has no repository identity")?;
            let jobs = rest_list(
                &mut api,
                &format!("repos/{repository}/actions/runs/{id}/attempts/{attempt}/jobs"),
                "jobs",
            )?;
            let selected: Vec<_> = jobs.iter().filter(|job| job["name"] == *required).collect();
            if selected.len() > 1 {
                return Err(format!(
                    "Required job {required} is ambiguous within one attempt."
                ));
            }
            for job in selected {
                if job["run_id"].as_u64() != Some(id)
                    || job["run_attempt"].as_u64() != Some(attempt)
                    || job["head_sha"].as_str() != Some(head)
                    || !known_check_status(&job["status"])
                {
                    return Err(format!(
                        "Required job {required} does not match its current workflow attempt."
                    ));
                }
                let check = checks
                    .iter()
                    .find(|check| {
                        job["check_run_url"].as_str()
                            == Some(&format!(
                                "https://api.github.com/repos/{canonical_repository}/check-runs/{}",
                                check["id"]
                            ))
                    })
                    .ok_or_else(|| {
                        format!("Required job {required} has no exact check-run receipt.")
                    })?;
                if check["name"] != *required
                    || check["app"]["id"].as_u64() != Some(GITHUB_ACTIONS_APP_ID)
                    || check["app"]["slug"].as_str() != Some("github-actions")
                    || check["check_suite"]["id"].as_u64() != Some(suite)
                    || check["status"] != job["status"]
                    || check["conclusion"] != job["conclusion"]
                {
                    return Err(format!(
                        "Required job {required} has a mismatched producer, suite or check state."
                    ));
                }
                let started = job["started_at"]
                    .as_str()
                    .ok_or("Required job has no start time; attempt order is unavailable")?;
                // GitHub's UTC second-resolution form sorts chronologically.
                // Other forms need explicit normalization, never raw ordering.
                if started.len() != 20
                    || !started.bytes().enumerate().all(|(i, c)| match i {
                        4 | 7 => c == b'-',
                        10 => c == b'T',
                        13 | 16 => c == b':',
                        19 => c == b'Z',
                        _ => c.is_ascii_digit(),
                    })
                {
                    return Err("Required job start time has an unsupported format.".to_owned());
                }
                current_jobs.push(job.clone());
            }
        }
    }
    Ok(required_checks_verdict(
        &current_jobs,
        &current_workflows,
        head,
    ))
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
    use super::{head_check_with_api, rest_list, subcommand_and_first_argument, validate_head};
    use serde_json::{Value, json};

    const REPOSITORY: &str = "CathedralOS/Omega";
    const HEAD: &str = "5c91c558035a5e9c45501c99b97440721a28f239";

    fn fixture() -> Value {
        let mut checks = Vec::new();
        let mut jobs = Vec::new();
        let mut workflows = Vec::new();
        for (name, file, id, suite, check) in [
            (
                "compiler-check",
                "compiler.yml",
                43115526581u64,
                116760391297u64,
                130842405903u64,
            ),
            (
                "workspace-clippy",
                "repository-lint.yml",
                43115530214u64,
                116760400518u64,
                130842418520u64,
            ),
        ] {
            checks.push(json!({"id":check,"name":name,"head_sha":HEAD,"status":"completed","conclusion":"success","app":{"id":15368,"slug":"github-actions"},"check_suite":{"id":suite}}));
            jobs.push(json!({"id":check,"run_id":id,"run_attempt":1,"name":name,"head_sha":HEAD,"status":"completed","conclusion":"success","started_at":"2026-11-11T22:53:53Z","check_run_url":format!("https://api.github.com/repos/{REPOSITORY}/check-runs/{check}")}));
            workflows.push(json!({"id":id,"workflow_id":suite,"run_number":100,"run_attempt":1,"status":"completed","check_suite_id":suite,"path":format!(".github/workflows/{file}"),"head_sha":HEAD,"repository":{"full_name":REPOSITORY}}));
        }
        json!({"checks":checks,"jobs":jobs,"compiler_runs":[workflows[0]],"lint_runs":[workflows[1]]})
    }

    fn fixture_response(f: &Value, endpoint: &str) -> Value {
        assert!(endpoint.starts_with(&format!("repos/{REPOSITORY}/")));
        assert!(endpoint.ends_with("per_page=100&page=1"));
        let (field, rows) = if endpoint.contains("/check-runs?") {
            assert!(endpoint.contains(&format!("/commits/{HEAD}/check-runs?filter=all&")));
            ("check_runs", f["checks"].as_array().unwrap().clone())
        } else if endpoint.contains("/workflows/") {
            assert!(endpoint.contains(&format!("head_sha={HEAD}&")));
            let key = if endpoint.contains("/compiler.yml/") {
                "compiler_runs"
            } else {
                assert!(endpoint.contains("/repository-lint.yml/"));
                "lint_runs"
            };
            ("workflow_runs", f[key].as_array().unwrap().clone())
        } else {
            let parts: Vec<_> = endpoint.split('/').collect();
            let id: u64 = parts[5].parse().unwrap();
            let attempt: u64 = parts[7].parse().unwrap();
            assert_eq!(parts[6], "attempts");
            (
                "jobs",
                f["jobs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|job| job["run_id"] == id && job["run_attempt"] == attempt)
                    .cloned()
                    .collect(),
            )
        };
        json!({"total_count":rows.len(),field:rows})
    }

    fn check(f: &Value) -> Result<(String, String), String> {
        head_check_with_api(REPOSITORY, HEAD, |endpoint| {
            Ok(fixture_response(f, endpoint))
        })
    }

    // A new run deliberately has smaller database IDs than the old success.
    fn add_compiler_attempt(
        f: &mut Value,
        coordinates: (u64, u64, u64, u64),
        started: &str,
        outcome: (&str, &str),
    ) {
        let (id, number, attempt, check) = coordinates;
        let mut run = f["compiler_runs"][0].clone();
        run["id"] = json!(id);
        run["run_number"] = json!(number);
        run["run_attempt"] = json!(attempt);
        run["status"] = json!(outcome.0);
        f["compiler_runs"].as_array_mut().unwrap().push(run);
        let mut job = f["jobs"][0].clone();
        job["id"] = json!(check);
        job["run_id"] = json!(id);
        job["run_attempt"] = json!(attempt);
        job["started_at"] = json!(started);
        job["status"] = json!(outcome.0);
        job["conclusion"] = json!(outcome.1);
        job["check_run_url"] = json!(format!(
            "https://api.github.com/repos/{REPOSITORY}/check-runs/{check}"
        ));
        f["jobs"].as_array_mut().unwrap().push(job);
        let mut receipt = f["checks"][0].clone();
        receipt["id"] = json!(check);
        receipt["status"] = json!(outcome.0);
        receipt["conclusion"] = json!(outcome.1);
        f["checks"].as_array_mut().unwrap().push(receipt);
    }

    #[test]
    fn real_jobs_do_not_depend_on_a_commit_rollup() {
        let f = fixture();
        let mut requests = Vec::new();
        let result = head_check_with_api(REPOSITORY, HEAD, |endpoint| {
            requests.push(endpoint.to_owned());
            Ok(fixture_response(&f, endpoint))
        })
        .unwrap();
        assert_eq!(result.0, "green");
        assert_eq!(requests.len(), 5);
        assert!(!requests.iter().any(|request| request.contains("graphql")));
        assert_eq!(
            head_check_with_api("cathedralos/omega", HEAD, |endpoint| {
                Ok(fixture_response(
                    &f,
                    &endpoint.replacen("cathedralos/omega", REPOSITORY, 1),
                ))
            })
            .unwrap()
            .0,
            "green"
        );
    }

    #[test]
    fn chronology_does_not_depend_on_check_or_workflow_database_ids() {
        for (status, conclusion, expected) in [
            ("queued", "", "pending"),
            ("in_progress", "", "pending"),
            ("completed", "failure", "red"),
            ("completed", "success", "green"),
        ] {
            let mut f = fixture();
            add_compiler_attempt(
                &mut f,
                (1, 101, 1, 2),
                "2026-11-11T23:00:00Z",
                (status, conclusion),
            );
            assert_eq!(check(&f).unwrap().0, expected);
            f["compiler_runs"].as_array_mut().unwrap().reverse();
            f["checks"].as_array_mut().unwrap().reverse();
            assert_eq!(check(&f).unwrap().0, expected);
        }
        let mut f = fixture();
        f["checks"][0]["conclusion"] = json!("failure");
        f["jobs"][0]["conclusion"] = json!("failure");
        add_compiler_attempt(
            &mut f,
            (1, 101, 1, 2),
            "2026-11-11T23:00:00Z",
            ("completed", "success"),
        );
        assert_eq!(check(&f).unwrap().0, "green");
    }

    #[test]
    fn queued_new_run_and_rerun_cannot_reuse_old_attempt_jobs() {
        let mut f = fixture();
        let mut queued = f["compiler_runs"][0].clone();
        queued["id"] = json!(1);
        queued["run_number"] = json!(101);
        queued["status"] = json!("queued");
        f["compiler_runs"].as_array_mut().unwrap().push(queued);
        assert_eq!(check(&f).unwrap().0, "pending");
        let mut f = fixture();
        f["compiler_runs"][0]["run_attempt"] = json!(2);
        f["compiler_runs"][0]["status"] = json!("queued");
        assert_eq!(check(&f).unwrap().0, "pending");
        f["compiler_runs"][0]["status"] = json!("completed");
        assert_eq!(check(&f).unwrap().0, "missing");
    }

    #[test]
    fn old_initial_runs_are_superseded_but_later_reruns_are_not() {
        let mut f = fixture();
        let mut old = f["compiler_runs"][0].clone();
        old["id"] = json!(999999999999u64);
        old["run_number"] = json!(99);
        old["status"] = json!("queued");
        f["compiler_runs"].as_array_mut().unwrap().push(old);
        assert_eq!(check(&f).unwrap().0, "green");
        f["compiler_runs"][1]["run_attempt"] = json!(2);
        assert_eq!(check(&f).unwrap().0, "pending");
        for (started, expected) in [
            ("2026-11-11T23:00:00Z", "red"),
            ("2026-11-11T22:00:00Z", "green"),
        ] {
            let mut f = fixture();
            add_compiler_attempt(&mut f, (1, 99, 2, 2), started, ("completed", "failure"));
            assert_eq!(check(&f).unwrap().0, expected);
        }
        let mut f = fixture();
        add_compiler_attempt(
            &mut f,
            (1, 99, 2, 2),
            "2026-11-11T22:53:53Z",
            ("completed", "failure"),
        );
        assert_eq!(check(&f).unwrap().0, "red");
    }

    #[test]
    fn authentication_and_unknown_attempt_metadata_refuse() {
        for (root, field, value) in [
            ("checks", vec!["head_sha"], json!("wrong")),
            ("checks", vec!["app", "id"], json!(99)),
            ("checks", vec!["app", "slug"], json!("wrong")),
            ("checks", vec!["check_suite", "id"], json!(0)),
            ("checks", vec!["status"], json!("unknown")),
            ("compiler_runs", vec!["head_sha"], json!("wrong")),
            (
                "compiler_runs",
                vec!["repository", "full_name"],
                json!("wrong/repo"),
            ),
            (
                "compiler_runs",
                vec!["path"],
                json!(".github/workflows/wrong.yml"),
            ),
            ("compiler_runs", vec!["workflow_id"], Value::Null),
            ("compiler_runs", vec!["check_suite_id"], json!(0)),
            ("compiler_runs", vec!["run_attempt"], Value::Null),
            ("compiler_runs", vec!["run_number"], Value::Null),
            ("compiler_runs", vec!["status"], json!("unknown")),
            ("jobs", vec!["head_sha"], json!("wrong")),
            (
                "jobs",
                vec!["check_run_url"],
                json!("https://api.github.com/repos/other/repo/check-runs/1"),
            ),
            ("jobs", vec!["started_at"], Value::Null),
            ("jobs", vec!["started_at"], json!("not-a-time")),
        ] {
            let mut f = fixture();
            let mut slot = &mut f[root][0];
            for key in field {
                slot = &mut slot[key];
            }
            *slot = value;
            assert!(check(&f).is_err(), "{root}: {f}");
        }
    }

    #[test]
    fn missing_jobs_and_non_success_conclusions_cannot_authorize_queueing() {
        let mut f = fixture();
        f["compiler_runs"] = json!([]);
        assert_eq!(check(&f).unwrap().0, "missing");
        let mut f = fixture();
        f["jobs"].as_array_mut().unwrap().remove(0);
        assert_eq!(check(&f).unwrap().0, "missing");
        for conclusion in [
            "failure",
            "skipped",
            "neutral",
            "cancelled",
            "timed_out",
            "unknown",
            "",
        ] {
            let mut f = fixture();
            f["jobs"][0]["conclusion"] = json!(conclusion);
            f["checks"][0]["conclusion"] = json!(conclusion);
            assert_eq!(check(&f).unwrap().0, "red");
        }
    }

    #[test]
    fn mixed_workflow_identities_duplicate_numbers_and_duplicate_jobs_refuse() {
        for field in ["workflow_id", "run_number"] {
            let mut f = fixture();
            add_compiler_attempt(
                &mut f,
                (1, 101, 1, 2),
                "2026-11-11T23:00:00Z",
                ("completed", "success"),
            );
            f["compiler_runs"][1][field] = if field == "workflow_id" {
                json!(1)
            } else {
                json!(100)
            };
            assert!(check(&f).is_err());
        }
        let mut f = fixture();
        let mut extra = f["jobs"][0].clone();
        extra["id"] = json!(1);
        f["jobs"].as_array_mut().unwrap().push(extra);
        assert!(check(&f).is_err());
    }

    #[test]
    fn every_page_is_required_even_when_the_first_contains_success() {
        let f = fixture();
        let mut requests = Vec::new();
        let verdict = head_check_with_api(REPOSITORY, HEAD, |endpoint| {
            requests.push(endpoint.to_owned());
            if endpoint.contains("/check-runs?") {
                if endpoint.ends_with("page=1") {
                    let mut rows = f["checks"].as_array().unwrap().clone();
                    rows.extend((1..=98).map(|id| json!({"id":id,"head_sha":HEAD})));
                    Ok(json!({"total_count":101,"check_runs":rows}))
                } else {
                    Ok(json!({"total_count":101,"check_runs":[{"id":99,"head_sha":"wrong"}]}))
                }
            } else {
                Ok(fixture_response(&f, endpoint))
            }
        });
        assert!(verdict.is_err());
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn pagination_errors_and_unknown_shapes_are_not_empty_successes() {
        for field in ["check_runs", "workflow_runs", "jobs"] {
            for response in [
                Value::Null,
                json!({"total_count":1,field:[]}),
                json!({"total_count":0,field:[{"id":1}]}),
            ] {
                assert!(
                    rest_list(&mut |_| Ok(response.clone()), "repos/test/list", field).is_err()
                );
            }
            let rows: Vec<_> = (1..=100).map(|id| json!({"id":id})).collect();
            for second in [
                Err("unavailable".to_owned()),
                Ok(json!({"total_count":102,field:[]})),
                Ok(json!({"total_count":101,field:[]})),
                Ok(json!({"total_count":101,field:[{"id":1}]})),
            ] {
                assert!(
                    rest_list(
                        &mut |endpoint| if endpoint.ends_with("page=1") {
                            Ok(json!({"total_count":101,field:rows}))
                        } else {
                            second.clone()
                        },
                        "repos/test/list",
                        field
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn expected_head_and_base_fences_are_unchanged() {
        let mut pr = json!({"baseRefName":"main","headRefOid":HEAD});
        assert!(validate_head(&pr, HEAD).is_ok());
        assert!(validate_head(&pr, "another-head").is_err());
        pr["headRefOid"] = json!("0".repeat(40));
        assert!(validate_head(&pr, HEAD).is_err());
        pr["headRefOid"] = json!(HEAD);
        pr["baseRefName"] = json!("another-base");
        assert!(validate_head(&pr, HEAD).is_err());
    }

    #[test]
    fn a_leading_pull_request_number_selects_land() {
        let args = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            subcommand_and_first_argument(&args(&["2236", "--expected-head", "abc"])),
            ("land", 0)
        );
        assert_eq!(
            subcommand_and_first_argument(&args(&["land", "2236", "--expected-head", "abc"])),
            ("land", 1)
        );
        assert_eq!(
            subcommand_and_first_argument(&args(&["status"])),
            ("status", 1)
        );
    }
}
