"""Rebuild the native-host checkpoint from the pinned proof and its GitHub Actions logs."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


REPO = "CathedralOS/Omega"
RUN = "44227027634"
PROOF = "edc830ed21a353efd63ac94e19503c7224e13a01"
INTEGRATION = "683122d180a90b6c339904e1c07c0a3ad028b61b"
PROVIDER_CHANGE = "3d14fe87dff4fc08ed0d6ee8cbf2f5d8bde00348"
RECORD = Path(__file__).resolve().parents[1] / "checkpoints/native-host-execution-f658.json"
ROOT = Path(__file__).resolve().parents[4]
HOSTS = {
    "linux_x86_64": ("Linux", "x86_64", "X64", "ELF64"),
    "linux_arm64": ("Linux", "aarch64", "ARM64", "ELF64"),
    "macos_arm64": ("Darwin", "arm64", "ARM64", "Mach-O 64"),
    "windows_x86_64": ("Windows", "amd64", "X64", "PE32+"),
}
LANES = ("baseline", "candidate")


def command(*args):
    return subprocess.check_output(args, cwd=ROOT)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def collect_runs(log, manifest):
    rows = {}
    for line in log.splitlines():
        job, separator, payload = line.partition("\t")
        if not separator or ' {"exit":' not in payload:
            continue
        row = json.loads(payload[payload.index(' {"exit":') + 1:])
        target, lane = row["target"], row["lane"]
        require(job == target and target in HOSTS and lane in LANES, "unexpected execution")
        require((target, lane) not in rows, f"duplicate execution: {target} {lane}")
        require((*row["host"], row["format"]) == HOSTS[target], f"wrong host or format: {target}")
        require(row["exit"] == manifest["expected_exit"] == 0, f"unexpected exit: {target} {lane}")
        require(row["sha256"] == manifest["images"][target][lane], f"wrong image: {target} {lane}")
        rows[target, lane] = row
    require(set(rows) == {(t, lane) for t in HOSTS for lane in LANES}, "incomplete run matrix")
    return rows


def checkpoint():
    workflow = json.loads(command("gh", "api", f"repos/{REPO}/actions/runs/{RUN}"))
    require(workflow["head_sha"] == PROOF and workflow["conclusion"] == "success", "wrong workflow")
    jobs = json.loads(command("gh", "api", f"repos/{REPO}/actions/runs/{RUN}/jobs"))["jobs"]
    require(len(jobs) == len(HOSTS) and {j["name"] for j in jobs} == set(HOSTS), "wrong jobs")
    manifest = json.loads(command("git", "show", f"{PROOF}:native-host-proof/images.json"))
    rows = collect_runs(command("gh", "run", "view", RUN, "--repo", REPO, "--log").decode(), manifest)
    reference, fixture = manifest["reference_git"], manifest["fixture"]
    command("git", "merge-base", "--is-ancestor", reference, INTEGRATION)
    command("git", "merge-base", "--is-ancestor", INTEGRATION, PROVIDER_CHANGE)
    require(PROVIDER_CHANGE[:11] in manifest["candidate_provenance"], "wrong migration build line")
    for name, sha in manifest["sources_sha256"].items():
        require(digest(command("git", "show", f"{reference}:{fixture}/{name}")) == sha,
                f"wrong fixture source: {name}")
    library = command("git", "rev-parse", f"{reference}:source/library").decode().strip()
    require(library == manifest["toolchain_library_git_tree"], "wrong official library")
    targets = {}
    for job in sorted(jobs, key=lambda j: j["name"]):
        target = job["name"]
        require(job["conclusion"] == "success" and job["head_sha"] == PROOF, f"unsuccessful job: {target}")
        images = {}
        for lane in LANES:
            row = rows[target, lane]
            name = "omega-program.exe" if target == "windows_x86_64" else "omega-program"
            image = command("git", "show", f"{PROOF}:native-host-proof/images/{lane}/{target}/{name}")
            require(len(image) == row["size"] and digest(image) == row["sha256"],
                    f"committed image differs from executed image: {target} {lane}")
            images[lane] = {"sha256": row["sha256"], "bytes": row["size"], "exit": row["exit"]}
        require(images["baseline"]["sha256"] == images["candidate"]["sha256"], f"images differ: {target}")
        targets[target] = {"job": job["html_url"], "runner": job["labels"],
                           "host": list(HOSTS[target][:3]), "format": HOSTS[target][3], "images": images}
    return {
        "workflow": workflow["html_url"], "proof_commit": PROOF,
        "compilers": {
            "baseline": {"source_revision": reference, "sha256": manifest["compiler_sha256"]["baseline"]},
            "candidate": {"integrated_base": INTEGRATION, "includes_provider_change": PROVIDER_CHANGE,
                          "exact_checkout": None, "sha256": manifest["compiler_sha256"]["candidate"]},
        },
        "source_library": {"git_tree": library},
        "fixture": {"path": fixture, "git_tree": command("git", "rev-parse", f"{reference}:{fixture}").decode().strip(),
                    "sha256": manifest["sources_sha256"]},
        "targets": targets,
    }


def check_products(record, products):
    environment = json.loads((products / "environment.json").read_text())
    fixture = record["fixture"]
    for name, sha in fixture["sha256"].items():
        require(environment["source_files"][fixture["path"] + "/" + name] == sha, f"fixture changed: {name}")
    reference = record["compilers"]["baseline"]["source_revision"]
    library = environment["toolchain_library_file_hashes"]
    files = command("git", "ls-tree", "-r", "--name-only", f"{reference}:source/library").decode().splitlines()
    require(set(library) == set(files), "library inventory differs")
    for name in files:
        require(digest(command("git", "show", f"{reference}:source/library/{name}")) == library[name],
                f"library file differs: {name}")
    for lane, compiler in record["compilers"].items():
        binary = environment["binaries"][lane]
        require(digest(Path(binary["path"]).read_bytes()) == binary["sha256"] == compiler["sha256"],
                f"different compiler executable: {lane}")
        for target, host in record["targets"].items():
            name = "omega-program.exe" if target == "windows_x86_64" else "omega-program"
            image = (products / lane / "matrix" / target / name).read_bytes()
            require(digest(image) == host["images"][lane]["sha256"], f"different local image: {target} {lane}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="replace the archived checkpoint")
    parser.add_argument("--products", type=Path, help="also match the original local matrix, compilers and library")
    args = parser.parse_args()
    record = checkpoint()
    if args.products:
        check_products(record, args.products)
    if args.write:
        RECORD.write_text(json.dumps(record, indent=2) + "\n")
    else:
        require(record == json.loads(RECORD.read_text()), "saved checkpoint differs from Git and Actions")
    print("Verified both compiler images on all four matching native hosts.")
