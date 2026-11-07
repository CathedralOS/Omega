"""Sequential command recorder; keeps exits and exact streams under build/."""
import argparse, json, subprocess, time
from pathlib import Path
parser = argparse.ArgumentParser()
parser.add_argument('--name', required=True)
parser.add_argument('--input', type=Path)
parser.add_argument('--expect-exit', type=int)
parser.add_argument('command', nargs=argparse.REMAINDER)
arguments = parser.parse_args()
command = arguments.command
if command and command[0] == '--':
    command = command[1:]
assert command, 'command required'
root = Path(__file__).resolve().parents[1]
payload = arguments.input.read_bytes() if arguments.input else b''
started = time.monotonic()
result = subprocess.run(command, input=payload, capture_output=True)
record = dict(command=command, cwd=str(Path.cwd()), input=str(arguments.input) if arguments.input else None,
              exit=result.returncode, elapsed_seconds=time.monotonic()-started,
              stdout=result.stdout.decode('utf-8',errors='replace'), stderr=result.stderr.decode('utf-8',errors='replace'))
destination = root/'build'/'commands'/f'{arguments.name}.json'
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_text(json.dumps(record, indent=2)+'\n')
print(record['stdout'], end='')
print(record['stderr'], end='')
print(f"recorded {arguments.name}: exit {result.returncode}, {record['elapsed_seconds']:.3f}s")
if arguments.expect_exit is not None:
    assert result.returncode == arguments.expect_exit, record
