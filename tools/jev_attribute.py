#!/usr/bin/env python3
"""jev-attribute: rank candidate commits by plausibility of causing a failure.

Given a failing signature (test name, diagnostic, crash shape) and a candidate
commit window, ask Jev which commits could plausibly produce the failure —
the blame-rank a bisect spends wall-clock on. Solo re-asks localize flagged
candidates (batch labels are contaminated by the worst member, per
build/experiments/failure-triage/CLASSIFY.md).

Usage:
    jev_attribute.py --signature "TEXT" --range BASE..HEAD [--diff-lines 60]
    jev_attribute.py --signature "TEXT" --commits SHA1,SHA2,...

Silent no-op exit 2 without TYPESAFE_API_KEY; OMEGA_JEV_OFFLINE=1 suppresses.
"""
import argparse
import json
import os
import subprocess
import sys
import urllib.request

JEV_ENDPOINT = "https://api.typesafe.ai/v1/systemone"
JEV_MODEL = "jev-1.13.0"


def jev_key() -> str:
    return os.environ.get("TYPESAFE_API_KEY", "").strip()


def jev_post(payload: dict, key: str) -> dict:
    request = urllib.request.Request(
        JEV_ENDPOINT, data=json.dumps(payload).encode(),
        headers={"Authorization": f"Bearer {key}",
                 "Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)


def git(*args) -> str:
    return subprocess.run(["git"] + list(args), capture_output=True,
                          text=True, timeout=30, check=True).stdout


def candidates(range_or_commits) -> list:
    if "," in range_or_commits or ".." not in range_or_commits:
        revs = [r.strip() for r in range_or_commits.split(",") if r.strip()]
    else:
        revs = git("log", "--first-parent", "--format=%H", range_or_commits).split()
    out = []
    for sha in revs:
        # diff against first parent so merges report the changes they brought
        # to main, not an empty patch.
        files = git("diff", "--name-only", f"{sha}^1", sha).split()
        out.append({
            "sha": sha[:10],
            "subject": git("log", "-1", "--format=%s", sha).strip(),
            "files_touched_count": len(files),
            "files": files[:12],
        })
    return out


def questions(count: int) -> dict:
    result = {}
    for index in range(count):
        result[f"evidence_{index}"] = {
            "type": "choice",
            "instructions": (
                "Does THIS candidate's added-lines excerpt literally show "
                "the evidence named by the failing signature? "
                "'contains' = the excerpt visibly adds the offending "
                "content (the marker text, the removed-line shape, the "
                "missing symbol's call). 'absent' = the diff is plainly "
                "unrelated — no matching content. 'inconclusive' = the "
                "excerpt is too truncated or too vague to tell. A commit "
                "that merely COULD have caused the failure (a merge in "
                "the same area, a touched file) with no visible evidence "
                "is 'absent'."),
            "criteria": {
                "contains": "the excerpt literally shows the offending "
                            "content being added",
                "absent": "the diff is plainly unrelated — no matching "
                          "content",
                "inconclusive": "excerpt too truncated or vague to tell"},
        }
    result["no_suspect"] = {
        "type": "noul",
        "instructions": (
            "Is it plausible that NONE of these candidates caused the "
            "failure (the cause lies outside the window or in the "
            "environment)?"),
        "criteria": {
            "true": "the cause is likely outside this window",
            "false": "at least one candidate is a plausible cause"},
    }
    return result


def solo_questions() -> dict:
    one = questions(1)
    one["introduces_0"] = {
        "type": "noul",
        "instructions": (
            "How strongly does this candidate's diff carry the failing "
            "signature's evidence? High = the excerpt literally contains "
            "it; low = unrelated or merely adjacent."),
        "criteria": {
            "true": "the excerpt literally contains the failure evidence",
            "false": "no literal evidence in this excerpt"},
    }
    return one


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--signature", required=True,
                        help="failing signature: test name, diagnostic fragment, "
                             "or a one-paragraph failure description")
    parser.add_argument("--window", required=True,
                        help="commit range BASE..HEAD or comma-separated SHAs")
    parser.add_argument("--with-diff", type=int, default=0, metavar="LINES",
                        help="include up to N diff lines per candidate")
    options = parser.parse_args()

    if os.environ.get("OMEGA_JEV_OFFLINE", "").strip() == "1":
        return 2
    key = jev_key()
    if not key:
        print("no TYPESAFE_API_KEY", file=sys.stderr)
        return 2

    cands = candidates(options.window)
    if not cands:
        print("empty candidate window", file=sys.stderr)
        return 1
    for cand in cands:
        if options.with_diff:
            raw = git("diff", f"{cand['sha']}^1", cand["sha"],
                      "--unified=1", "--", ".", ":(exclude)*lock*")
            # Added lines carry the defect evidence; keep a per-hunk header
            # plus added lines so large diffs still surface them first.
            kept = [line for line in raw.splitlines()
                    if line.startswith(("+++", "@@", "+", "diff "))]
            cand["diff_added_lines"] = "\n".join(
                kept)[:options.with_diff * 80]

    state = {"failing_signature": options.signature,
             "candidate_commits": cands}
    request = {"model": JEV_MODEL, "state": state,
               "questions": questions(len(cands))}
    try:
        answers = jev_post(request, key).get("answers", {})
    except Exception as error:
        print(f"jev unavailable: {type(error).__name__}", file=sys.stderr)
        return 2

    verdicts = {}
    no_suspect = answers.get("no_suspect", {}).get("noul", 0.5)
    flagged = []
    for index, cand in enumerate(cands):
        verdict = answers.get(f"evidence_{index}", {}).get("choice", "inconclusive")
        verdicts[cand["sha"]] = verdict
        if verdict in ("contains", "inconclusive"):
            flagged.append(cand)

    # Solo re-asks on flagged candidates: batch labels are contaminated by
    # the worst member (CLASSIFY.md), and a batch 'contains' often belongs
    # to the one guilty commit contaminating innocent neighbors.
    solo = {}
    for cand in flagged[:10]:
        request = {"model": JEV_MODEL,
                   "state": {"failing_signature": options.signature,
                             "candidate_commits": [cand]},
                   "questions": solo_questions()}
        try:
            one = jev_post(request, key).get("answers", {})
            solo[cand["sha"]] = (
                one.get("evidence_0", {}).get("choice", "inconclusive"),
                one.get("introduces_0", {}).get("noul", 0.5))
        except Exception:
            solo[cand["sha"]] = ("inconclusive", 0.5)

    rank_verdict = {"contains": 2, "inconclusive": 1, "absent": 0}

    def rank_key(cand):
        if cand["sha"] in solo:
            verdict, score = solo[cand["sha"]]
            return (rank_verdict.get(verdict, 1), score)
        return (rank_verdict.get(verdicts[cand["sha"]], 1), 0.5)

    ranked = sorted(cands, key=rank_key, reverse=True)

    print(f"failing signature: {options.signature}")
    print(f"window: {len(cands)} candidates | "
          f"cause-outside-window {no_suspect:.2f}")
    for cand in ranked:
        sha = cand["sha"]
        if sha in solo:
            verdict, score = solo[sha]
            flag = "SUSPECT" if verdict == "contains" else \
                ("MAYBE  " if verdict == "inconclusive" else "       ")
            print(f"  {flag} {verdict:12s} {score:.2f} {sha} "
                  f"{cand['subject'][:72]}")
        else:
            verdict = verdicts[sha]
            flag = "MAYBE  " if verdict == "inconclusive" else "       "
            print(f"  {flag} {verdict:12s}  --   {sha} "
                  f"{cand['subject'][:72]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
