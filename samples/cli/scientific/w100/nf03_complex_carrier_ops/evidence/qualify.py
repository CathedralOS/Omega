"""Sequential source qualification without creating NF01 package metadata.

Run from any directory: python qualify.py --cli /absolute/path/to/omega
All copied .omg implementation/main sources are byte-identical. Only scratch
Build metadata differs. Full commands, outputs, input sources and digests remain
under --output. This harness never changes dependency sources or product code.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--cli', required=True)
parser.add_argument('--output', default='/tmp/nf03-qualification')
args = parser.parse_args()
repository = Path(__file__).resolve().parents[6]
samples = Path(__file__).resolve().parents[1]
output = Path(args.output).resolve()
output.mkdir(parents=True, exist_ok=True)
library = output / 'numerics'
(library / 'complex').mkdir(parents=True, exist_ok=True)
carrier = repository / 'source/library/numerics/complex/complex_carrier_ops.omg'
shutil.copy2(carrier, library / 'complex/complex_carrier_ops.omg')
(library / 'build.omg').write_text('machine build(builder: &mut Build) { Build::package(builder, "nf03_qualification"); }\n')
environment = os.environ.copy()
environment['OMEGA_LIBRARY_ROOT'] = str(repository / 'source/library')
shutil.copy2(samples / 'ORACLES.md', output / 'ORACLES.md')
records = []
def run(name, route, arguments):
    command = [str(Path(args.cli).resolve()), *map(str, arguments)]
    started = time.monotonic()
    with (output / f'{name}-{route}.log').open('w') as log:
        log.write('$ ' + shlex.join(command) + '\n')
        log.flush()
        result = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=log,
                                stderr=subprocess.STDOUT, env=environment, timeout=300)
    record = dict(project=name, route=route, command=command,
                  exit=result.returncode, seconds=round(time.monotonic()-started, 3))
    records.append(record)
    (output / 'results.json').write_text(json.dumps(records, indent=2)+'\n')
    print(json.dumps(record), flush=True)
    return result.returncode

for name in ['phasor_power', 'quadrature_rotation', 'invalid_alias']:
    project = output / name
    project.mkdir(exist_ok=True)
    shutil.copy2(samples / name / 'main.omg', project / 'main.omg')
    (project / 'build.omg').write_text(f'''machine build(builder: &mut Build) {{
    Build::application(builder, "nf03_{name}");
    Build::depend(builder, Source::Path {{ location: "{repository}/source/library/std" }});
    Build::depend_as(builder, "numerics", Source::Path {{ location: "{library}" }});
    Build::bind(builder, linux_x86_64::ProgramEntry, Main::main);
}}\n''')
    run(name, 'update', ['update', '--project', project, '--target', 'linux_x86_64'])
    review = project / 'build/package-manager/review-linux_x86_64.txt'
    if review.exists():
        # Record the complete local-only findings before accepting. These are
        # disposable harness inputs; acceptance does not claim a security audit.
        shutil.copy2(review, output / f'{name}-review.txt')
        review.write_bytes(re.sub(rb'^(decision row [0-9a-f]+) pending$',
                                 rb'\1 accept', review.read_bytes(), flags=re.MULTILINE))
        run(name, 'resume', ['update', '--project', project, '--resume'])
    run(name, 'check', ['--check', '--target', 'linux_x86_64', project / 'main.omg'])
    if name != 'invalid_alias':
        run(name, 'native', ['run', project / 'main.omg'])
        run(name, 'terminal-comparison', ['run', '--both', project / 'main.omg'])

(output / 'sources.sha256').write_text('\n'.join(
    f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(output)}'
    for p in sorted(output.rglob('*.omg')) if 'build/package-manager' not in str(p))+'\n')

valid_routes = [r for r in records if r['project'] != 'invalid_alias'
                and r['route'] in ('check', 'native', 'terminal-comparison')]
positives = len(valid_routes) == 6 and all(r['exit'] == 0 for r in valid_routes)
terminal = all('interp exit: 0' in (output / f'{name}-terminal-comparison.log').read_text()
               for name in ('phasor_power', 'quadrature_rotation'))
negative_record = next(r for r in records if r['project'] == 'invalid_alias' and r['route'] == 'check')
negative_text = (output / 'invalid_alias-check.log').read_text()
negative = negative_record['exit'] == 1 and any(
    fragment in negative_text.lower() for fragment in ('overlap', 'exclusive', 'already borrowed', 'conflicting loan'))
verdict = dict(valid_routes=positives, terminal_agreement=terminal, intended_rejection=negative)
(output / 'verdict.json').write_text(json.dumps(verdict, indent=2)+'\n')
print(json.dumps(verdict), flush=True)
raise SystemExit(0 if all(verdict.values()) else 1)
