"""Advisory annotations for Omega proof rejections via TypeSafe Jev.

NOT a compiler feature: this runs after `omega --check`, never changes the
verdict, and never touches the artifact. When the checker reports a
"cannot prove"-class rejection it cannot classify further, this tool asks
Jev whether the rejected contract is self-evidently false (a logic bug) or
needs machinery the bounded entailment engine lacks (a capability gap) --
the distinction the diagnostic alone cannot offer.

Measured profile (build/experiments/proof-smell*, 2026-09-18): ~0.1% false
positives at 1,900-program scale, ~55-60% recall on unannotated logic
garbage. A flag means "look here first"; silence means "no evident
contradiction," not "correct."

Python 3 standard library only. Usage:

  python tools/proof_advisor.py path/to/main.omg
  python tools/proof_advisor.py --omega target/debug/omega.exe main.omg
  python tools/proof_advisor.py --self-test

Key: TYPESAFE_API_KEY env var, or --key-file build/typesafe.env.txt.
Without a key the diagnostics pass through unchanged.
"""
import argparse
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.error
import urllib.request

ENDPOINT = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-1.13.0"
FLAG = 0.5

# The residue class this advisory classifies: the engine proved a falsehood,
# could not establish a contract, or stood down on unimplemented entailment.
# Syntax/type diagnostics are out of scope (measured: they do not false-flag).
REJECTION = re.compile(
    r"cannot prove|disproved|refuted|no entailment tier|cannot construct"
    r"|required fact|structurally false|proof-only",
    re.I)
CLAUSE_HEAD = re.compile(r"(requires|ensures|invariant)\b")
TOP_LEVEL = re.compile(r"(machine|data|state|fn|let|const)\b")


def comment_cut(line):
    quoted = False
    for index in range(len(line) - 1):
        if line[index] == '"' and (index == 0 or line[index - 1] != "\\"):
            quoted = not quoted
        elif line[index] == "/" and line[index + 1] == "/" and not quoted:
            return index
    return len(line)


def contract_clauses(source):
    """requires/ensures/invariant clause lines -> 'L<n>' choice criteria."""
    clauses, role = [], None
    for number, raw in enumerate(source.splitlines(), 1):
        text = raw[:comment_cut(raw)].strip()
        if not text:
            continue
        head = CLAUSE_HEAD.match(text)
        if head:
            role = head.group(1)
            rest = text[head.end():].strip().rstrip("{").strip()
            if rest:
                clauses.append((number, rest))
            if "{" in text:
                role = None
        elif role:
            if "{" in text or "}" in text or TOP_LEVEL.match(text):
                role = None
            else:
                clauses.append((number, text))
    return clauses


def make_request(source, diagnostics):
    """One batch: per rejection, the measured false_claim + capability_gap
    nouls, plus a culprit choice over extracted contract clauses."""
    questions = {}
    for index, line in enumerate(diagnostics):
        rid = f"r{index}"
        base = (f"for `programs.p0.source` whose compiler rejected with "
                f"`diagnostics.{rid}`")
        questions[f"{rid}::false_claim"] = {
            "type": "noul",
            "instructions": (
                f"Decide {base} whether the Omega program asserts a contract or "
                "proof fact that is logically false or unsatisfiable under its own "
                "declared premises (requires, ensures, invariants, guards). Answer "
                "yes only for genuine logical falsehood, for example an ensures "
                "that contradicts its requires or mutually unsatisfiable premises. "
                "Do not answer yes merely because the program relies on unsupported "
                "features, missing citations, or unimplemented machinery."),
            "criteria": {
                "true": "Some asserted clause is logically false or the premises "
                        "are mutually unsatisfiable.",
                "false": "No asserted clause is logically false; any rejection "
                         "would come from unsupported capability, not falsehood."}}
        questions[f"{rid}::capability_gap"] = {
            "type": "noul",
            "instructions": (
                f"Decide {base} whether the program depends on machinery beyond a "
                "bounded integer, order, and structural entailment engine: quotient "
                "types, algebraic ring or group laws, lemma citation, proof-only "
                "values, or other surface a bounded checker would stand down on."),
            "criteria": {
                "true": "The program needs machinery beyond bounded integer, "
                        "order, or structural entailment.",
                "false": "The program stays within bounded integer, order, or "
                         "structural reasoning."}}
        clauses = contract_clauses(source)
        if clauses:
            criteria = {f"L{n}": f"line {n}: {t}" for n, t in clauses}
            criteria["none"] = ("No single clause is false; the failure is "
                                "structural rather than a false clause.")
            questions[f"{rid}::culprit"] = {
                "type": "choice",
                "instructions": (
                    f"In `programs.p0.source`, if the rejected contract asserts a "
                    "logically false claim, select the single clause that carries "
                    "the falsehood. If no clause is false, choose none."),
                "criteria": criteria}
    return {"model": MODEL,
            "state": {"programs": {"p0": {"source": source}},
                      "diagnostics": {f"r{i}": d for i, d in enumerate(diagnostics)}},
            "questions": questions}


def evaluate(request, response):
    answers = response.get("answers")
    if not isinstance(answers, dict) or set(answers) != set(request["questions"]):
        raise ValueError("Response question IDs do not match the request")
    verdicts = {}
    for index in range(len(request["state"]["diagnostics"])):
        rid = f"r{index}"
        false_p = answers[f"{rid}::false_claim"].get("noul")
        cap_p = answers[f"{rid}::capability_gap"].get("noul")
        if not all(isinstance(p, (int, float)) and math.isfinite(p)
                   and 0 <= p <= 1 for p in (false_p, cap_p)):
            raise ValueError("Invalid Noul response")
        culprit = answers.get(f"{rid}::culprit", {}).get("choice")
        criteria = request["questions"].get(f"{rid}::culprit", {}).get("criteria", {})
        verdicts[rid] = dict(false_p=false_p, cap_p=cap_p, culprit=culprit,
                             culprit_text=None if culprit in (None, "none")
                             else criteria.get(culprit))
    return verdicts


def self_test():
    source = ("machine m(a: u64, b: u64)\nrequires\n    a < b\nensures\n"
              "    b < a\n{\n}\n")
    request = make_request(source, ["cannot prove ensures contract: b < a"])
    assert set(request["questions"]) == {
        "r0::false_claim", "r0::capability_gap", "r0::culprit"}
    assert set(request["questions"]["r0::culprit"]["criteria"]) == {
        "L3", "L5", "none"}
    response = {"answers": {
        "r0::false_claim": {"type": "noul", "noul": 0.9},
        "r0::capability_gap": {"type": "noul", "noul": 0.2},
        "r0::culprit": {"type": "choice", "choice": "L5"}}}
    verdict = evaluate(request, response)["r0"]
    assert verdict["false_p"] == 0.9 and verdict["culprit_text"].startswith("line 5")
    response["answers"]["r0::false_claim"]["noul"] = 1.5
    try:
        evaluate(request, response)
    except ValueError:
        pass
    else:
        raise AssertionError("Invalid probability accepted")
    print("Self-test passed: clause extraction, request shape, scoring")


def render(verdicts, diagnostics):
    out = []
    for index, line in enumerate(diagnostics):
        out.append(line)
        verdict = verdicts.get(f"r{index}")
        if not verdict:
            continue
        false_p, cap_p = verdict["false_p"], verdict["cap_p"]
        if false_p >= FLAG:
            clause = verdict["culprit_text"]
            where = f" near {clause}" if clause else " in the rejected contract"
            out.append(f"  = advisory (jev {false_p:.2f}): likely a logic bug -- "
                       f"contradiction{where}.")
        elif cap_p >= FLAG:
            out.append(f"  = advisory (jev {cap_p:.2f}): likely needs machinery "
                       f"the entailment engine lacks -- debugging the logic "
                       f"probably won't help.")
        else:
            out.append(f"  = advisory (jev: false {false_p:.2f}, capability "
                       f"{cap_p:.2f}): no evident contradiction; debug normally.")
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path, nargs="?", help="Omega entrypoint")
    parser.add_argument("--omega", type=Path, help="omega binary path")
    parser.add_argument("--key-file", type=Path)
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        self_test()
        return
    omega = arguments.omega or Path(os.environ.get("OMEGA", "omega"))
    completed = subprocess.run(
        [str(omega), "--check", str(arguments.root)],
        capture_output=True, text=True)
    sys.stdout.write(completed.stdout)
    stderr_lines = completed.stderr.splitlines()
    diagnostics = [line for line in stderr_lines if REJECTION.search(line)]
    if completed.returncode == 0 or not diagnostics:
        sys.stderr.write(completed.stderr)
        return  # clean compile or no in-scope rejection: nothing to advise
    key = os.environ.get("TYPESAFE_API_KEY", "").strip()
    if not key and arguments.key_file and arguments.key_file.exists():
        keys = [line.partition("=")[2].strip().strip("\"'")
                for line in arguments.key_file.read_text(
                    encoding="utf-8-sig").splitlines()
                if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
        if len(keys) == 1:
            key = keys[0]
    if not key:
        sys.stderr.write(completed.stderr)
        print("  = advisory unavailable: no TYPESAFE_API_KEY", file=sys.stderr)
        return
    # Scope: the entrypoint file. Multi-file projects need import walking;
    # corpus and sample programs are single-file.
    source = arguments.root.read_text(encoding="utf-8")
    request = make_request(source, diagnostics)
    http = urllib.request.Request(
        ENDPOINT, data=json.dumps(request).encode(),
        headers={"Authorization": f"Bearer {key}",
                 "Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.build_opener(
                urllib.request.HTTPRedirectHandler()).open(
                http, timeout=60) as connection:
            response = json.load(connection)
    except (urllib.error.URLError, TimeoutError, urllib.error.HTTPError):
        sys.stderr.write(completed.stderr)
        print("  = advisory unavailable: TypeSafe request failed",
              file=sys.stderr)
        return
    if key in json.dumps(response):
        raise RuntimeError("Response contains the credential; refusing output")
    rendered = render(evaluate(request, response), diagnostics)
    advisory = iter(rendered)
    for line in stderr_lines:
        if line in diagnostics and REJECTION.search(line):
            print(next(advisory), file=sys.stderr)
            print(next(advisory), file=sys.stderr)
        else:
            print(line, file=sys.stderr)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        raise SystemExit(f"proof_advisor failed: {type(error).__name__}: {error}")
