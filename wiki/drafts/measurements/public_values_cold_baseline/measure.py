"""Run from a detached Omega checkout under mbx reserve. Use a new output directory."""

import argparse
import ctypes
import hashlib
import io
import json
import os
import platform
import struct
import subprocess
import sys
import time
import zipfile
from pathlib import Path

FIXTURE = Path("source-rust/application/tests/fixtures/filesystem/run/public_values")
parser = argparse.ArgumentParser()
parser.add_argument("omega", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--count", type=int, default=5)
parser.add_argument("--profile-only", action="store_true")
parser.add_argument("--verify-only", action="store_true")
args = parser.parse_args()
omega = args.omega.resolve(strict=True)
out = args.output.resolve()
out.mkdir(mode=0o700)
rev = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
archive = subprocess.check_output(
    ["git", "archive", "--format=zip", rev, str(FIXTURE), "source/library"]
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def client(name):
    root = out / name
    root.mkdir(mode=0o700)
    with zipfile.ZipFile(io.BytesIO(archive)) as files:
        files.extractall(root)
    assert (
        digest(root / FIXTURE / "build.omg")
        == "dc4bda0f879ed834eb3f9ff081d6a0b00e483fd86bebe8c3e0c3f29e5d8ca7fd"
    )
    assert (
        digest(root / FIXTURE / "main.omg")
        == "85df47708700d685cf77bae0fed481298d00ab8808885de9ae63e2e78cab9fa8"
    )
    for d in ("cache", "home", "scratch", "temp"):
        (root / d).mkdir(mode=0o700)
    env = {k: v for k, v in os.environ.items() if not k.startswith("OMEGA_")}
    env.update(
        OMEGA_LIBRARY_ROOT=str(root / "source/library"),
        XDG_CACHE_HOME=str(root / "cache"),
        LOCALAPPDATA=str(root / "cache"),
        HOME=str(root / "home"),
        USERPROFILE=str(root / "home"),
        TMPDIR=str(root / "temp"),
        TEMP=str(root / "temp"),
        TMP=str(root / "temp"),
    )
    return root, env


def measure(root, env, label, cmd):
    with (
        (root / (label + ".stdout")).open("wb") as stdout,
        (root / (label + ".stderr")).open("wb") as stderr,
    ):
        started = time.perf_counter_ns()
        proc = subprocess.Popen(
            cmd,
            stdin=subprocess.DEVNULL,
            stdout=stdout,
            stderr=stderr,
            cwd=root / "scratch",
            env=env,
        )
        if os.name == "nt":
            rc = proc.wait()
            end = time.perf_counter_ns()

            class Counters(ctypes.Structure):
                _fields_ = [("cb", ctypes.c_uint32), ("faults", ctypes.c_uint32)] + [
                    (key, ctypes.c_size_t)
                    for key in (
                        "peak",
                        "working",
                        "pp",
                        "p",
                        "np",
                        "n",
                        "page",
                        "peakpage",
                    )
                ]

            mem = Counters()
            mem.cb = ctypes.sizeof(mem)
            if not ctypes.windll.psapi.GetProcessMemoryInfo(
                ctypes.c_void_p(int(proc._handle)), ctypes.byref(mem), mem.cb
            ):
                raise ctypes.WinError()
            rss = mem.peak // 1024
        else:
            _, status, usage = os.wait4(proc.pid, 0)
            end = time.perf_counter_ns()
            rc = proc.returncode = os.waitstatus_to_exitcode(status)
            rss = usage.ru_maxrss
    return {
        "label": label,
        "cmd": list(map(str, cmd)),
        "rc": rc,
        "seconds": (end - started) / 1e9,
        "peak_kib": rss,
    }


def record(row):
    print(json.dumps(row), flush=True)
    with (out / "results.jsonl").open("a", encoding="utf-8") as log:
        print(json.dumps(row), file=log)


def build(root, env, label, timing=False, target="windows_x86_64"):
    fixture = root / FIXTURE
    cmd = [omega, "build", "--target", target]
    if timing:
        cmd.append("--timings")
    row = measure(root, env, label, [*cmd, fixture / "build.omg"])
    row.update(
        client=root.name,
        build_sha256=digest(fixture / "build.omg"),
        main_sha256=digest(fixture / "main.omg"),
    )
    if row["rc"] == 0:
        lines = (root / (label + ".stdout")).read_text().splitlines()
        matches = [
            line.removeprefix("published native output to ")
            for line in lines
            if line.startswith("published native output to ")
        ]
        if len(matches) != 1:
            raise RuntimeError(f"expected one native publication in {lines}")
        binary = root / "scratch" / matches[0]
        assert binary.resolve().is_relative_to(fixture / "build")
        data = binary.read_bytes()
        if target.startswith("windows"):
            assert data[:2] == b"MZ"
            pe = struct.unpack_from("<I", data, 0x3C)[0]
            assert (
                data[pe : pe + 4] == b"PE\0\0"
                and struct.unpack_from("<H", data, pe + 4)[0] == 0x8664
            )
            assert struct.unpack_from("<H", data, pe + 24)[0] == 0x20B
        row.update(binary=str(binary), bytes=len(data), binary_sha256=digest(binary))
    record(row)
    if row["rc"]:
        sys.exit(f"{root}/{label}.stderr")
    return Path(row["binary"])


metadata = {
    "revision": rev,
    "omega_sha256": digest(omega),
    "host": platform.uname()._asdict(),
    "cpu": os.environ.get("PROCESSOR_IDENTIFIER"),
    "os_page_cache": "uncontrolled; source staged before each trial",
}
if os.name == "nt":
    cpu = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor).Name",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    metadata["cpu"] = cpu.stdout.strip()
else:
    metadata["cpu"] = next(
        line.split(":", 1)[1].strip()
        for line in Path("/proc/cpuinfo").read_text().splitlines()
        if line.startswith("model name")
    )
    metadata["affinity"] = sorted(os.sched_getaffinity(0))
    metadata["nice"] = os.getpriority(os.PRIO_PROCESS, 0)
    metadata["cgroup_cpu_max"] = Path("/sys/fs/cgroup/cpu.max").read_text().strip()
    metadata["cgroup_memory_max"] = (
        Path("/sys/fs/cgroup/memory.max").read_text().strip()
    )
(out / "host.json").write_text(json.dumps(metadata, indent=2))
if args.profile_only:
    root, env = client("sampled")
    record(
        measure(
            root,
            env,
            "sampled",
            [
                "gprofng",
                "collect",
                "app",
                "-o",
                str(root / "sample.er"),
                "-p",
                "on",
                omega,
                "build",
                "--target",
                "windows_x86_64",
                root / FIXTURE / "build.omg",
            ],
        )
    )
    sys.exit()
if args.verify_only:
    root, env = client("verify")
    target = "windows_x86_64" if os.name == "nt" else "linux_x86_64"
    binary = build(root, env, "host-build", target=target)
    record(measure(root, env, "native", [binary]))
    record(
        measure(
            root,
            env,
            "psi",
            [omega, "run", "--target", target, root / FIXTURE / "build.omg"],
        )
    )
    if os.name != "nt":
        record(
            measure(
                root,
                env,
                "psi-windows",
                [
                    omega,
                    "run",
                    "--target",
                    "windows_x86_64",
                    root / FIXTURE / "build.omg",
                ],
            )
        )
    sys.exit()
for i in range(1, args.count + 1):
    for timing in (True, False) if i % 2 else (False, True):
        root, env = client(("instrumented" if timing else "wall") + f"-{i}")
        assert (
            not any((root / "cache").iterdir())
            and not (root / FIXTURE / "build").exists()
        )
        binary = build(root, env, "cold", timing)
        original_binary_sha = digest(binary)
        if os.name == "nt":
            record(measure(root, env, "native-cold", [binary]))
        src = root / FIXTURE / "main.omg"
        data = src.read_bytes()
        assert data.count(b"portable_source.tmp") == 5
        src.write_bytes(data.replace(b"portable_source.tmp", b"portable_edited.tmp"))
        (root / "cache-after-prime.txt").write_text(
            "\n".join(str(p.relative_to(root)) for p in (root / "cache").rglob("*"))
        )
        binary = build(root, env, "edited", timing)
        assert digest(binary) != original_binary_sha, (
            "stale executable after filename edit"
        )
        if os.name == "nt":
            record(measure(root, env, "native-edited", [binary]))
