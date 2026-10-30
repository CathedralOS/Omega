"""One isolated-cache, exact-producer diagnostic compile and native execution."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

REPOSITORY = Path.cwd()
sys.path.insert(0, str(REPOSITORY / "tools/benchmark"))
import benchmark

producer = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
if sys.platform != "win32":
    raise RuntimeError("this experiment isolates the Windows source-cache location")
output.mkdir(parents=True, exist_ok=False)
project = output / "input"
project.mkdir()
cache = output / "cache"
cache.mkdir()
source = REPOSITORY / "samples/cli/facts/thirtytwo_arg_stack_calling_plan/other_machine"
for filename in ["main.omg", "build.omg"]:
    shutil.copyfile(source / filename, project / filename)
identity = benchmark.sha256_file(producer)
profile = benchmark.require_optimized(producer)
os.environ["LOCALAPPDATA"] = str(cache)
os.environ.pop("OMEGA_PHASE_PROFILE", None)
benchmark.settle_package_review(producer, project, "windows_x86_64")
record_paths=list(cache.rglob("*.rec"))
assert record_paths, "review created no retained records"
withheld=[]
if sys.argv[3]=='B':
    archive=output/'review-records';archive.mkdir()
    for path in record_paths:
        assert path.resolve().is_relative_to(cache.resolve())
        destination=archive/path.relative_to(cache)
        destination.parent.mkdir(parents=True,exist_ok=True)
        withheld.append({'relative_path':str(path.relative_to(cache)),'sha256':benchmark.sha256_file(path),'bytes':path.stat().st_size})
        path.rename(destination)
os.environ["OMEGA_PHASE_PROFILE"] = "1"
command = [str(producer), "--timings", "--target", "windows_x86_64", "--build-dir", str(output / "build"), str(project / "main.omg")]
code, wall, memory, transcript = benchmark.capture_compile(command, REPOSITORY)
(output / "output.txt").write_text(transcript, encoding="utf-8")
stages, published = benchmark.parse_timings(transcript)
assert code == 0 and published and Path(published).is_file(), transcript[-3000:]
artifact = Path(published)
execution = subprocess.run([str(artifact)], capture_output=True, timeout=30)
assert benchmark.sha256_file(producer) == identity
for filename in ["main.omg", "build.omg"]:
    assert benchmark.sha256_file(source / filename) == benchmark.sha256_file(project / filename)
record = dict(withheld_records=withheld, command=command, producer_sha256=identity, producer_profile=profile,
              source_sha256=benchmark.sha256_file(source / "main.omg"),
              build_sha256=benchmark.sha256_file(source / "build.omg"),
              revision=subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
              compiler_exit=code, wall_ms=wall, peak_job_memory_bytes=memory, stages=stages,
              artifact_sha256=benchmark.sha256_file(artifact), artifact_bytes=artifact.stat().st_size,
              native_exit=execution.returncode, stdout_hex=execution.stdout.hex(), stderr_hex=execution.stderr.hex())
(output / "result.json").write_text(json.dumps(record, indent=2), encoding="utf-8")
print(json.dumps(record, indent=2))
assert execution.returncode == 0
