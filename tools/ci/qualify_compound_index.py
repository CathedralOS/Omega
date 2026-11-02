"""Temporary bounded observations after the ordinary compound-index pilot."""
from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / 'build/compiler-pilot'
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import require_optimized
import sample_native_gate


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, text=True).strip()


def execute(command, directory, label, timeout=240):
    started = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True,
                            stdin=subprocess.DEVNULL, timeout=timeout)
    (directory / (label + '.stdout')).write_text(result.stdout)
    (directory / (label + '.stderr')).write_text(result.stderr)
    return result, time.monotonic() - started


def prepare(cli, project, target, directory, label):
    failure, review = sample_native_gate.accept_review(str(cli), str(project), target, 180)
    (directory / (label + '.review.txt')).write_text(review)
    assert failure is None, (label, failure, review)


summary = json.loads((OUTPUT / 'summary.json').read_text())
assert summary['passed']
producer = summary['producer']
revision = git('rev-parse', 'HEAD')
assert revision == producer['revision'] and not git('status', '--porcelain')
cli = Path(producer['cli'])
profile = require_optimized(cli)
assert profile == producer['cli_profile']
output = OUTPUT / 'compound-index-extra'
output.mkdir(exist_ok=False)
source_files = git('ls-files', '--', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                   'omega-rust', 'source/library').splitlines()
inputs = {name: digest(ROOT / name) for name in source_files}
(output / 'source-inputs.json').write_text(json.dumps(
    {'revision': revision, 'tree': git('rev-parse', 'HEAD^{tree}'), 'files': inputs}, indent=2) + '\n')
sdk = {name: value for name, value in inputs.items() if name.startswith('source/library/')}
for name in sdk:
    destination = output / 'sdk-snapshot' / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / name, destination)
(output / 'sdk-inputs.json').write_text(json.dumps(sdk, indent=2) + '\n')
# Keep the exact ordinary pilot SDK origin during observations; the copy is
# retained for independent replay, never substituted for this producer's input.
os.environ['OMEGA_LIBRARY_ROOT'] = str(ROOT / 'source/library')
public_rows = []
for place in ['', 'target_body', 'match_join']:
    project = ROOT / 'samples/cli/facts/compound_index_guard' / place
    label = 'public-' + (place or 'root')
    prepare(cli, project, producer['target'], output, label)
    command = [str(cli), 'run', '--both', str(project / 'main.omg')]
    result, seconds = execute(command, output, label)
    lines = result.stderr.splitlines()
    passed = result.returncode == 0 and 'native exit 0' in lines and 'interp exit: 0' in lines
    public_rows.append(dict(project=str(project), command=command, returncode=result.returncode,
                            seconds=seconds, native_zero='native exit 0' in lines,
                            Terminal_zero='interp exit: 0' in lines, passed=passed))
    (output / 'public-raw.json').write_text(json.dumps(public_rows, indent=2) + '\n')
    assert passed, (label, result.stdout, result.stderr)
observations = []
for name, place, old, new in [
    ('selected-store', 'target_body', 'main.grid[main.b * 20 + main.a] = 7;',
     'main.grid[main.b * 20 + main.a] = 8;'),
    ('first-match-route', 'match_join', 'true -> mark_coordinate(main.a, &mut route, 17),',
     'true -> mark_coordinate(main.a, &mut route, 19),'),
]:
    source_project = ROOT / 'samples/cli/facts/compound_index_guard' / place
    project = output / 'mutants' / name
    project.mkdir(parents=True)
    for name_in_project in ['main.omg', 'build.omg', 'sample.json', 'expected.txt']:
        source = source_project / name_in_project
        if source.is_file():
            shutil.copy2(source, project / source.name)
    main = project / 'main.omg'
    original = main.read_text()
    assert old in original
    if name == 'selected-store':
        assert original.count(old) == 1
    main.write_text(original.replace(old, new, 1))
    before = {p.name: digest(p) for p in project.iterdir() if p.is_file()}
    prepare(cli, project, producer['target'], output, name)
    command = [str(cli), 'run', '--both', str(main)]
    result, seconds = execute(command, output, name)
    lines = result.stderr.splitlines()
    signals = [int(match.group(1)) for line in lines
               if (match := re.fullmatch(r'native signal (\d+)', line))]
    terminal_trap = 'interp crash: Trap (host presentation agrees)' in lines
    passed = result.returncode == 255 and len(signals) == 1 and terminal_trap
    row = dict(name=name, command=command, returncode=result.returncode, seconds=seconds,
               source_main_sha256=digest(source_project / 'main.omg'), input_sha256=before,
               mutation={'old': old, 'new': new, 'replacements': 1},
               native_signal=signals, terminal_trap=terminal_trap, passed=passed)
    observations.append(row)
    (output / 'counterfactuals.json').write_text(json.dumps(observations, indent=2) + '\n')
    assert all(digest(project / path) == value for path, value in before.items())
    assert passed, (name, result.stdout, result.stderr)

artifacts = []
project = ROOT / 'samples/cli/facts/compound_index_guard/target_body'
for target in ['windows_x86_64', 'linux_x86_64', 'macos_arm64', 'linux_arm64']:
    prepare(cli, project, target, output, target)
    report_file = output / (target + '.report.txt')
    command = [str(cli), '--build-dir', str(output / 'artifacts' / target),
               '--report-file', str(report_file), '--target', target, str(project / 'main.omg')]
    result, seconds = execute(command, output, target)
    report = report_file.read_text() if report_file.exists() else ''
    found = []
    for section in report.split('product: native artifact\n')[1:]:
        assert re.search(r'^target: (\w+)$', section, re.M).group(1) == target
        published = re.search(r'^published native output to (.+)$', section, re.M)
        if published:
            image = Path(published.group(1))
        else:
            package = Path(re.search(r'^published macOS package to (.+)$', section, re.M).group(1))
            candidates = [path for path in (package / 'Contents/MacOS').iterdir()
                          if path.is_file() and path.read_bytes()[:4] == b'\xcf\xfa\xed\xfe'
                          and int.from_bytes(path.read_bytes()[12:16], 'little') == 2]
            assert len(candidates) == 1
            image = candidates[0]
        data = image.read_bytes()
        kind = ('PE' if data[:2] == b'MZ' else 'ELF' if data[:4] == b'\x7fELF'
                else 'Mach-O' if data[:4] == b'\xcf\xfa\xed\xfe' else 'unknown')
        assert kind == ('PE' if target.startswith('windows') else 'ELF' if target.startswith('linux') else 'Mach-O')
        found.append(dict(target=target, path=str(image), sha256=digest(image), bytes=len(data),
                          format=kind, executed=False))
    passed = result.returncode == 0 and len(found) == 1
    artifacts.append(dict(target=target, command=command, seconds=seconds,
                          returncode=result.returncode, artifacts=found, passed=passed))
    (output / 'four-target-artifacts.json').write_text(json.dumps(artifacts, indent=2) + '\n')
    assert passed, (target, result.stdout, result.stderr)

assert all(digest(ROOT / name) == value for name, value in inputs.items())
assert require_optimized(cli) == profile
assert revision == git('rev-parse', 'HEAD') and not git('status', '--porcelain')
(output / 'verdict.json').write_text(json.dumps(dict(
    revision=revision, producer=producer, source_inputs=len(inputs), sdk_files=len(sdk),
    actual_library_root=str(ROOT / 'source/library'),
    retained_sdk_snapshot=str(output / 'sdk-snapshot/source/library'),
    public_raw=public_rows, counterfactuals=observations, four_target_artifacts=artifacts, passed=True,
    scope='Two actual counterfactual native signals with agreeing Terminal Trap; four explicit native artifacts, foreign execution unclaimed.'), indent=2) + '\n')
