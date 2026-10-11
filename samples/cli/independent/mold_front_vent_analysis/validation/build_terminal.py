"""Link this private harness against the exact verified compiler build's rlibs.
Avoids a second Cargo workspace or changes to any shared compiler export.
Usage: python3 validation/build_terminal.py /absolute/path/to/target/release
"""
from pathlib import Path
import subprocess
import sys
from reference import ROOT
release = Path(sys.argv[1]).resolve()
dependencies = release
rlibs = list(release.rglob('*.rlib'))
command = ['rustc', '--edition=2024', '-C', 'opt-level=1', str(ROOT/'validation/terminal.rs'),
           '-o', str(ROOT/'build/terminal-validator')]
for directory in sorted({path.parent for path in rlibs + list(release.rglob('*.so'))}):
    command += ['-L',f'dependency={directory}']
for name in ['compiler','package_manager','target','terminal_interpreter']:
    matches = [path for path in rlibs if path.name.startswith(f'lib{name}-')]
    if len(matches) != 1:
        raise RuntimeError(f'Expected one exact {name} rlib, observed {matches}')
    command += ['--extern',f'{name}={matches[0]}']
    metadata=matches[0].with_suffix('.rmeta')
    if metadata.exists():
        command += ['--extern',f'{name}={metadata}']
print(' '.join(command),flush=True)
result=subprocess.run(command)
raise SystemExit(result.returncode)
