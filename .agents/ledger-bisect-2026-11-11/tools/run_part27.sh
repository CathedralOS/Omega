#!/bin/bash
# Strict part-27 bisect. Checkpointed under /tmp/ledger-bisect-part27.
# Resume-safe: completed results/*.json are skipped; a recorded good sha is reused.
set -u
PIN=51e8c45c793007f211d0d9baee7de16767172444
REPO=/agent/repos/omega
STATE=/tmp/ledger-bisect-part27
TOOLS_SRC="$REPO/.agents/ledger-bisect-2026-11-11/tools"
TOOLS="$STATE/tools"
mkdir -p "$STATE/results" "$STATE/pin" "$TOOLS"
cp -a "$TOOLS_SRC/." "$TOOLS/"
chmod +x "$TOOLS/"*.sh "$TOOLS/"*.py || true
cd "$REPO"

progress() { printf '%s %s\n' "$(date -u +%H:%M:%S)" "$*" | tee -a "$STATE/progress.txt" >&2; }

commit_cluster() {
  local cid="$1"
  git bisect reset >/dev/null 2>&1 || true
  git -c advice.detachedHead=false checkout -f work/ledger-bisect-part-27
  cp -a "$STATE/results/." "$REPO/.agents/ledger-bisect-2026-11-11/results/" 2>/dev/null || mkdir -p "$REPO/.agents/ledger-bisect-2026-11-11/results"
  cp -a "$STATE/results/." "$REPO/.agents/ledger-bisect-2026-11-11/results/"
  cp "$STATE/measured.json" "$REPO/.agents/ledger-bisect-2026-11-11/measured.json" 2>/dev/null || true
  python3 "$TOOLS/write_report.py"
  git add .agents/ledger-bisect-2026-11-11/part-27.md .agents/ledger-bisect-2026-11-11/part-27.tsv \
    .agents/ledger-bisect-2026-11-11/results .agents/ledger-bisect-2026-11-11/measured.json \
    .agents/ledger-bisect-2026-11-11/tools
  if git diff --cached --quiet; then
    progress "$cid nothing to commit"
    return 0
  fi
  git commit -m "repo: ledger bisect part 27 cluster ${cid}"
  local n=0
  until git push origin work/ledger-bisect-part-27; do
    n=$((n + 1))
    if [[ $n -ge 4 ]]; then
      progress "$cid push failed"
      return 1
    fi
    sleep $((4 * n))
  done
  progress "$cid pushed"
}

run_code() {
  local code=0
  bash "$TOOLS/bisect_witness.sh" >"$STATE/last-witness.log" 2>&1 || code=$?
  printf '%s' "$code"
}

ensure_pin_measure() {
  if [[ -f "$STATE/measured.json" ]]; then
    progress "PIN measure already present"
    return 0
  fi
  progress "measuring PIN with fresh target"
  git bisect reset >/dev/null 2>&1 || true
  git -c advice.detachedHead=false checkout -f "$PIN"
  local sha="$PIN"
  export CARGO_TARGET_DIR="/tmp/bisect-target-${sha}"
  export CARGO_PROFILE_DEV_OPT_LEVEL=1 OMEGA_CORPUS_JOBS=1 CARGO_BUILD_JOBS=4 MBX_SCHEDULER_CPUS=4
  rm -rf /tmp/bisect-target-*
  mkdir -p "$CARGO_TARGET_DIR"
  nice -n 19 mbx build -p omega < /dev/null >"$STATE/pin-build.log" 2>&1 || {
    progress "PIN build failed"
    return 1
  }
  nice -n 19 mbx test -p compiler --test corpus_runner --no-run < /dev/null >>"$STATE/pin-build.log" 2>&1 || {
    progress "PIN runner build failed"
    return 1
  }
  local runner
  runner=$(find "$CARGO_TARGET_DIR/debug/build/compiler" -type f -name 'corpus_runner-*' -perm -111 ! -name '*.d' | head -1)
  [[ -x "$runner" ]] || { progress "PIN runner missing"; return 1; }
  local filter='fail/contracts/target_family_recursive_guarantee,fail/contracts/target_family_unshared_result,fail/targets/sibling_body_type_error_rejected,fail/targets/unrealized_conformance_rejected,pass/targets/private_target_supplies_remain_disclosed,pass/targets/retained_target_family_bodies,pass/targets/retained_target_provider_family_exit,fail/borrows/aliased_forwarded_placed_views,pass/borrows/runtime_stored_reference_case_residual_retirement_exit,run/borrows/terminal_case_completed_observation,pass/traits/prepared_callable_nested_borrow_paths_exit,pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit,pass/facts/case_payload_exclusive_loan_through_other_machine_exit'
  export OMEGA_CORPUS_ROOT="$REPO" OMEGA_LIBRARY_ROOT="$REPO/source/library"
  export OMEGA_CORPUS_FIXTURE_FILTER="$filter" OMEGA_CORPUS_RECORDS="$STATE/pin-records.txt"
  rm -f "$STATE/pin-records.txt"
  nice -n 19 "$runner" < /dev/null >"$STATE/pin-run.log" 2>&1 || true
  # Ledger at the PIN, not whatever branch commit is checked out later.
  git show "${PIN}:tests/omega/corpus_outcomes.txt" >"$STATE/corpus_outcomes.txt"
  MEASURED_OUT="$STATE/measured.json" python3 "$TOOLS/measure_pin.py" \
    "$STATE/pin-records.txt" "$STATE/corpus_outcomes.txt" "$STATE/pin"
  progress "PIN measure done"
}

fixture_path() {
  local fix="$1"
  if [[ -e "$REPO/tests/omega/$fix.omg" ]]; then
    echo "tests/omega/$fix.omg"
  elif [[ -d "$REPO/tests/omega/$fix" ]]; then
    echo "tests/omega/$fix"
  else
    echo ""
  fi
}

find_good() {
  local witness="$1"
  local cid="$2"
  local cache="$STATE/${cid}-good.txt"
  if [[ -s "$cache" ]]; then
    cat "$cache"
    return 0
  fi
  export BISECT_WITNESS="$witness"
  export BISECT_WITNESS_PIN="$STATE/pin/$(echo "$witness" | tr '/' '_').json"
  export BISECT_STEP_LOG="$STATE/${cid}-findgood.tsv"
  local depth=1
  local path
  path="$(fixture_path "$witness")"
  local add=""
  if [[ -n "$path" ]]; then
    add=$(git log --diff-filter=A --format=%H "$PIN" -- "$path" | tail -1 || true)
  fi
  while [[ $depth -le 65536 ]]; do
    local sha
    if ! sha=$(git rev-parse "${PIN}~${depth}" 2>/dev/null); then
      progress "$cid no ancestor at depth $depth"
      break
    fi
    if [[ -n "$path" ]] && ! git cat-file -e "${sha}:${path}" 2>/dev/null; then
      progress "$cid walked past fixture at depth $depth"
      break
    fi
    git -c advice.detachedHead=false checkout -f "$sha" >/dev/null 2>&1
    local code
    code=$(run_code)
    progress "$cid find-good depth=$depth $sha code=$code"
    if [[ "$code" == "0" ]]; then
      printf '%s\n' "$sha" | tee "$cache"
      return 0
    fi
    depth=$((depth * 2))
  done
  if [[ -n "$add" ]]; then
    git -c advice.detachedHead=false checkout -f "$add" >/dev/null 2>&1
    local code
    code=$(run_code)
    progress "$cid fixture-add $add code=$code"
    if [[ "$code" == "0" ]]; then
      printf '%s\n' "$add" | tee "$cache"
      return 0
    fi
    printf 'never-good %s\n' "$add" | tee "$cache"
    return 0
  fi
  printf 'never-good unknown\n' | tee "$cache"
}

triplicate() {
  local sha="$1"
  local witness="$2"
  local cid="$3"
  local label="$4"
  export BISECT_WITNESS="$witness"
  export BISECT_WITNESS_PIN="$STATE/pin/$(echo "$witness" | tr '/' '_').json"
  export BISECT_STEP_LOG="$STATE/${cid}-verify.tsv"
  git -c advice.detachedHead=false checkout -f "$sha" >/dev/null 2>&1
  local out=""
  local i code
  for i in 1 2 3; do
    code=$(run_code)
    out+="${code}"
    [[ $i -lt 3 ]] && out+=","
    progress "$cid $label run$i $sha $code"
  done
  printf '%s' "$out"
}

member_once() {
  local sha="$1"
  local witness="$2"
  local cid="$3"
  local pin="$STATE/pin/$(echo "$witness" | tr '/' '_').json"
  if [[ ! -f "$pin" ]]; then
    printf '0'
    return 0
  fi
  export BISECT_WITNESS="$witness"
  export BISECT_WITNESS_PIN="$pin"
  export BISECT_STEP_LOG="$STATE/${cid}-members.tsv"
  git -c advice.detachedHead=false checkout -f "$sha" >/dev/null 2>&1
  run_code
}

pr_of() {
  local sha="$1"
  local text
  text=$(git log -1 --format='%s%n%b' "$sha")
  if [[ "$text" =~ \#([0-9]+) ]]; then
    printf '%s' "${BASH_REMATCH[1]}"
  else
    printf 'none'
  fi
}

bisect_cluster() {
  local cid="$1"
  local witness="$2"
  shift 2
  local members=("$@")
  if [[ -f "$STATE/results/${cid}.json" ]]; then
    progress "$cid already complete"
    return 0
  fi
  local pinjson="$STATE/pin/$(echo "$witness" | tr '/' '_').json"
  if [[ ! -f "$pinjson" ]]; then
    progress "$cid witness not broken on fresh PIN; skipping bisect"
    python3 - "$cid" "$witness" "${members[@]}" <<'PY'
import json, sys
from pathlib import Path
cid, witness, *members = sys.argv[1:]
Path("/tmp/ledger-bisect-part27/results").mkdir(parents=True, exist_ok=True)
Path(f"/tmp/ledger-bisect-part27/results/{cid}.json").write_text(json.dumps({
    "cluster": cid, "witness": witness, "members": members,
    "status": "fixed-on-pin", "good": "", "breaking": "", "pr": "",
    "parent_runs": [], "bad_runs": [], "mechanism": "fixed on PIN under a fresh target",
    "note": "Fresh commit-scoped PIN measurement meets the ledger expectation.",
}, indent=2))
PY
    commit_cluster "$cid"
    return 0
  fi
  progress "$cid searching good"
  local good_line good
  good_line=$(find_good "$witness" "$cid")
  good=$(printf '%s' "$good_line" | tail -1)
  if [[ "$good" == never-good* ]]; then
    local since="${good#never-good }"
    python3 - "$cid" "$witness" "$since" "${members[@]}" <<'PY'
import json, sys
from pathlib import Path
cid, witness, since, *members = sys.argv[1:]
Path(f"/tmp/ledger-bisect-part27/results/{cid}.json").write_text(json.dumps({
    "cluster": cid, "witness": witness, "members": members,
    "status": "never-good", "good": "", "breaking": "", "pr": "",
    "since": since, "parent_runs": [], "bad_runs": [],
    "mechanism": "mechanism not established",
    "note": f"never-good since {since}",
}, indent=2))
PY
    commit_cluster "$cid"
    return 0
  fi
  progress "$cid good=$good"
  export BISECT_WITNESS="$witness"
  export BISECT_WITNESS_PIN="$pinjson"
  export BISECT_STEP_LOG="$STATE/${cid}-bisect.tsv"
  git bisect reset >/dev/null 2>&1 || true
  git -c advice.detachedHead=false checkout -f work/ledger-bisect-part-27 >/dev/null 2>&1 || true
  git bisect start --first-parent "$PIN" "$good"
  set +e
  git bisect run bash "$TOOLS/bisect_witness.sh" >"$STATE/${cid}-bisect.log" 2>&1
  local brc=$?
  set -e
  progress "$cid bisect exit=$brc"
  local breaking
  breaking=$(sed -n 's/^\([0-9a-f]\{40\}\) is the first bad commit$/\1/p' "$STATE/${cid}-bisect.log" | tail -1)
  if [[ -z "$breaking" ]]; then
    python3 - "$cid" "$witness" "$good" "$brc" "${members[@]}" <<'PY'
import json, sys
from pathlib import Path
cid, witness, good, brc, *members = sys.argv[1:]
Path(f"/tmp/ledger-bisect-part27/results/{cid}.json").write_text(json.dumps({
    "cluster": cid, "witness": witness, "members": members, "good": good,
    "status": "bisect-failed", "breaking": "", "pr": "",
    "parent_runs": [], "bad_runs": [],
    "mechanism": "mechanism not established",
    "note": f"git bisect run did not name a first bad commit (exit {brc}). See {cid}-bisect.log.",
}, indent=2))
PY
    git bisect reset >/dev/null 2>&1 || true
    commit_cluster "$cid"
    return 0
  fi
  git bisect reset >/dev/null 2>&1 || true
  local parent
  parent=$(git rev-parse "${breaking}^1")
  progress "$cid first-bad=$breaking parent=$parent"
  local bad_runs parent_runs
  bad_runs=$(triplicate "$breaking" "$witness" "$cid" bad)
  parent_runs=$(triplicate "$parent" "$witness" "$cid" parent)
  # If the parent is not a clean good, walk every first-parent commit from good
  # up to breaking and record the first non-match and the first PIN-class hit.
  local stop_matching="$breaking"
  local class_commit="$breaking"
  if [[ "$parent_runs" != "0,0,0" ]]; then
    progress "$cid parent not 0,0,0 ($parent_runs); walking backward to the nearest good"
    : >"$STATE/${cid}-backwalk.tsv"
    printf '%s\t1\n' "$breaking" >>"$STATE/${cid}-backwalk.tsv"
    local c="$parent" code
    while [[ "$c" != "$good" ]]; do
      git -c advice.detachedHead=false checkout -f "$c" >/dev/null 2>&1
      code=$(run_code)
      printf '%s\t%s\n' "$c" "$code" >>"$STATE/${cid}-backwalk.tsv"
      progress "$cid back $c $code"
      if [[ "$code" == "0" ]]; then
        break
      fi
      c=$(git rev-parse "${c}^1" 2>/dev/null) || break
    done
    local picked
    picked=$(python3 - "$STATE/${cid}-backwalk.tsv" <<'PY'
import sys
rows = [ln.split("\t") for ln in open(sys.argv[1]) if ln.strip()]
# file is newest first; reverse to oldest first
rows = list(reversed(rows))
stop = cls = parent = ""
seen_good = False
for sha, code in ((a, b.strip()) for a, b in rows):
    if code == "0":
        seen_good = True
        parent = sha
        stop = cls = ""
        continue
    if seen_good and not stop:
        stop = sha
    if seen_good and code == "1" and not cls:
        cls = sha
print(f"{parent}\t{stop}\t{cls}")
PY
)
    local new_parent new_stop new_class
    IFS=$'\t' read -r new_parent new_stop new_class <<<"$picked"
    [[ -n "$new_stop" ]] && stop_matching="$new_stop"
    if [[ -n "$new_class" ]]; then
      breaking="$new_class"
      parent="${new_parent:-$(git rev-parse "${breaking}^1")}"
      bad_runs=$(triplicate "$breaking" "$witness" "$cid" bad)
      parent_runs=$(triplicate "$parent" "$witness" "$cid" parent)
    fi
  fi
  declare -A member_parent member_bad
  local m pc bc
  for m in "${members[@]}"; do
    pc=$(member_once "$parent" "$m" "$cid")
    bc=$(member_once "$breaking" "$m" "$cid")
    member_parent["$m"]=$pc
    member_bad["$m"]=$bc
    progress "$cid member $m parent=$pc bad=$bc"
  done
  local pr subj goodsubj mech
  pr=$(pr_of "$breaking")
  subj=$(git log -1 --format='%s' "$breaking")
  goodsubj=$(git log -1 --format='%s' "$good")
  local diag
  diag=$(python3 -c 'import json;print(json.load(open("'"$pinjson"'"))["pin_first_diagnostic"])')
  mech=$(python3 "$TOOLS/mechanism_hunk.py" "$breaking" \
    "reserved_target" "archive_target" "canonical target" "still active" \
    "executable body" "requires contract" "selected case" "live case" \
    $(printf '%s\n' "$diag" | tr -c 'A-Za-z0-9_' ' ' ))
  python3 - "$cid" "$witness" "$good" "$goodsubj" "$breaking" "$subj" "$pr" "$parent" \
    "$bad_runs" "$parent_runs" "$stop_matching" "$mech" "${members[@]}" <<'PY'
import json, sys
from pathlib import Path
(cid, witness, good, goodsubj, breaking, subj, pr, parent,
 bad_runs, parent_runs, stop, mech, *members) = sys.argv[1:]
# member codes are passed via env-less trailing pairs? The shell writes them below.
Path("/tmp/ledger-bisect-part27/partial.json").write_text(json.dumps({
    "cluster": cid, "witness": witness, "members": members,
    "good": good, "good_subject": goodsubj,
    "breaking": breaking, "breaking_subject": subj, "pr": pr,
    "parent": parent,
    "bad_runs": [int(x) for x in bad_runs.split(",") if x != ""],
    "parent_runs": [int(x) for x in parent_runs.split(",") if x != ""],
    "stop_matching": stop, "mechanism": mech,
    "status": "bisected",
}, indent=2))
PY
  : >"$STATE/member-codes.tsv"
  for m in "${members[@]}"; do
    printf '%s\t%s\t%s\n' "$m" "${member_parent[$m]}" "${member_bad[$m]}" >>"$STATE/member-codes.tsv"
  done
  python3 - <<'PY'
import json
from pathlib import Path
partial = json.loads(Path("/tmp/ledger-bisect-part27/partial.json").read_text())
raw = Path("/tmp/ledger-bisect-part27/member-codes.tsv").read_text().splitlines()
runs = {}
for line in raw:
    if not line.strip():
        continue
    name, parent, bad = line.split("\t")
    runs[name] = {"at_parent": int(parent), "at_bad": int(bad)}
partial["member_runs"] = runs
notes = []
if partial["bad_runs"] != [1, 1, 1] or partial["parent_runs"] != [0, 0, 0]:
    notes.append(
        f"verification bad={partial['bad_runs']} parent={partial['parent_runs']} (required 1,1,1 and 0,0,0)"
    )
    partial["status"] = "unstable"
splits = [n for n, info in runs.items() if not (info["at_parent"] == 0 and info["at_bad"] == 1)]
if splits:
    notes.append("members that did not flip with the witness: " + ", ".join(splits))
partial["note"] = " ".join(notes)
Path(f"/tmp/ledger-bisect-part27/results/{partial['cluster']}.json").write_text(json.dumps(partial, indent=2))
PY
  commit_cluster "$cid"
}

progress "driver start"
ensure_pin_measure || { progress "measure failed"; exit 1; }

# Clusters: id, witness, member list (comma-separated)
while IFS=$'\t' read -r cid witness members; do
  [[ -z "$cid" ]] && continue
  IFS=',' read -r -a arr <<<"$members"
  # write member codes into /tmp inside bisect via a wrapper: patch by exporting STATE
  bisect_cluster "$cid" "$witness" "${arr[@]}"
done <<'EOF'
c01	pass/targets/private_target_supplies_remain_disclosed	pass/targets/private_target_supplies_remain_disclosed,pass/targets/retained_target_provider_family_exit
c02	pass/targets/retained_target_family_bodies	pass/targets/retained_target_family_bodies
c03	pass/borrows/runtime_stored_reference_case_residual_retirement_exit	pass/borrows/runtime_stored_reference_case_residual_retirement_exit
c04	run/borrows/terminal_case_completed_observation	run/borrows/terminal_case_completed_observation
c05	pass/traits/prepared_callable_nested_borrow_paths_exit	pass/traits/prepared_callable_nested_borrow_paths_exit
c06	pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit	pass/facts/saved_observation_snapshot_indexed_handback_other_machine_exit
c07	pass/facts/case_payload_exclusive_loan_through_other_machine_exit	pass/facts/case_payload_exclusive_loan_through_other_machine_exit
EOF

progress "ALL_DONE"
printf 'ALL_DONE\n' >"$STATE/ALL_DONE"
git -c advice.detachedHead=false checkout -f work/ledger-bisect-part-27 >/dev/null 2>&1 || true
