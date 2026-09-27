#!/usr/bin/env python3
"""Advisory spec-drift audit: which documented claims no longer match reality?

Two layers, matching what each can actually decide:

- path-mention claims are checked mechanically — a cited `dir/file.rs` that
  no longer exists is drift, no judgment needed, EXCEPT where the sentence
  citing it says it should not exist. An architecture contract names the
  umbrella it refuses to grow and a convention names the spelling to avoid;
  there the absence is the claim holding. Those are reported separately with
  their sentence rather than dropped, and every finding prints its sentence
  so the reader can judge without opening the file.
- semantic claims (behavior, invariants, numbers) get a Jev verdict against
  the evidence a cheap gatherer can surface: existence results, grep hits
  for cited symbols, and the claim's own wording. Claims whose evidence the
  gatherer cannot reach are reported unverifiable rather than trusted.

Advisory only: prints a table, never gates, silent without a key for the
semantic pass (mechanical path findings still print). Suppress the semantic
pass with OMEGA_JEV_OFFLINE=1. Worked example:
build/experiments/failure-triage/DRIFT.md — 9/10, both real moved-file
drift cases caught, honest unverifiable hedging on partial evidence.

    tools/spec_drift_advisor.py wiki/drafts/audits/pipeline_placement_audit.md
    tools/spec_drift_advisor.py AGENTS.md --limit 15
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.request

ENDPOINT = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-1.13.0"
PATH_TOKEN = re.compile(r"`([A-Za-z0-9_./{}-]+/[-A-Za-z0-9_./{}]+)`")
CLAIM_LINE = re.compile(
    r"(`[A-Za-z0-9_./{}-]+/[-A-Za-z0-9_./{}]+`|--[a-z-]+|"
    r"\b(?:must|never|always|only|defaults? to|requires?|no longer)\b)")


def repository_root():
    done = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                          capture_output=True, text=True)
    return Path(done.stdout.strip())


def extract_claims(document, limit):
    """Sentences carrying checkable vocabulary, deduplicated."""
    claims = []
    seen = set()
    for line in document.read_text(encoding="utf-8").splitlines():
        stripped = line.strip().strip("|*- ")
        if len(stripped) < 30 or not CLAIM_LINE.search(stripped):
            continue
        key = stripped[:80]
        if key in seen:
            continue
        seen.add(key)
        claims.append(stripped)
        if len(claims) >= limit:
            break
    return claims


PATH_PREFIXES = ("omega-rust/", "tests/", "tools/", "wiki/", "source/",
                 "bootstrap/", "samples/")
CRATE_PREFIXES = ("psi/", "omega/")


def resolve_repo_path(root, token):
    """Docs cite both repo-relative paths (`omega-rust/psi/...`) and
    crate-relative ones (`psi/...`, `omega/...`). Resolve either form."""
    candidates = [token]
    if token.startswith(CRATE_PREFIXES):
        candidates.append("omega-rust/" + token)
    return next((c for c in candidates if (root / c).exists()), None)


# A path can be cited precisely BECAUSE it must not exist: an architecture
# contract naming the umbrella it refuses to grow, or a convention naming the
# spelling to avoid. Its absence is the claim holding, not drift. These
# markers say the sentence is about non-existence or prohibition, so a plain
# "does not depend on" keeps its citation auditable.
ABSENCE_MARKERS = (
    "there is no", "there are no", "no longer", "does not exist",
    "do not exist", "is gone", "was deleted", "was removed", "retired",
    "do not invent", "do not use", "do not add", "never", "avoid",
    "rather than", "instead of", "obscures", "must not", "no second",
)


def sentences_of(text):
    """The document as sentences, rewrapped: a doc's sentence spans lines."""
    flat = re.sub(r"\s+", " ", text)
    return re.split(r"(?<=[.:;])\s+", flat)


def cited_as_absent(sentence):
    lowered = sentence.lower()
    return any(marker in lowered for marker in ABSENCE_MARKERS)


def path_findings(root, document):
    """Mechanical: cited repo-relative paths that no longer exist.

    Returns (missing, deliberate) — the second list holds citations whose own
    sentence says the path should not exist, which are reported apart from
    drift rather than dropped.
    """
    missing = []
    deliberate = []
    text = document.read_text(encoding="utf-8")
    sentences = sentences_of(text)
    seen = set()
    for raw in PATH_TOKEN.findall(text):
        # A leading slash is not a repo-relative citation. A spec writing
        # `/source/assets/font.bin` is quoting a serialized value, and the
        # repository has no such root to check it against.
        if raw.startswith("/"):
            continue
        token = raw.strip("/")
        if token in seen or "{" in token:
            continue
        seen.add(token)
        if token.startswith(("http", "omega::")):
            continue
        repoish = token.startswith(PATH_PREFIXES) or \
            token.startswith(CRATE_PREFIXES)
        if not repoish or resolve_repo_path(root, token) is not None:
            continue
        context = next((sentence for sentence in sentences
                        if f"`{raw}`" in sentence), "")
        (deliberate if cited_as_absent(context) else missing).append(
            (token, context.strip()))
    return sorted(missing), sorted(deliberate)


def evidence_for(root, claim):
    """Cheap evidence: existence results for cited paths, grep hits for
    cited symbols, plus the claim itself."""
    parts = []
    for token in PATH_TOKEN.findall(claim):
        stripped = token.strip("/")
        resolved = resolve_repo_path(root, stripped)
        if resolved is None:
            parts.append(f"`{stripped}` exists: False (no repo resolution)")
        else:
            parts.append(f"`{stripped}` exists: True at {resolved}")
    symbols = set(re.findall(r"`([A-Za-z_][A-Za-z0-9_:]*(?:\.rs|\.omg)?)`",
                             claim))
    for symbol in list(symbols)[:3]:
        if "/" in symbol or not symbol.strip("`"):
            continue
        name = symbol.rsplit("::", 1)[-1]
        done = subprocess.run(
            ["git", "grep", "-l", name, "--", "omega-rust/", "tools/"],
            cwd=root, capture_output=True, text=True, timeout=10)
        hits = len(done.stdout.splitlines())
        parts.append(f"`{name}` grep hits in source: {hits}")
    return "; ".join(parts) if parts else "(no checkable anchors found)"


def jev_key():
    key = os.environ.get("TYPESAFE_API_KEY", "").strip()
    if key:
        return key
    for path in (Path("build/typesafe.env.txt"),
                 Path.home() / ".config" / "typesafe" / "typesafe.env.txt"):
        if path.is_file():
            keys = [line.partition("=")[2].strip().strip("\"'")
                    for line in path.read_text(encoding="utf-8-sig").splitlines()
                    if line.partition("=")[0].strip() == "TYPESAFE_API_KEY"]
            if len(keys) == 1:
                return keys[0]
    return ""


def jev_verdict(source, claim, evidence, key):
    request = {"model": MODEL,
               "state": {"document": source, "claim": claim,
                         "evidence": evidence},
               "questions": {"verdict": {
                   "type": "choice",
                   "instructions": (
                       "Does `evidence` confirm `claim` still describes "
                       "reality? `still_true`: evidence supports the claim. "
                       "`drifted`: evidence shows the claim no longer "
                       "matches reality (paths moved, behavior changed, "
                       "numbers stale). `unverifiable`: the evidence cannot "
                       "confirm or refute."),
                   "criteria": {
                       "still_true": "evidence supports the claim as written",
                       "drifted": "evidence shows the claim is stale or false",
                       "unverifiable": "evidence insufficient to decide"}}}}
    http = urllib.request.Request(
        ENDPOINT, data=json.dumps(request).encode(),
        headers={"Authorization": f"Bearer {key}",
                 "Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(http, timeout=20) as response:
            return (json.load(response).get("answers", {})
                    .get("verdict", {}).get("choice", "unverifiable"))
    except (urllib.error.URLError, TimeoutError, OSError, ValueError):
        return "unverifiable"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("document", type=Path)
    parser.add_argument("--limit", type=int, default=30,
                        help="max claims to evaluate (default 30)")
    arguments = parser.parse_args()
    if not arguments.document.is_file():
        print(f"spec_drift_advisor: {arguments.document} not found",
              file=sys.stderr)
        return 2
    root = repository_root()
    missing, deliberate = path_findings(root, arguments.document)
    if missing:
        print(f"stale path citations ({len(missing)}):")
        for path, context in missing:
            print(f"  ~ {path} — cited but does not exist")
            if context:
                print(f"      {context[:160]}")
    else:
        print("stale path citations: none")
    if deliberate:
        print(f"\ncited as deliberately absent ({len(deliberate)}) — the "
              f"sentence claims the path should NOT exist, so its absence is "
              f"the claim holding:")
        for path, context in deliberate:
            print(f"  = {path}\n      {context[:160]}")
    if os.environ.get("OMEGA_JEV_OFFLINE", "").strip() == "1":
        return 0
    key = jev_key()
    if not key:
        print("semantic pass unavailable: no TYPESAFE_API_KEY",
              file=sys.stderr)
        return 0
    claims = extract_claims(arguments.document, arguments.limit)
    counts = {"still_true": 0, "drifted": 0, "unverifiable": 0}
    print(f"\nsemantic claims ({len(claims)} evaluated):")
    for claim in claims:
        verdict = jev_verdict(str(arguments.document), claim,
                              evidence_for(root, claim), key)
        counts[verdict] = counts.get(verdict, 0) + 1
        if verdict != "still_true":
            print(f"  [{verdict}] {claim[:110]}")
    print(f"\nsummary: {counts['still_true']} still true, "
          f"{counts['drifted']} drifted, "
          f"{counts['unverifiable']} unverifiable")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError) as error:
        raise SystemExit(
            f"spec_drift_advisor failed: {type(error).__name__}: {error}")
