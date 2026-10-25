#!/usr/bin/env python3
"""Host preflight: can this machine build and measure the compiler?

Every check answers a failure this repository has actually hit, before a
session burns an hour rediscovering it:

  - the pinned Rust toolchain resolves and its linker works end to end
    (an Xcode linker older than the active SDK rejected every link on
    macOS 27 hosts — the probe compiles and runs a binary, which is the
    thing that failed, rather than parsing version strings);
  - mbx and cargo-nextest exist at the versions tools/testing.md assumes;
  - the claims coordination ref is reachable, since every shared-path
    edit starts with `claims.py status`;
  - pull.rebase is set, since publication requires linear history.

Exit codes: 0 with warnings allowed, 1 when a check failed, 2 on usage
error. `--json PATH` also writes the structured report. Each check prints
`ok`/`warn`/`fail`/`skip` plus one line of evidence and, when there is a
known remedy, the command or change that fixes it. Read-only: doctor
changes nothing on the host. Standard library only, Python 3.9+ (it must
itself run on the oldest host it might diagnose).
"""

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
CLAIMS_REF = "refs/coordination/omega-claims/main"
# cargo-nextest 0.9.140 is the floor AGENTS.md documents for `mbx nextest`.
NEXTEST_MINIMUM = (0, 9, 140)
# Subprocesses get hard deadlines: a stalled transport or a toolchain shim
# waiting on a lock must not park the preflight itself.
COMMAND_TIMEOUT = 45
PROBE_TIMEOUT = 90


def run(arguments, timeout=COMMAND_TIMEOUT, cwd=None):
    """(returncode, stdout, stderr), or None when the command is missing or
    outlived its budget."""
    env = dict(os.environ)
    env["GIT_TERMINAL_PROMPT"] = "0"
    # The same transport bounds claims.py applies, so a dead connection
    # fails fast instead of hanging the doctor.
    env.setdefault("GIT_HTTP_LOW_SPEED_LIMIT", "100")
    env.setdefault("GIT_HTTP_LOW_SPEED_TIME", "20")
    env.setdefault("GIT_SSH_COMMAND",
                   "ssh -o BatchMode=yes -o ConnectTimeout=10")
    try:
        result = subprocess.run(arguments, cwd=cwd or ROOT, env=env,
                                capture_output=True, text=True,
                                timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return result.returncode, result.stdout.strip(), result.stderr.strip()


def check(name, status, detail, fix=None):
    entry = {"name": name, "status": status, "detail": detail}
    if fix:
        entry["fix"] = fix
    return entry


def toolchain_channel():
    """The channel pinned in rust-toolchain.toml, or None."""
    path = ROOT / "rust-toolchain.toml"
    if not path.is_file():
        return None
    match = re.search(r'channel\s*=\s*"([^"]+)"',
                      path.read_text(encoding="utf-8", errors="replace"))
    return match.group(1) if match else None


def check_python():
    version = sys.version_info
    text = f"python {version.major}.{version.minor}.{version.micro}"
    if version >= (3, 9):
        return check("python", "ok", text)
    return check("python", "fail", f"{text} — repo tools require 3.9+",
                 fix="run with a newer python3")


def check_git_repository():
    result = run(["git", "rev-parse", "--verify", "HEAD"])
    if result is None:
        return check("git", "fail", "git is missing or timed out")
    code, head, error = result
    if code or not head:
        return check("git", "fail",
                     f"no commit at HEAD ({error or 'unborn branch'})")
    rebase = run(["git", "config", "--get", "pull.rebase"])
    detail = f"HEAD {head[:12]}"
    if rebase is None or rebase[0] or rebase[1] != "true":
        return check("git", "warn", detail + "; pull.rebase is not true",
                     fix="git config pull.rebase true — publication is "
                         "squash-only through the merge queue (tools/landing.md)")
    return check("git", "ok", detail)


def check_rust_toolchain():
    channel = toolchain_channel()
    result = run(["rustc", "--version"])
    if result is None:
        return check("rustc", "fail", "rustc missing or timed out",
                     fix="install rustup; rust-toolchain.toml selects the "
                         "pinned toolchain")
    code, version, error = result
    if code:
        return check("rustc", "fail",
                     f"rustc --version failed: {error or version}",
                     fix=(f"rustup toolchain install {channel}" if channel
                          else "install the toolchain rust-toolchain.toml "
                               "names"))
    detail = version + (f" (pinned {channel})" if channel else "")
    if channel and channel.startswith("nightly") and "nightly" not in version:
        return check("rustc", "warn",
                     detail + " — pin wants nightly, resolved toolchain is "
                     "not", fix="rustup toolchain install " + channel)
    return check("rustc", "ok", detail)


def versioned_sdk(path):
    """True for a version-numbered SDK directory (MacOSX26.2.sdk), which an
    Xcode update deletes. The unversioned MacOSX.sdk is maintained across
    updates — and Apple's /usr/bin/python3 shim exports exactly that path
    into its own process, so flagging it would warn about a variable the
    user never set."""
    return re.fullmatch(r"MacOSX\d[\d.]*\.sdk", Path(path).name) is not None


def check_rust_link_probe():
    """Compile, link, and run a trivial binary — the end-to-end check that
    catches a linker/SDK version mismatch version strings cannot see.

    rustc runs with the repository as its working directory, so rustup
    resolves rust-toolchain.toml and the probe exercises the pinned
    toolchain; from a scratch directory rustup falls back to its default
    (stable on the development Mac), which is not what builds Omega."""
    sdkroot = os.environ.get("SDKROOT", "").strip()
    if sdkroot and not Path(sdkroot).is_dir():
        return check("rust-link", "fail",
                     f"SDKROOT names a missing SDK: {sdkroot}",
                     fix="unset SDKROOT or resolve it dynamically: "
                         'export SDKROOT="$(xcrun --sdk macosx '
                         '--show-sdk-path)"')
    with tempfile.TemporaryDirectory(prefix="omega doctor ") as scratch:
        source = Path(scratch) / "probe.rs"
        binary = Path(scratch) / ("probe.exe" if os.name == "nt" else "probe")
        source.write_text("fn main() { std::process::exit(7) }\n",
                          encoding="utf-8")
        built = run(["rustc", str(source), "-o", str(binary)],
                    timeout=PROBE_TIMEOUT, cwd=ROOT)
        if built is None:
            return check("rust-link", "fail", "rustc probe timed out")
        code, out, error = built
        if code:
            detail = (error or out)[-400:]
            # On macOS the load-bearing evidence is the linker/SDK pair;
            # surface it beside the failure so the diagnosis is one step.
            if platform.system().lower() == "darwin":
                lines = [f"link probe failed: {detail}"]
                selected = run(["xcode-select", "-p"])
                if selected and not selected[0]:
                    lines.append(f"xcode-select: {selected[1]}")
                sdk = run(["xcrun", "--show-sdk-path"])
                if sdk and not sdk[0]:
                    lines.append(f"sdk: {sdk[1]}")
                linker = run(["ld", "-v"])
                if linker:
                    lines.append(f"ld: {(linker[2] or linker[1])[:200]}")
                if sdkroot:
                    lines.append(f"SDKROOT={sdkroot}")
                return check("rust-link", "fail", "; ".join(lines),
                             fix="the linker must be at least as new as the "
                                 "SDK — update Xcode or xcode-select the "
                                 "Command Line Tools (tools/release/README.md)")
            return check("rust-link", "fail", f"link probe failed: {detail}")
        ran = run([str(binary)], timeout=10, cwd=scratch)
        if ran is None:
            return check("rust-link", "fail", "probe binary would not run")
        if ran[0] != 7:
            return check("rust-link", "fail",
                         f"probe binary exited {ran[0]}, expected 7")
    detail = "compiled, linked, and ran a probe with the pinned toolchain"
    if sdkroot and versioned_sdk(sdkroot):
        return check("rust-link", "warn",
                     f"{detail} (SDKROOT={sdkroot})",
                     fix="SDKROOT names a version-numbered SDK that the "
                         "next Xcode update deletes; unset it or resolve "
                         'it via xcrun: export SDKROOT="$(xcrun --sdk '
                         'macosx --show-sdk-path)"')
    return check("rust-link", "ok", detail)


def check_mbx():
    path = shutil.which("mbx")
    if not path:
        return check("mbx", "warn", "mbx not on PATH",
                     fix="AGENTS.md prefers mbx over cargo; install it or "
                         "use cargo directly")
    version = run(["mbx", "--version"], timeout=20)
    text = version[1] if version and not version[0] else "version unknown"
    return check("mbx", "ok", f"{text} ({path})")


def check_nextest():
    result = run(["cargo", "nextest", "--version"], timeout=20)
    if result is None or result[0]:
        return check("nextest", "warn", "cargo nextest unavailable",
                     fix="cargo install cargo-nextest — needed for "
                         "`mbx nextest run`")
    match = re.search(r"(\d+)\.(\d+)\.(\d+)", result[1] or result[2] or "")
    if not match:
        return check("nextest", "warn", "could not parse nextest version")
    found = tuple(int(part) for part in match.groups())
    if found < NEXTEST_MINIMUM:
        wanted = ".".join(str(part) for part in NEXTEST_MINIMUM)
        return check("nextest", "warn",
                     f"nextest {found[0]}.{found[1]}.{found[2]} < "
                     f"{wanted}", fix="cargo install cargo-nextest --locked")
    return check("nextest", "ok", f"nextest {found[0]}.{found[1]}.{found[2]}")


def check_claims_remote():
    url = run(["git", "remote", "get-url", "--push", "--all", "origin"])
    if url is None or url[0] or not url[1]:
        return check("claims", "warn", "no push URL for origin",
                     fix="claims.py needs exactly one push URL")
    urls = url[1].splitlines()
    if len(urls) != 1:
        return check("claims", "warn",
                     f"origin has {len(urls)} push URLs; claims needs one")
    remote = run(["git", "ls-remote", "--refs", urls[0], CLAIMS_REF],
                 timeout=COMMAND_TIMEOUT)
    if remote is None or remote[0]:
        return check("claims", "warn",
                     f"claims ref unreachable: "
                     f"{(remote[2] if remote else 'timed out')[:200]}",
                     fix="claims.py status/note/claim will fail until the "
                         "coordination remote answers")
    return check("claims", "ok", f"{CLAIMS_REF} reachable")


def collect_checks():
    checks = [check_python(), check_git_repository(), check_rust_toolchain(),
              check_rust_link_probe(), check_mbx(), check_nextest(),
              check_claims_remote()]
    load = None
    try:
        load = round(os.getloadavg()[0], 2)
    except (AttributeError, OSError):
        pass
    detail = (f"{platform.system().lower()}/{platform.machine().lower()}, "
              f"{os.cpu_count()} cpus"
              + (f", load1 {load}" if load is not None else ""))
    checks.append(check("host", "ok", detail))
    return checks


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--json", metavar="PATH",
                        help="also write the structured report to PATH")
    options = parser.parse_args(argv)
    checks = collect_checks()
    failed = [entry for entry in checks if entry["status"] == "fail"]
    warned = [entry for entry in checks if entry["status"] == "warn"]
    for entry in checks:
        line = f"{entry['status']:>4}  {entry['name']}: {entry['detail']}"
        if entry.get("fix"):
            line += f"  [fix: {entry['fix']}]"
        print(line)
    print(f"doctor: {len(failed)} failed, {len(warned)} warnings, "
          f"{len(checks)} checks")
    if options.json:
        path = Path(options.json)
        if str(path.parent) not in ("", "."):
            path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(
            {"schema": "omega-doctor-v1", "checks": checks,
             "failed": len(failed), "warned": len(warned)},
            indent=2) + "\n", encoding="utf-8")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
