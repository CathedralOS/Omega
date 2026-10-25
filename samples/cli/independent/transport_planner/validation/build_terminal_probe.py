"""Link validation source to the verified compiler build's existing Rust libraries.

The portable Cargo templates generate a private workspace under ignored build/. This bounded route avoids a second
whole compiler build caused by an independent workspace's absolute source paths.
Every selected input and the exact rustc invocation is recorded for review.
"""
import argparse, hashlib, json, shutil, subprocess
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('verified_target', type=Path, nargs='?')
parser.add_argument('--prepare-cargo', action='store_true', help='materialize the portable private validation workspace without building')
arguments = parser.parse_args()
root = Path(__file__).resolve().parents[1]
if arguments.verified_target is None:
    workspace = root/'build/terminal-probe-cargo'
    workspace.mkdir(parents=True, exist_ok=True)
    for source, destination in [('Cargo.template.toml', 'Cargo.toml'), ('Cargo.template.lock', 'Cargo.lock')]:
        shutil.copyfile(root/'validation/terminal_probe'/source, workspace/destination)
    if arguments.prepare_cargo:
        print(workspace/'Cargo.toml')
        raise SystemExit(0)
    command = [shutil.which('mbx') or shutil.which('cargo') or 'cargo', 'build', '--release',
               '--manifest-path', str(workspace/'Cargo.toml'), '--target-dir', str(workspace/'target')]
    result = subprocess.run(command, capture_output=True, text=True)
    (root/'build/terminal-probe-cargo-build.json').write_text(json.dumps(dict(command=command,
        exit=result.returncode, stdout=result.stdout, stderr=result.stderr), indent=2)+'\n')
    print(result.stdout, end=''); print(result.stderr, end='')
    assert result.returncode == 0, 'private validation workspace did not build'
    print(workspace/'target/release/ind01-terminal-probe')
    raise SystemExit(0)
assert not arguments.prepare_cargo, '--prepare-cargo is for the portable workspace'
target = arguments.verified_target.resolve()/'release'
command = ['rustc', '--edition=2024', '--crate-name', 'ind01_terminal_probe',
           '-C', 'opt-level=2', '-C', 'debuginfo=0',
           str(root/'validation/terminal_probe/src/main.rs'), '-o', str(root/'build/ind01-terminal-probe')]
inputs = []
for crate in ['compiler', 'terminal_runtime', 'package_manager', 'target', 'terminal_interpreter', 'serde_json']:
    candidates = list(target.glob(f'build/**/out/lib{crate}-*.rlib')) + list(target.glob(f'deps/lib{crate}-*.rlib'))
    assert candidates, crate
    # This optional fast path reuses this executor's recorded build cache.
    # Its target crate was built first by the main workspace, then duplicated
    # by the cancelled separate-workspace build; retain the former identity.
    # Changed producer crates come from the latest recorded main build. Rustc
    # rejects a mismatched dependency graph. For another cache use Cargo.
    select = min if crate == 'target' else max
    selected = select(candidates, key=lambda path: path.stat().st_mtime_ns)
    metadata = selected.with_suffix('.rmeta')
    assert metadata.is_file(), metadata
    command += ['--extern', f'{crate}={metadata}', '--extern', f'{crate}={selected}']
    inputs.append(dict(crate=crate, path=str(selected), sha256=hashlib.sha256(selected.read_bytes()).hexdigest()))
for directory in sorted(path for path in target.glob('build/**/out') if path.is_dir()):
    command += ['-L', f'dependency={directory}']
if (target/'deps').is_dir():
    command += ['-L', f'dependency={target/"deps"}']
result = subprocess.run(command, capture_output=True, text=True)
record = dict(command=command, libraries=inputs, exit=result.returncode, stdout=result.stdout, stderr=result.stderr,
              source_sha256=hashlib.sha256((root/'validation/terminal_probe/src/main.rs').read_bytes()).hexdigest())
(root/'build/terminal-probe-link.json').write_text(json.dumps(record,indent=2)+'\n')
print(result.stdout, end=''); print(result.stderr, end='')
assert result.returncode == 0, 'validation harness did not build'
print('PASS: validation harness linked to recorded main-build libraries')
