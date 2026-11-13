#!/usr/bin/env python3
"""Ledger bisect one cluster: exponential good, git bisect break, triple verify, member split."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
# Neither the measured commit nor #2228 is an admissible break edge.
FORBIDDEN_EDGES = {
    PIN,
    "f3612364ffbb9489d7be1409397df6867fcbebc6",
}
ROOT = Path(__file__).resolve().parents[2]
AG = Path(__file__).resolve().parent
TOOLHOME = Path("/tmp/ledger-bisect-part-21-tools")
LOGROOT = Path("/tmp/ledger-bisect-logs")
WITNESS_SH = TOOLHOME / "tools" / "bisect_witness.sh"


def run(cmd: list[str], *, cwd: Path = ROOT, check: bool = True, env: dict | None = None) -> subprocess.CompletedProcess:
    e = os.environ.copy()
    if env:
        e.update(env)
    return subprocess.run(cmd, cwd=cwd, env=e, text=True, check=check)


def flush_logs(cluster_id: str, out_lines: list[str], findlog: list[str]) -> None:
    # Outside the worktree: git checkout during bisect must not restore a tracked log.
    LOGROOT.mkdir(parents=True, exist_ok=True)
    (LOGROOT / f"find-good-{cluster_id}.log").write_text("\n".join(findlog) + "\n")
    if out_lines:
        (LOGROOT / f"bisect-{cluster_id}.log").write_text("\n".join(out_lines) + "\n\n" + "\n".join(findlog) + "\n")


def setup_tools() -> None:
    TOOLHOME.mkdir(parents=True, exist_ok=True)
    (TOOLHOME / "tools").mkdir(exist_ok=True)
    run(["cp", "-a", f"{AG}/tools/.", f"{TOOLHOME}/tools/"])
    run(["cp", "-a", f"{AG}/prepare_witness.sh", f"{AG}/part21_observed.txt", str(TOOLHOME) + "/"])


def prepare_spec(witness: str) -> Path:
    spec = subprocess.check_output(
        ["bash", str(TOOLHOME / "prepare_witness.sh"), witness],
        env={**os.environ, "BISECT_REPO_ROOT": str(ROOT)},
        text=True,
    ).strip()
    return Path(spec)


def first_parent_chain(intro: str, pin: str) -> list[str]:
    revs: list[str] = []
    cur = pin
    while True:
        revs.append(cur)
        if cur == intro:
            break
        cur = subprocess.check_output(["git", "rev-parse", f"{cur}^1"], text=True).strip()
    revs.reverse()
    return revs


def fixture_intro(witness: str) -> str:
    rel = f"tests/omega/{witness}/build.omg"
    lines = subprocess.check_output(
        ["git", "log", "--reverse", "--format=%H", "--", rel], text=True, cwd=ROOT
    ).splitlines()
    if not lines:
        raise SystemExit(f"no fixture intro for {witness}")
    return lines[0]


def witness_env(witness: str, spec: Path, rev: str, mode: str = "outcome") -> dict[str, str]:
    # The witness script keys the cache on HEAD. This dir is only the default for that rev.
    td = Path(f"/tmp/bisect-target-{rev}")
    return {
        "BISECT_WITNESS": witness,
        "BISECT_REPO_ROOT": str(ROOT),
        "BISECT_SPEC": str(spec),
        "BISECT_LEDGER": "/tmp/ledger-bisect-pin-corpus_outcomes.txt",
        "BISECT_MODE": mode,
        "BISECT_TARGET_DIR": str(td),
        "CARGO_TARGET_DIR": str(td),
        "CARGO_PROFILE_DEV_OPT_LEVEL": "1",
        "OMEGA_CORPUS_JOBS": "1",
        "CARGO_BUILD_JOBS": "4",
        "MBX_SCHEDULER_CPUS": "4",
    }


def test_witness(
    witness: str,
    spec: Path,
    rev: str,
    log: list[str],
    label: str,
    cluster_id: str,
    mode: str = "outcome",
) -> int:
    run(["git", "checkout", "-f", "-q", rev], check=True)
    env = witness_env(witness, spec, rev, mode)
    proc = run(["nice", "-n", "19", str(WITNESS_SH)], check=False, env=env)
    oneline = subprocess.check_output(["git", "log", "-1", "--oneline", rev], text=True).strip()
    log.append(f"{label} {rev} exit={proc.returncode} {oneline}")
    flush_logs(cluster_id, [], log)
    return proc.returncode


def find_latest_good(
    chain: list[str], witness: str, spec: Path, log: list[str], cluster_id: str
) -> str | None:
    pin_i = len(chain) - 1
    if test_witness(witness, spec, chain[pin_i], log, "pin", cluster_id) != 1:
        raise SystemExit("PIN does not reproduce regression (exit != 1)")
    step = 1
    anchor: int | None = None
    while pin_i - step >= 0:
        idx = pin_i - step
        ec = test_witness(witness, spec, chain[idx], log, "good_exp", cluster_id)
        if ec == 0:
            anchor = idx
            break
        step *= 2
    if anchor is None:
        return None
    lo, hi = anchor, pin_i
    while lo < hi - 1:
        mid = (lo + hi) // 2
        ec = test_witness(witness, spec, chain[mid], log, "good_refine", cluster_id)
        if ec == 0:
            lo = mid
        elif ec == 1:
            hi = mid
        else:
            hi = mid
    ec = test_witness(witness, spec, chain[lo], log, "good_final", cluster_id)
    return chain[lo] if ec == 0 else None


def triple_verify(
    witness: str,
    spec: Path,
    rev: str,
    want: int,
    log: list[str],
    label: str,
    cluster_id: str,
    mode: str = "outcome",
) -> tuple[bool, list[int]]:
    codes: list[int] = []
    for i in range(3):
        ec = test_witness(witness, spec, rev, log, f"{label}_{i+1}", cluster_id, mode)
        codes.append(ec)
    return all(c == want for c in codes), codes


def git_bisect_break(
    witness: str,
    spec: Path,
    good: str,
    bad: str,
    log: list[str],
    cluster_id: str,
    mode: str = "outcome",
) -> str:
    run(["git", "bisect", "reset"], check=False)
    env = os.environ.copy()
    env.update(witness_env(witness, spec, bad, mode))
    subprocess.run(
        ["git", "bisect", "start", "--first-parent", bad, good],
        cwd=ROOT,
        check=True,
        env=env,
    )
    proc = subprocess.run(
        ["git", "bisect", "run", str(WITNESS_SH)],
        cwd=ROOT,
        env=env,
        text=True,
    )
    blog = subprocess.check_output(["git", "bisect", "log"], text=True, cwd=ROOT)
    LOGROOT.mkdir(parents=True, exist_ok=True)
    tag = "outcome" if mode == "outcome" else "class"
    (LOGROOT / f"git-bisect-{cluster_id}-{tag}.log").write_text(blog)
    if "--first-parent" not in blog.splitlines()[2:3] and "--first-parent" not in blog:
        raise SystemExit("bisect did not record --first-parent")
    log.append(f"bisect_run mode={mode} rc={proc.returncode} first_parent=yes")
    brk = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    run(["git", "bisect", "reset"], check=False)
    return brk


def pr_number(rev: str) -> str:
    subj = subprocess.check_output(["git", "log", "-1", "--format=%s", rev], text=True)
    m = re.search(r"#(\d+)", subj)
    return m.group(1) if m else ""


def diff_hunk(break_rev: str) -> str:
    parent = subprocess.check_output(["git", "rev-parse", f"{break_rev}^1"], text=True).strip()
    files = subprocess.check_output(
        ["git", "diff", "--name-only", parent, break_rev], text=True
    ).splitlines()
    code = [f for f in files if f.startswith(("omega-rust/", "source/")) and not f.endswith(".md")]
    if not code:
        return "mechanism not established"
    path = code[0]
    diff = subprocess.check_output(
        ["git", "diff", "-U3", parent, break_rev, "--", path], text=True, errors="replace"
    )
    if not diff.strip():
        return "mechanism not established"
    lines = diff.splitlines()[:40]
    suffix = "\n```" if len(diff.splitlines()) > 40 else "\n```"
    return "```diff\n" + "\n".join(lines) + suffix


def cluster_members(cluster_id: str) -> list[str]:
    base = cluster_id.split(".", 1)[0]
    if "." in cluster_id:
        suffix = cluster_id.split(".", 1)[1]
        for fix in load_all_broken_fixtures():
            if fix.split("/")[-1] == suffix or fix.replace("/", "_") == suffix:
                return [fix]
        return [suffix]
    sys.path.insert(0, str(AG / "tools"))
    from classify_corpus import classify_all, norm_diag, parse_records  # noqa: E402

    obs = parse_records((AG / "part21_observed.txt").read_text())
    cls = classify_all(AG / "part21_observed.txt", AG / "part21_fixtures.txt")
    clusters: dict[str, list[str]] = {}
    for fix in cls["broken"]:
        first = (obs[fix]["diagnostics"] or [""])[0]
        clusters.setdefault(norm_diag(first), []).append(fix)
    ranked = sorted(clusters.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    idx = int(re.match(r"c(\d+)", base).group(1))
    return sorted(ranked[idx][1])


def load_all_broken_fixtures() -> list[str]:
    sys.path.insert(0, str(AG / "tools"))
    from classify_corpus import classify_all  # noqa: E402

    return list(classify_all(AG / "part21_observed.txt", AG / "part21_fixtures.txt")["broken"])


def first_pin_class_commit(
    chain: list[str], witness: str, spec: Path, start_idx: int, log: list[str], cluster_id: str
) -> str | None:
    for idx in range(start_idx, len(chain)):
        ec = test_witness(witness, spec, chain[idx], log, "class_fwd", cluster_id)
        if ec == 1:
            return chain[idx]
    return None


def member_flip_rows(
    members: list[str],
    parent: str,
    brk: str,
    log: list[str],
    cluster_id: str,
    mode: str = "outcome",
) -> list[tuple[str, int, int]]:
    rows: list[tuple[str, int, int]] = []
    for fix in sorted(members):
        spec = prepare_spec(fix)
        pe = test_witness(
            fix, spec, parent, log, f"member_parent_{mode}_{fix.split('/')[-1]}", cluster_id, mode
        )
        be = test_witness(
            fix, spec, brk, log, f"member_break_{mode}_{fix.split('/')[-1]}", cluster_id, mode
        )
        rows.append((fix, pe, be))
    return rows


def write_detail(
    cluster_id: str,
    witness: str,
    good: str,
    parent: str,
    brk: str,
    on_chain: bool,
    codes_bad: list[int],
    codes_good: list[int],
    flip_rows: list[tuple[str, int, int]],
    mech: str,
    class_rev: str | None,
    extra: list[str] | None = None,
) -> None:
    matrix = ["| fixture | parent exit | first-bad exit |", "| --- | ---: | ---: |"]
    for fix, pe, be in flip_rows:
        matrix.append(f"| `{fix}` | {pe} | {be} |")
    detail = [
        f"## Cluster {cluster_id}",
        "",
        f"- Witness: `{witness}`",
        f"- Good anchor: `{good}`",
        f"- Parent (first-bad^1): `{parent}`",
        f"- First bad: `{brk}`",
        f"- On first-parent between good and PIN: **{on_chain}**",
        f"- PR: **#{pr_number(brk)}**",
        f"- Triple verify break: `{codes_bad}`",
        f"- Triple verify parent: `{codes_good}`",
        "",
        "### Member flip table",
        "",
        *matrix,
        "",
        "### Mechanism",
        "",
        mech,
    ]
    if class_rev and class_rev != brk:
        detail += ["", "### First commit showing PIN diagnostic class", "", f"`{class_rev}`"]
    if extra:
        detail += ["", *extra]
    LOGROOT.mkdir(parents=True, exist_ok=True)
    (LOGROOT / f"cluster-{cluster_id}.md").write_text("\n".join(detail) + "\n")


def run_cluster_pipeline(witness: str, cluster_id: str, *, do_splits: bool = True) -> None:
    setup_tools()
    subprocess.check_call(
        ["bash", str(TOOLHOME / "prepare_witness.sh"), witness],
        env={**os.environ, "BISECT_REPO_ROOT": str(ROOT)},
    )
    spec = prepare_spec(witness)
    members = cluster_members(cluster_id)
    intro = fixture_intro(witness)
    chain = first_parent_chain(intro, PIN)
    findlog: list[str] = []
    out_lines: list[str] = []

    good = find_latest_good(chain, witness, spec, findlog, cluster_id)
    if good is None:
        out_lines.append(
            f"cluster={cluster_id} witness={witness} status=never_good intro={intro} "
            f"verdict=never-good_since_{intro}"
        )
        flush_logs(cluster_id, out_lines, findlog)
        write_detail(
            cluster_id,
            witness,
            "",
            "",
            "",
            False,
            [],
            [],
            [],
            "n/a",
            None,
            [f"**Verdict:** never-good since `{intro}`"],
        )
        run(["git", "checkout", "-f", "-q", PIN], check=False)
        return

    brk = git_bisect_break(witness, spec, good, PIN, findlog, cluster_id, "outcome")
    if brk in FORBIDDEN_EDGES:
        out_lines.append(
            f"cluster={cluster_id} witness={witness} good={good} break={brk} "
            f"status=inadmissible_edge reason=forbidden_commit"
        )
        flush_logs(cluster_id, out_lines, findlog)
        write_detail(
            cluster_id, witness, good, "", brk, False, [], [], [], "n/a", None,
            ["**Verdict:** inadmissible edge (PIN or #2228)."],
        )
        run(["git", "checkout", "-f", "-q", "work/ledger-bisect-part-21"], check=False)
        return
    parent = subprocess.check_output(["git", "rev-parse", f"{brk}^1"], text=True).strip()
    ok_bad, codes_bad = triple_verify(witness, spec, brk, 1, findlog, "verify_break", cluster_id, "outcome")
    ok_good, codes_good = triple_verify(witness, spec, parent, 0, findlog, "verify_parent", cluster_id, "outcome")
    if not (ok_bad and ok_good):
        out_lines.append(
            f"cluster={cluster_id} witness={witness} good={good} parent={parent} break={brk} "
            f"status=flaky verify_break_codes={codes_bad} verify_parent_codes={codes_good}"
        )
        flush_logs(cluster_id, out_lines, findlog)
        write_detail(
            cluster_id,
            witness,
            good,
            parent,
            brk,
            False,
            codes_bad,
            codes_good,
            [],
            "n/a",
            None,
            ["**Verdict:** flaky"],
        )
        run(["git", "checkout", "-f", "-q", PIN], check=False)
        return

    on_chain = good in chain and brk in chain and chain.index(good) < chain.index(brk)
    mech = diff_hunk(brk)
    class_at_break = test_witness(witness, spec, brk, findlog, "class_at_outcome_break", cluster_id, "class")
    class_notes: list[str] = []
    class_edge: str | None = brk if class_at_break == 1 else None
    if class_at_break == 1:
        ok_c_bad, c_bad = triple_verify(witness, spec, brk, 1, findlog, "verify_class_break", cluster_id, "class")
        ok_c_par, c_par = triple_verify(witness, spec, parent, 0, findlog, "verify_class_parent", cluster_id, "class")
        class_notes.append(
            f"- PIN-class matches this outcome edge. Class codes break `{c_bad}` parent `{c_par}` "
            f"stable={ok_c_bad and ok_c_par}."
        )
        if not (ok_c_bad and ok_c_par):
            class_edge = None
            class_notes.append("- PIN-class did not hold 1,1,1 at the break and 0,0,0 at `^1`.")
    else:
        class_notes.append("- Outcome edge diagnostic class differs from the PIN class. Class-only bisect follows.")
        cbrk = git_bisect_break(witness, spec, brk, PIN, findlog, cluster_id, "class")
        if cbrk in FORBIDDEN_EDGES:
            class_notes.append(f"- **PIN-class edge inadmissible** (`{cbrk}`).")
        else:
            cparent = subprocess.check_output(["git", "rev-parse", f"{cbrk}^1"], text=True).strip()
            ok_c_bad, c_bad = triple_verify(witness, spec, cbrk, 1, findlog, "verify_class_break", cluster_id, "class")
            ok_c_par, c_par = triple_verify(
                witness, spec, cparent, 0, findlog, "verify_class_parent", cluster_id, "class"
            )
            class_edge = cbrk if ok_c_bad and ok_c_par else None
            class_notes.append(
                f"- PIN-class first-bad `{cbrk}` parent `{cparent}` PR #{pr_number(cbrk)} "
                f"codes break `{c_bad}` parent `{c_par}`."
            )
            if class_edge:
                class_notes.append("")
                class_notes.append(diff_hunk(cbrk))
                class_flips = member_flip_rows(members, cparent, cbrk, findlog, cluster_id, "class")
                class_notes.append("")
                class_notes.append("| fixture | class parent | class first-bad |")
                class_notes.append("| --- | ---: | ---: |")
                for fix, pe, be in class_flips:
                    class_notes.append(f"| `{fix}` | {pe} | {be} |")
    flip_rows = member_flip_rows(members, parent, brk, findlog, cluster_id, "outcome")
    split_notes: list[str] = list(class_notes)
    split_ids: set[str] = set()

    if do_splits and len(members) > 1:
        for fix, pe, be in flip_rows:
            if fix == witness:
                continue
            if pe == 0 and be == 1:
                continue
            split_id = f"{cluster_id}.{fix.split('/')[-1]}"
            split_ids.add(split_id)
            split_notes.append(f"- Split bisect `{fix}` → cluster `{split_id}` (parent {pe} → break {be})")
            run_cluster_pipeline(fix, split_id, do_splits=False)

    write_detail(
        cluster_id,
        witness,
        good,
        parent,
        brk,
        on_chain,
        codes_bad,
        codes_good,
        flip_rows,
        mech,
        class_edge,
        split_notes or None,
    )
    out_lines.append(
        f"cluster={cluster_id} witness={witness} good={good} parent={parent} break={brk} "
        f"pr={pr_number(brk)} on_first_parent={on_chain} verify_break_codes={codes_bad} "
        f"verify_parent_codes={codes_good} status=bisected"
    )
    flush_logs(cluster_id, out_lines, findlog)
    run(["git", "checkout", "-f", "-q", PIN], check=False)
    print(out_lines[0])


def main() -> int:
    witness, cluster_id = sys.argv[1], sys.argv[2]
    run_cluster_pipeline(witness, cluster_id)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
