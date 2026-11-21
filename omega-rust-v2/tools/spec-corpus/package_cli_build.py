"""Build the V2 package CLI and identify its executable from Cargo output."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).absolute().parents[2]
BUILD = ['mbx', 'build', '--locked', '-p', 'omega', '--bin', 'omega', '--message-format=json']


def build(directory, env, record, log_name):
    record.update(command=BUILD, log=log_name)
    with (directory / log_name).open('xb') as log:
        try:
            result = subprocess.run(BUILD, cwd=ROOT, env=env, stdin=subprocess.DEVNULL,
                                    capture_output=True, timeout=600)
        except subprocess.TimeoutExpired as error:
            log.write((error.stdout or b'') + b'\n' + (error.stderr or b'') + b'\nbuild timed out\n')
            raise ValueError('build timed out') from error
        except OSError as error:
            log.write(str(error).encode('utf-8', 'backslashreplace'))
            raise
        log.write(result.stdout + b'\n' + result.stderr)
    record['exit'] = result.returncode
    if result.returncode:
        raise ValueError(f'build exited {result.returncode}')
    executables = []
    for line in result.stdout.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue  # mbx may print non-Cargo progress lines
        if not isinstance(row, dict) or row.get('reason') != 'compiler-artifact':
            continue
        target, manifest = row.get('target'), row.get('manifest_path')
        if (isinstance(target, dict) and target.get('name') == 'omega' and target.get('kind') == ['bin']
                and isinstance(manifest, str) and Path(manifest) == ROOT / 'application/Cargo.toml'
                and isinstance(row.get('executable'), str)):
            executables.append(Path(row['executable']))
    if len(executables) != 1 or not executables[0].is_absolute() or not executables[0].is_file():
        raise ValueError('expected one built omega binary compiler-artifact')
    record['executable'] = str(executables[0])
    return executables[0]
