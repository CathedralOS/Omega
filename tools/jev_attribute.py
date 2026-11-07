#!/usr/bin/env python3
"""jev-attribute: rank candidate commits by plausibility of causing a failure.

Given a failing signature (test name, diagnostic, crash shape) and a candidate
commit window, ask Jev which commits could plausibly produce the failure —
the blame-rank a bisect spends wall-clock on. Solo re-asks localize flagged
candidates (batch labels are contaminated by the worst member, per
build/experiments/failure-triage/CLASSIFY.md).

Usage:
    jev_attribute.py --signature "TEXT" --window BASE..HEAD [--with-diff 60]
        [--paths P1,P2,...]
    jev_attribute.py --signature "TEXT" --window SHA1,SHA2,...

A large commit's deciding hunk falls outside the --with-diff excerpt cap
unless --paths names its file — the window selects commits, --paths selects
evidence.

Silent no-op exit 2 without TYPESAFE_API_KEY; OMEGA_JEV_OFFLINE=1 suppresses.
Key resolution: TYPESAFE_API_KEY, build/typesafe.env.txt (in this or the
main checkout), ~/.config/typesafe/typesafe.env.txt.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

try:
    import typesafe_key
except ImportError:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import typesafe_key


# The API rejects over-limit payloads with HTTP 400 max_tokens_exceeded;
# 164 KB failed, so keep each request's candidate list well under it.
BATCH_BUDGET_BYTES = 60_000


def git(*args) -> str:
    return subprocess.run(["git"] + list(args), capture_output=True,
                          text=True, encoding="utf-8", errors="replace",
                          timeout=30, check=True).stdout


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


def batches(cands: list) -> list:
    """Consecutive batches in window order whose JSON-encoded
    candidate_commits stays under BATCH_BUDGET_BYTES; one oversize
    candidate still forms its own batch."""
    result, current, size = [], [], 0
    for cand in cands:
        encoded = len(json.dumps(cand).encode())
        if current and size + encoded > BATCH_BUDGET_BYTES:
            result.append(current)
            current, size = [], 0
        current.append(cand)
        size += encoded
    if current:
        result.append(current)
    return result


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
                "is 'absent'. A candidate with `touches_excerpt_scope` "
                "false changed nothing under the files the caller "
                "implicated; answer 'absent' for it unless the signature "
                "itself names that commit."),
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
    parser.add_argument("--paths", metavar="P1,P2,...",
                        help="restrict diff excerpts to the files the "
                             "signature implicates; the window still covers "
                             "every commit")
    options = parser.parse_args()

    if os.environ.get("OMEGA_JEV_OFFLINE", "").strip() == "1":
        return 2
    key = typesafe_key.find_key(None)
    if not key:
        print("no TYPESAFE_API_KEY", file=sys.stderr)
        return 2

    cands = candidates(options.window)
    if not cands:
        print("empty candidate window", file=sys.stderr)
        return 1
    pathspecs = [path.strip() for path in (options.paths or "").split(",")
                 if path.strip()] or [".", ":(exclude)*lock*"]
    for cand in cands:
        if options.with_diff:
            raw = git("diff", f"{cand['sha']}^1", cand["sha"],
                      "--unified=1", "--", *pathspecs)
            # Added lines carry the defect evidence; keep a per-hunk header
            # plus added lines so large diffs still surface them first.
            kept = [line for line in raw.splitlines()
                    if line.startswith(("+++", "@@", "+", "diff "))]
            excerpt = "\n".join(kept)[:options.with_diff * 80]
            cand["touches_excerpt_scope"] = bool(excerpt)
            cand["diff_added_lines"] = (
                excerpt or "(no changes in the excerpt scope)")

    groups = batches(cands)
    verdicts = {}
    no_suspect = 1.0
    for group in groups:
        state = {"failing_signature": options.signature,
                 "candidate_commits": group}
        request = {"model": typesafe_key.MODEL, "state": state,
                   "questions": questions(len(group))}
        try:
            response = typesafe_key.post(typesafe_key.ENDPOINT, key, request)
        except Exception as error:
            print(f"jev unavailable: {type(error).__name__}", file=sys.stderr)
            return 2
        if response is None:
            print("jev unavailable: no response", file=sys.stderr)
            return 2
        batch_answers = response.get("answers", {})
        # The cause is outside the window only if every batch says so.
        no_suspect = min(no_suspect,
                         batch_answers.get("no_suspect", {}).get("noul", 0.5))
        for index, cand in enumerate(group):
            verdicts[cand["sha"]] = (batch_answers
                                     .get(f"evidence_{index}", {})
                                     .get("choice", "inconclusive"))

    flagged = [cand for cand in cands
               if verdicts[cand["sha"]] in ("contains", "inconclusive")]

    # Solo re-asks on flagged candidates: batch labels are contaminated by
    # the worst member (CLASSIFY.md), and a batch 'contains' often belongs
    # to the one guilty commit contaminating innocent neighbors. A candidate
    # outside the excerpt scope is never re-asked.
    solo = {}
    soloable = [cand for cand in flagged
                if cand.get("touches_excerpt_scope", True)]
    for cand in soloable[:10]:
        request = {"model": typesafe_key.MODEL,
                   "state": {"failing_signature": options.signature,
                             "candidate_commits": [cand]},
                   "questions": solo_questions()}
        try:
            one = typesafe_key.post(typesafe_key.ENDPOINT, key, request)
        except Exception:
            one = None
        if one is None:
            solo[cand["sha"]] = ("inconclusive", 0.5)
        else:
            solo_answers = one.get("answers", {})
            solo[cand["sha"]] = (
                solo_answers.get("evidence_0", {}).get("choice", "inconclusive"),
                solo_answers.get("introduces_0", {}).get("noul", 0.5))

    rank_verdict = {"contains": 2, "inconclusive": 1, "absent": 0}

    def rank_key(cand):
        scope = cand.get("touches_excerpt_scope", True)
        if cand["sha"] in solo:
            verdict, score = solo[cand["sha"]]
            return (rank_verdict.get(verdict, 1), score, scope)
        return (rank_verdict.get(verdicts[cand["sha"]], 1), 0.5, scope)

    ranked = sorted(cands, key=rank_key, reverse=True)

    print(f"failing signature: {options.signature}")
    print(f"window: {len(cands)} candidates in {len(groups)} batch(es) | "
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
