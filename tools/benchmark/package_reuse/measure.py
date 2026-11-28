"""Measure fresh and edited builds of the filesystem fixture using a release omega."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import time

BASE = "e6477bc08c36684b78cca182ba91bd012b4c5a4d"
FIXTURE = Path("source-rust/application/tests/fixtures/filesystem/run/public_values")


def cpu_model():
    if os.name == "nt":
        import winreg
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"HARDWARE\DESCRIPTION\System\CentralProcessor\0") as key:
            return winreg.QueryValueEx(key, "ProcessorNameString")[0].strip()
    info = Path("/proc/cpuinfo")
    if info.exists():
        for line in info.read_text().splitlines():
            if line.startswith("model name"):
                return line.partition(":")[2].strip()
    return platform.processor()


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def measure(label, compiler, fixture, library, output, cache, *, profile=False):
    run = output / label
    run.mkdir()
    work = run / "cwd"
    work.mkdir()
    cache.mkdir(exist_ok=True)
    before = sum(1 for p in cache.rglob("*") if p.is_file())
    env = os.environ.copy()
    env["LOCALAPPDATA" if os.name == "nt" else "XDG_CACHE_HOME"] = str(cache)
    env["OMEGA_LIBRARY_ROOT"] = str(library)
    command = [str(compiler), "build", "--target", "windows_x86_64",
               "--build-dir", str(run / "product")]
    if profile:
        command.append("--timings")
    command.append(str(fixture / "build.omg"))
    start = time.perf_counter_ns()
    process = subprocess.run(command, cwd=work, env=env, stdin=subprocess.DEVNULL, capture_output=True)
    elapsed = time.perf_counter_ns() - start
    (run / "stdout.txt").write_bytes(process.stdout)
    (run / "stderr.txt").write_bytes(process.stderr)
    row = dict(case=label, command=command, cwd=str(work), cache_files_before=before,
               cache_files_after=sum(1 for p in cache.rglob("*") if p.is_file()),
               returncode=process.returncode, elapsed_ns=elapsed,
               build_sha256=sha256(fixture / "build.omg"), main_sha256=sha256(fixture / "main.omg"))
    images = list((run / "product").rglob("*.exe"))
    if process.returncode == 0 and len(images) == 1:
        image = images[0]
        data = image.read_bytes()
        pe = struct.unpack_from("<I", data, 0x3c)[0]
        row["pe_x86_64"] = (data.startswith(b"MZ") and data[pe:pe+4] == b"PE\0\0" and
                            struct.unpack_from("<H", data, pe+4)[0] == 0x8664 and
                            struct.unpack_from("<H", data, pe+24)[0] == 0x20b)
        row.update(image=str(image), image_bytes=len(data), image_sha256=sha256(image))
        if os.name == "nt" and row["pe_x86_64"]:
            native = subprocess.run([str(image)], cwd=work, capture_output=True)
            row.update(native_exit=native.returncode, native_stdout_hex=native.stdout.hex(),
                       native_stderr_hex=native.stderr.hex(), remaining_files=sorted(p.name for p in work.iterdir()))
    print(json.dumps(row), flush=True)
    (run / "result.json").write_text(json.dumps(row, indent=2) + "\n")
    if not row.get("pe_x86_64"):
        raise SystemExit(f"{label} did not publish one x86-64 PE executable; inspect {run}")
    if os.name == "nt" and (row["native_exit"] or row["native_stdout_hex"] or row["native_stderr_hex"] or row["remaining_files"]):
        raise SystemExit(f"{label} executable did not meet the fixture contract")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("output", type=Path, help="new output directory")
    parser.add_argument("--samples", type=int, default=3)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be positive")
    compiler = args.compiler.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir()
    repo = Path(__file__).resolve().parents[3]

    def git(*args):
        return subprocess.check_output(["git", *args], cwd=repo)

    for name in ("build.omg", "main.omg"):
        if (repo / FIXTURE / name).read_bytes() != git("show", BASE + ":" + (FIXTURE / name).as_posix()):
            raise SystemExit("fixture differs from benchmark base")
    (output / "host.json").write_text(json.dumps(dict(
        source_head=git("rev-parse", "HEAD").decode().strip(), fixture_base=BASE,
        compiler_sha256=sha256(compiler), compiler=str(compiler), platform=platform.platform(),
        compiler_version=subprocess.check_output([str(compiler), "--version"], text=True).strip(),
        rayon_workers=os.getenv("RAYON_NUM_THREADS"),
        processor=cpu_model(), logical_cpus=os.cpu_count(),
        system_source_files=len(list((repo / "source/library/system").rglob("*.omg")))), indent=2))
    library = repo / "source/library"
    for sample in range(1, args.samples + 1):
        measure(f"cold-{sample}", compiler, repo / FIXTURE, library, output, output / f"cold-cache-{sample}")
    # The copy keeps the original relative declaration and the same source dependency
    # location for both edited-build runs. Only authored, committed files enter it.
    clone = output / "source-copy"
    for rel in git("ls-files", "-z", "--", FIXTURE.as_posix(), "source/library").split(b"\0"):
        if rel:
            path = Path(os.fsdecode(rel))
            (clone / path).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(repo / path, clone / path)
    cache = output / "edited-cache"
    measure("prepare", compiler, clone / FIXTURE, library, output, cache)
    main = clone / FIXTURE / "main.omg"
    previous = main.read_bytes()
    for sample in range(1, args.samples + 1):
        updated = previous.replace(b"portable_copy.tmp", f"portable_edited_{sample}.tmp".encode())
        if updated == previous:
            raise SystemExit("expected filesystem operation absent")
        main.write_bytes(updated)
        measure(f"edited-{sample}", compiler, clone / FIXTURE, library, output, cache)
    measure("profile", compiler, repo / FIXTURE, library, output, output / "profile-cache", profile=True)


if __name__ == "__main__":
    main()
