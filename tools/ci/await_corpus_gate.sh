#!/bin/bash
# Make the required `compiler-check` status carry the corpus gate's verdict.
#
# Main's ruleset requires `compiler-check`, and changing which checks are
# required is an owner operation. So instead of a new required check, the
# compiler-check job ends by waiting for this head commit's run of
# .github/workflows/corpus-gate.yml and failing when that run fails. A pull
# request that breaks corpus rows then cannot merge under today's protection.
#
#   REPO=owner/name PR=123 HEAD_SHA=<sha> GH_TOKEN=... bash tools/ci/await_corpus_gate.sh
#
# - A draft passes at once: it cannot merge, and the gate runs when it is
#   marked ready (compiler.yml also runs on ready_for_review).
# - A run whose `plan` job was skipped is a draft run; a newer run for the
#   same commit is expected, so it is not accepted as a verdict.
# - A cancelled run was superseded; keep waiting for the newer one.
# - A ledger- or Markdown-only pull request's gate run skips its shards and
#   finishes in under a minute, so it stays fast here too.
set -euo pipefail
: "${REPO:?}" "${PR:?}" "${HEAD_SHA:?}"
deadline=$(( $(date +%s) + ${WAIT_SECS:-3300} ))
seen=""
while :; do
  if [ "$(gh api "repos/$REPO/pulls/$PR" --jq .draft)" = true ]; then
    echo "Draft pull request: the corpus gate decides when it is marked ready."
    exit 0
  fi
  run=$(gh api "repos/$REPO/actions/workflows/corpus-gate.yml/runs?head_sha=$HEAD_SHA&per_page=30" \
    --jq '[.workflow_runs[] | select(.event == "pull_request")] | sort_by(.run_number) | last // empty | "\(.id) \(.status) \(.conclusion) \(.html_url)"')
  if [ -n "$run" ]; then
    read -r id status conclusion url <<<"$run"
    if [ "$run" != "$seen" ]; then echo "corpus gate run $url: $status ${conclusion/null/}"; seen=$run; fi
    if [ "$status" = completed ]; then
      plan=$(gh api "repos/$REPO/actions/runs/$id/jobs" --jq '[.jobs[] | select(.name == "plan")][0].conclusion // "missing"')
      case "$conclusion:$plan" in
        success:success)
          echo "The corpus gate passed for $HEAD_SHA."; exit 0 ;;
        success:*|cancelled:*|skipped:*)
          : ;; # a draft run or a superseded one; wait for the run that decides
        *)
          echo "::error::The corpus gate failed for $HEAD_SHA ($conclusion): $url"
          echo "Its shard summaries name each row that got worse than tests/omega/corpus_outcomes.txt."
          echo "After fixing or re-running it, re-run this job so compiler-check reads the new verdict."
          exit 1 ;;
      esac
    fi
  fi
  if [ "$(date +%s)" -ge "$deadline" ]; then
    echo "::error::No corpus gate verdict for $HEAD_SHA within the wait; re-run this job once the gate finishes."
    exit 1
  fi
  sleep 30
done
