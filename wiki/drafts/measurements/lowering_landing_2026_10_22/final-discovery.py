"""Replay existing fact rows on both frozen producers; compare exact artifacts."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path.cwd()
OUTPUT = ROOT/'build/lowering-landing/final/discovery/validation.json'
SCRATCH = ROOT/'build/lowering-landing/final/discovery/artifacts'
SCRATCH.mkdir(parents=True, exist_ok=True)
BINARIES = {name: ROOT/f'build/lowering-landing/omega-{"baseline" if name == "baseline" else "final"}' for name in ('baseline','candidate')}
PLACES = ('local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body')
SUBJECTS = [f'samples/cli/facts/{fact}/{place}/main.omg' for fact in ('length','open_const_implementation') for place in PLACES]
SUBJECTS += [f'tests/omega/pass/traits/{name}/main.omg' for name in ('unused_trait_implementation_member_exit','runtime_explicit_implementation_arguments_exit','applied_conformance_captured_fresh_regions_exit')]
CONTROL_SUBJECTS = [
    'tests/omega/fail/traits/implementation_argument_wrong_contract/main.omg',
    'tests/omega/fail/traits/implementation_argument_wrong_type_arguments/main.omg',
    'tests/omega/fail/generics/applied_implementation_unused_method_carrier/main.omg',
]
records = []

def execute(name, subject, mode, arguments):
    command = [str(BINARIES[name]), *arguments, subject]
    result = subprocess.run(command, capture_output=True, text=True, timeout=90)
    row = dict(producer=name, subject=subject, mode=mode, command=command, exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
    records.append(row)
    OUTPUT.write_text(json.dumps(dict(records=records),indent=2)+'\n')
    return row

if '--summarize' in sys.argv:
    records = json.loads(OUTPUT.read_text())['records']
else:
    for index, subject in enumerate(SUBJECTS):
        for name in BINARIES:
            execute(name, subject, 'both', ['run','--both','--keep'])
            artifact = SCRATCH/f'{index}-{name}.psi'
            row = execute(name, subject, 'terminal', ['inspect-terminal','--machine', 'Main::main' if 'applied_conformance_captured_fresh_regions_exit' in subject else 'main','--output',str(artifact)])
            if row['exit'] == 0:
                row['terminal_sha256'] = hashlib.sha256(artifact.read_bytes()).hexdigest()
            invalid = str(Path(subject).parent/'invalid/main.omg')
            if (ROOT/invalid).exists():
                execute(name, invalid, 'reject', ['--check'])
        OUTPUT.write_text(json.dumps(dict(records=records),indent=2)+'\n')
        print(index+1, subject, flush=True)
    for subject in CONTROL_SUBJECTS:
        for name in BINARIES:
            execute(name, subject, 'reject', ['--check'])
summary = []
for subject in dict.fromkeys(row['subject'] for row in records):
    for mode in dict.fromkeys(row['mode'] for row in records if row['subject']==subject):
        pair = [next(row for row in records if row['subject']==subject and row['mode']==mode and row['producer']==name) for name in BINARIES]
        old,new = pair
        # --keep reports a per-process scratch directory, outside guest streams.
        def diagnostics(row):
            return '\n'.join(line for line in row['stderr'].splitlines() if not line.startswith('build dir kept: '))
        equal = old['exit']==new['exit'] and old['stdout']==new['stdout'] and diagnostics(old)==diagnostics(new)
        if mode=='terminal' and old['exit']==0:
            equal = old['terminal_sha256']==new.get('terminal_sha256') and old['stdout']==new['stdout'] and new['exit']==0
        summary.append(dict(subject=subject,mode=mode,baseline_exit=old['exit'],candidate_exit=new['exit'],equal=equal))
OUTPUT.write_text(json.dumps(dict(source_hashes={str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for subject in SUBJECTS + CONTROL_SUBJECTS for path in (ROOT/subject).parent.rglob('*.omg')},binary_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in BINARIES.items()},records=records,summary=summary),indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
assert all(row['equal'] for row in summary)
assert all(row['candidate_exit'] == (1 if row['mode']=='reject' else 0) for row in summary)

assert all('native exit 0' in row['stderr'] and 'interp exit: 0' in row['stderr'] for row in records if row['mode']=='both')

for row in records:
    expected = Path(row['subject']).parent/'expected.txt'
    if row['mode'] == 'reject' and expected.exists():
        assert all(fragment.strip() in row['stderr'] for fragment in expected.read_text().splitlines() if fragment.strip()), row
