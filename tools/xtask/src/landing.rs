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
fn required_checks_verdict(nodes: &[Value], head: &str) -> (String, String) {
    let mut running = Vec::new();
    let mut missing = Vec::new();
    for (required, _) in REQUIRED_CHECKS {
        let latest = nodes
            .iter()
            .filter(|n| n["name"].as_str() == Some(required))
            // New attempts get new check-run IDs even before they start. A
            // queued retry must not let an older success authorize queueing.
            .max_by_key(|n| n["id"].as_u64());
        let Some(run) = latest else {
            missing.push(*required);
            continue;
        };
        let status = run["status"].as_str().unwrap_or("");
        if status != "completed" {
            running.push(format!("{required} is {}", status.to_ascii_lowercase()));
            continue;
        }
        let conclusion = run["conclusion"].as_str().unwrap_or("");
        if conclusion != "success" {
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
    head_check_with_api(&format!("{owner}/{name}"), head, |endpoint| {
        let out = run_cmd(root, &["gh", "api", endpoint])?;
        serde_json::from_str(&out).map_err(|e| format!("Invalid check response: {e}"))
    })
}

fn head_check_with_api(
    repository: &str,
    head: &str,
    mut api: impl FnMut(&str) -> Result<Value, String>,
) -> Result<(String, String), String> {
    // GitHub can omit a commit's rollup while its real jobs are available.
    // Read the complete job list, then authenticate the required jobs against
    // their workflow runs; an unavailable aggregate is never a success.
    let mut runs = Vec::new();
    let mut total = None;
    for page in 1.. {
        let response = api(&format!(
            "repos/{repository}/commits/{head}/check-runs?filter=all&per_page=100&page={page}"
        ))?;
        let count = response["total_count"]
            .as_u64()
            .ok_or("Check response has no total_count")?;
        if total.is_some_and(|previous| previous != count) {
            return Err("Check list changed during pagination; retry the head check.".to_owned());
        }
        total = Some(count);
        let rows = response["check_runs"]
            .as_array()
            .ok_or("Check response has no check_runs array")?;
        if rows.len() > 100 || runs.len() as u64 + rows.len() as u64 > count {
            return Err("Check response exceeds its declared page or total count.".to_owned());
        }
        runs.extend_from_slice(rows);
        if runs.len() as u64 == count {
            break;
        }
        if rows.len() != 100 {
            return Err("Incomplete check-run page; refusing a partial verdict.".to_owned());
        }
    }
    for run in &runs {
        if run["id"].as_u64().filter(|id| *id != 0).is_none()
            || run["head_sha"].as_str() != Some(head)
        {
            return Err("Check run has an invalid ID or a different source head.".to_owned());
        }
    }
    runs.sort_unstable_by_key(|run| run["id"].as_u64());
    if runs.windows(2).any(|pair| pair[0]["id"] == pair[1]["id"]) {
        return Err("Duplicate check-run IDs; refusing an incomplete or changing list.".to_owned());
    }
    for run in &runs {
        let Some((_, workflow)) = REQUIRED_CHECKS
            .iter()
            .find(|(name, _)| run["name"].as_str() == Some(name))
        else {
            continue;
        };
        if !matches!(
            run["status"].as_str(),
            Some("queued" | "in_progress" | "completed" | "waiting" | "pending" | "requested")
        ) {
            return Err(format!(
                "Required check {} has an unknown status.",
                run["name"]
            ));
        }
        if run["app"]["id"].as_u64() != Some(GITHUB_ACTIONS_APP_ID)
            || run["app"]["slug"].as_str() != Some("github-actions")
        {
            return Err(format!(
                "Required check {} is not from GitHub Actions.",
                run["name"]
            ));
        }
        let suite = run["check_suite"]["id"]
            .as_u64()
            .filter(|id| *id != 0)
            .ok_or("Required check has no check-suite identity")?;
        let response = api(&format!(
            "repos/{repository}/actions/runs?check_suite_id={suite}&head_sha={head}&per_page=100"
        ))?;
        let workflows = response["workflow_runs"]
            .as_array()
            .ok_or("Workflow response has no workflow_runs array")?;
        if response["total_count"].as_u64() != Some(1) || workflows.len() != 1 {
            return Err("Required check has no unique workflow run.".to_owned());
        }
        let source = &workflows[0];
        if source["check_suite_id"].as_u64() != Some(suite)
            || source["head_sha"].as_str() != Some(head)
            || source["path"].as_str() != Some(workflow)
            || !source["repository"]["full_name"]
                .as_str()
                .is_some_and(|name| name.eq_ignore_ascii_case(repository))
        {
            return Err(format!(
                "Required check {} does not match its repository, source head and workflow.",
                run["name"]
            ));
        }
    }
    Ok(required_checks_verdict(&runs, head))
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
    use super::{
        head_check_with_api, required_checks_verdict, subcommand_and_first_argument, validate_head,
    };
    use serde_json::{Value, json};

    // Branch protection requires both checks. A PR can have a green
    // `compiler-check` and no `workspace-clippy` run at all, as one retargeted
    // without a push did before `lint-when-mergeable.yml`, and that must not
    // read as green.
    #[test]
    fn a_required_check_that_never_ran_is_missing_not_green() {
        let run = |name: &str, status: &str, conclusion: &str| {
            serde_json::json!({
                "id": if name == "compiler-check" { 1 } else { 2 },
                "name": name,
                "status": status,
                "conclusion": conclusion,
                "started_at": "2026-11-10T00:00:00Z",
            })
        };
        let both = [
            run("compiler-check", "completed", "success"),
            run("workspace-clippy", "completed", "success"),
        ];
        assert_eq!(required_checks_verdict(&both, "abc").0, "green");

        let (verdict, why) =
            required_checks_verdict(&[run("compiler-check", "completed", "success")], "abc");
        assert_eq!(verdict, "missing");
        assert!(why.contains("workspace-clippy has not run on abc"));

        let running = [
            run("compiler-check", "completed", "success"),
            run("workspace-clippy", "in_progress", ""),
        ];
        assert_eq!(required_checks_verdict(&running, "abc").0, "pending");

        let failed = [
            run("compiler-check", "completed", "failure"),
            run("workspace-clippy", "completed", "success"),
        ];
        assert_eq!(required_checks_verdict(&failed, "abc").0, "red");
    }

    const REPOSITORY: &str = "CathedralOS/Omega";
    const HEAD: &str = "5c91c558035a5e9c45501c99b97440721a28f239";

    fn successful_runs() -> Vec<Value> {
        vec![
            json!({
                "id": 130842405903u64, "name": "compiler-check", "head_sha": HEAD,
                "status": "completed", "conclusion": "success",
                "app": {"id": 15368, "slug": "github-actions"},
                "check_suite": {"id": 116760391297u64},
            }),
            json!({
                "id": 130842418520u64, "name": "workspace-clippy", "head_sha": HEAD,
                "status": "completed", "conclusion": "success",
                "app": {"id": 15368, "slug": "github-actions"},
                "check_suite": {"id": 116760400518u64},
            }),
        ]
    }

    fn workflow_response(endpoint: &str) -> Value {
        let (suite, path) = if endpoint.contains("check_suite_id=116760391297&") {
            (116760391297u64, ".github/workflows/compiler.yml")
        } else {
            assert!(endpoint.contains("check_suite_id=116760400518&"));
            (116760400518u64, ".github/workflows/repository-lint.yml")
        };
        assert_eq!(
            endpoint,
            format!(
                "repos/{REPOSITORY}/actions/runs?check_suite_id={suite}&head_sha={HEAD}&per_page=100"
            )
        );
        json!({"total_count": 1, "workflow_runs": [{
            "head_sha": HEAD, "check_suite_id": suite, "path": path,
            "repository": {"full_name": REPOSITORY},
        }]})
    }

    fn check_runs(runs: &[Value]) -> Result<(String, String), String> {
        head_check_with_api(REPOSITORY, HEAD, |endpoint| {
            if endpoint.contains("/check-runs?") {
                assert!(endpoint.ends_with("filter=all&per_page=100&page=1"));
                Ok(json!({"total_count": runs.len(), "check_runs": runs}))
            } else {
                Ok(workflow_response(endpoint))
            }
        })
    }

    #[test]
    fn real_jobs_do_not_depend_on_a_commit_rollup() {
        // PR #2465 returned a null aggregate alongside these two real jobs.
        // The exercised ingress must request job records, not that aggregate.
        let mut requests = Vec::new();
        let verdict = head_check_with_api(REPOSITORY, HEAD, |endpoint| {
            requests.push(endpoint.to_owned());
            if endpoint.contains("/check-runs?") {
                assert_eq!(endpoint, format!(
                    "repos/{REPOSITORY}/commits/{HEAD}/check-runs?filter=all&per_page=100&page=1"
                ));
                Ok(json!({"total_count": 2, "check_runs": successful_runs()}))
            } else {
                Ok(workflow_response(endpoint))
            }
        }).unwrap();
        assert_eq!(verdict.0, "green");
        assert_eq!(requests.len(), 3);
        assert!(!requests.iter().any(|request| request.contains("graphql")));
    }

    #[test]
    fn required_jobs_are_authenticated_before_the_verdict() {
        for (path, replacement) in [
            (vec!["head_sha"], json!("another-head")),
            (vec!["id"], Value::Null),
            (vec!["status"], json!("unrecognized")),
            (vec!["app", "id"], json!(99)),
            (vec!["app", "slug"], json!("another-app")),
            (vec!["check_suite", "id"], Value::Null),
        ] {
            let mut runs = successful_runs();
            let mut field = &mut runs[0];
            for key in path {
                field = &mut field[key];
            }
            *field = replacement;
            assert!(check_runs(&runs).is_err());
        }
        for (path, replacement) in [
            (vec!["head_sha"], json!("another-head")),
            (vec!["path"], json!(".github/workflows/unrelated.yml")),
            (vec!["repository", "full_name"], json!("another/repository")),
            (vec!["check_suite_id"], json!(123)),
        ] {
            let error = head_check_with_api(REPOSITORY, HEAD, |endpoint| {
                if endpoint.contains("/check-runs?") {
                    return Ok(json!({"total_count": 2, "check_runs": successful_runs()}));
                }
                let mut response = workflow_response(endpoint);
                let mut field = &mut response["workflow_runs"][0];
                for key in &path {
                    field = &mut field[*key];
                }
                *field = replacement.clone();
                Ok(response)
            })
            .unwrap_err();
            assert!(error.contains("does not match"), "{error}");
        }
    }

    #[test]
    fn a_missing_job_or_an_unstarted_retry_cannot_use_an_older_success() {
        let runs = successful_runs();
        assert_eq!(check_runs(&runs[..1]).unwrap().0, "missing");
        for (status, conclusion, expected) in [
            ("queued", Value::Null, "pending"),
            ("in_progress", Value::Null, "pending"),
            ("completed", json!("failure"), "red"),
            ("completed", json!("skipped"), "red"),
            ("completed", json!("neutral"), "red"),
            ("completed", json!("cancelled"), "red"),
            ("completed", Value::Null, "red"),
        ] {
            let mut attempts = runs.clone();
            let mut retry = runs[0].clone();
            retry["id"] = json!(130842418521u64);
            retry["status"] = json!(status);
            retry["conclusion"] = conclusion;
            retry["started_at"] = Value::Null;
            attempts.insert(0, retry);
            assert_eq!(check_runs(&attempts).unwrap().0, expected);
            attempts.reverse();
            assert_eq!(check_runs(&attempts).unwrap().0, expected);
        }
        let mut attempts = runs;
        let mut old_failure = attempts[0].clone();
        old_failure["id"] = json!(1);
        old_failure["conclusion"] = json!("failure");
        attempts.push(old_failure);
        assert_eq!(check_runs(&attempts).unwrap().0, "green");
    }

    fn first_page() -> Vec<Value> {
        let mut rows: Vec<_> = (1..=98)
            .map(|id| json!({"id": id, "head_sha": HEAD, "name": "unrequired"}))
            .collect();
        rows.extend(successful_runs());
        rows
    }

    #[test]
    fn later_pages_can_block_successes_from_the_first_page() {
        let mut requests = Vec::new();
        let verdict = head_check_with_api(REPOSITORY, HEAD, |endpoint| {
            requests.push(endpoint.to_owned());
            if endpoint.contains("/check-runs?") {
                let rows = if endpoint.ends_with("page=1") {
                    first_page()
                } else {
                    assert!(endpoint.ends_with("page=2"));
                    let mut failed = successful_runs().remove(0);
                    failed["id"] = json!(130842418521u64);
                    failed["conclusion"] = json!("failure");
                    vec![failed]
                };
                Ok(json!({"total_count": 101, "check_runs": rows}))
            } else {
                Ok(workflow_response(endpoint))
            }
        })
        .unwrap();
        assert_eq!(verdict.0, "red");
        assert!(requests[1].ends_with("page=2"));
    }

    #[test]
    fn incomplete_or_changing_pages_and_transport_errors_refuse() {
        for response in [
            Value::Null,
            json!({"total_count": 2}),
            json!({"total_count": 3, "check_runs": successful_runs()}),
            json!({"total_count": 1, "check_runs": successful_runs()}),
        ] {
            assert!(head_check_with_api(REPOSITORY, HEAD, |_| Ok(response.clone())).is_err());
        }
        for second in [
            Err("GitHub unavailable".to_owned()),
            Ok(json!({"total_count": 102, "check_runs": []})),
            Ok(json!({"total_count": 101, "check_runs": []})),
            Ok(json!({"total_count": 101, "check_runs": [first_page()[0].clone()]})),
        ] {
            assert!(
                head_check_with_api(REPOSITORY, HEAD, |endpoint| {
                    assert!(endpoint.contains("/check-runs?"));
                    if endpoint.ends_with("page=1") {
                        Ok(json!({"total_count": 101, "check_runs": first_page()}))
                    } else {
                        second.clone()
                    }
                })
                .is_err()
            );
        }
    }

    #[test]
    fn missing_ambiguous_or_partial_workflow_receipts_refuse() {
        for response in [
            Value::Null,
            json!({"total_count": 0, "workflow_runs": []}),
            json!({"total_count": 2, "workflow_runs": workflow_response(
                "repos/CathedralOS/Omega/actions/runs?check_suite_id=116760391297&head_sha=5c91c558035a5e9c45501c99b97440721a28f239&per_page=100"
            )["workflow_runs"]}),
        ] {
            assert!(
                head_check_with_api(REPOSITORY, HEAD, |endpoint| {
                    if endpoint.contains("/check-runs?") {
                        Ok(json!({"total_count": 2, "check_runs": successful_runs()}))
                    } else {
                        Ok(response.clone())
                    }
                })
                .is_err()
            );
        }
    }

    #[test]
    fn expected_head_and_base_fences_are_unchanged() {
        let mut pr = json!({"baseRefName": "main", "headRefOid": HEAD});
        assert!(validate_head(&pr, HEAD).is_ok());
        assert!(validate_head(&pr, "another-head").is_err());
        pr["headRefOid"] = json!("0".repeat(40));
        assert!(validate_head(&pr, HEAD).is_err());
        pr["headRefOid"] = json!(HEAD);
        pr["baseRefName"] = json!("another-base");
        assert!(validate_head(&pr, HEAD).is_err());
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
