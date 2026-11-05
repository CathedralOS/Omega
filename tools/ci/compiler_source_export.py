"""Keep the measured source inspectable after a work branch is squashed.

The existing corpus owner creates and verifies the bundle. This caller makes
its shallow checkout complete above the published prerequisite, then copies
the verified bytes into the pilot artifact directory covered by its existing upload.
"""

import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

import corpus_gate
from qualification import file_digest


def git_result(directory, *arguments):
    try:
        return subprocess.run(["git", *arguments], cwd=directory,
                              capture_output=True, text=True, timeout=120)
    except subprocess.TimeoutExpired as error:
        raise ValueError("source export: git " + arguments[0] + " exceeded its 120-second allowance") from error


def git(*arguments):
    result = git_result(corpus_gate.ROOT, *arguments)
    if result.returncode:
        raise ValueError("source export: git " + arguments[0] + " refused: " + result.stderr[-2000:])
    return result.stdout.strip()


def source_bases(revisions):
    """Find complete source ranges; a shallow cut is not an export boundary."""
    main = corpus_gate.upstream_main()
    if main is None:
        arguments = ["fetch", "--no-tags"]
        if git("rev-parse", "--is-shallow-repository") == "true":
            arguments.append("--depth=64")
        git(*arguments, "origin", "refs/heads/main:refs/remotes/origin/main", *revisions)
        main = corpus_gate.upstream_main()
    if main is None:
        raise ValueError("source export has no known published main prerequisite")
    for attempt in range(17):
        shallow_file = Path(git("rev-parse", "--git-path", "shallow"))
        if not shallow_file.is_absolute():
            shallow_file = corpus_gate.ROOT / shallow_file
        cuts = set(shallow_file.read_text().splitlines()) if shallow_file.is_file() else set()
        bases = {}
        for revision in revisions:
            result = git_result(corpus_gate.ROOT, "merge-base", revision, main)
            if result.returncode not in (0, 1):
                raise ValueError("source export cannot determine its published prerequisite: " + result.stderr[-2000:])
            base = result.stdout.strip()
            if not base or cuts.intersection(git("rev-list", revision, "--not", base).splitlines()):
                break
            bases[revision] = base
        if len(bases) == len(revisions):
            return main, bases
        if not cuts or attempt == 16:
            raise ValueError("source export cannot authenticate complete history above published main")
        # Fetch only until the feature's ancestry reaches the published range.
        # The checkout and all compiler/fixture bytes stay on their exact SHA.
        git("fetch", "--no-tags", "--deepen=64", "origin",
            "refs/heads/main:refs/remotes/origin/main", *revisions)


def validate_bundle(bundle, revision, main, output):
    # An older bundle may have been made while the checkout was shallow.
    # Test its closure where those unpublished parents are actually absent.
    prerequisites = []
    with bundle.open("rb") as stream:
        while True:
            line = stream.readline()
            if line == b"\n":
                break
            if not line:
                raise ValueError("source export has an incomplete bundle header")
            if line.startswith(b"-"):
                match = re.fullmatch(rb"-([0-9a-f]{40}) [^\n]*\n", line)
                if match is None:
                    raise ValueError("source export has an invalid bundle prerequisite")
                prerequisite = match[1].decode("ascii")
                git("merge-base", "--is-ancestor", prerequisite, main)
                prerequisites.append(prerequisite)
    with tempfile.TemporaryDirectory(prefix="source-verify-", dir=output) as temporary:
        verifier = Path(temporary)

        def verify(*arguments):
            result = git_result(verifier, *arguments)
            if result.returncode:
                raise ValueError("source export cannot restore its bundle from published prerequisites: "
                                 + result.stderr[-2000:])

        verify("init", "--bare", "-q")
        if prerequisites:
            # Only these published commits and their trees enter the verifier.
            # Their parents are explicit shallow boundaries, never feature cuts.
            verify("fetch", "--no-tags", "--depth=1", corpus_gate.ROOT.resolve().as_uri(), *prerequisites)
        verify("bundle", "unbundle", str(bundle.resolve()))
        verify("fsck", "--full", "--no-dangling", revision)
    return prerequisites


def export(output, revisions):
    revisions = tuple(dict.fromkeys(revisions))
    if not revisions or any(not isinstance(revision, str)
                            or re.fullmatch(r"[0-9a-f]{40}", revision) is None for revision in revisions):
        raise ValueError("source export requires exact source commits")
    main, bases = source_bases(revisions)
    published = git("rev-parse", main)
    directory = output / "source"
    directory.mkdir(parents=True, exist_ok=True)
    records = []
    for revision in revisions:
        error = corpus_gate.retain_record_source(revision)
        if error:
            raise ValueError("source export: " + error)
        record = dict(revision=revision, tree=git("rev-parse", revision + "^{tree}"),
                      published_main=published, prerequisite=bases[revision])
        if bases[revision] == revision:
            record["bundle"] = None
        else:
            source = corpus_gate.ROOT / "build/corpus-source" / (revision + ".bundle")
            if source.is_symlink() or not source.is_file():
                raise ValueError("source export has no ordinary verified bundle file")
            digest = file_digest(source)
            record["bundle_prerequisites"] = validate_bundle(source, revision, main, output)
            destination = directory / source.name
            if destination.is_symlink():
                raise ValueError("source export destination is a link")
            if destination.exists():
                if file_digest(destination) != digest:
                    raise ValueError("source export would replace a different retained bundle")
            else:
                shutil.copyfile(source, destination)
            if file_digest(destination) != digest or file_digest(source) != digest:
                raise ValueError("source export changed during artifact capture")
            record.update(bundle=str(destination), sha256=digest, bytes=destination.stat().st_size)
        records.append(record)
    (directory / "manifest.json").write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    return records
