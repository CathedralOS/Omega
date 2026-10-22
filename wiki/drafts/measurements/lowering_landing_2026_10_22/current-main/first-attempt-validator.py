"""Replay existing fact rows on both frozen producers; compare exact artifacts."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path.cwd()
OUTPUT = ROOT/'build/lowering-landing/current-main/validation.json'
SCRATCH = ROOT/'build/lowering-landing/current-main/artifacts'
SCRATCH.mkdir(parents=True, exist_ok=True)
BINARIES = {name: ROOT/f'build/lowering-landing/omega-{"final" if name == "baseline" else "current-main"}' for name in ('baseline','candidate')}
PLACES = ('local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body')
SUBJECTS = [
 'samples/cli/collections/segmented_packet_checksum/main.omg',
 'samples/cli/facts/length/local/main.omg',
 'samples/cli/facts/open_const_implementation/local/main.omg',
 'samples/cli/facts/as_slice_views/call_result/main.omg',
 'samples/cli/facts/as_slice_views/other_state/main.omg',
 'samples/cli/facts/as_slice_views/target_body/main.omg',
 'tests/omega/pass/borrows/runtime_copied_range_record_domains_exit/main.omg',
 'tests/omega/pass/contracts/returned_reference_field_mutation_exit/main.omg',
]
CONTROL_SUBJECTS = [f'tests/omega/fail/{name}/main.omg' for name in (
 'traits/implementation_argument_wrong_contract',
 'generics/applied_implementation_unused_method_carrier',
 'borrows/returned_reference_invalidated_membership',
 'slices/scalar_copy_invalidated_domain',
 'slices/scalar_copy_missing_domain',
 'borrows/view_local_store_while_lent',
)]

def entry_machine(subject):
    build = Path(subject).parent/'build.omg'
    return 'Main::main' if build.exists() and 'Main::main' in build.read_text() else 'main'

def expected_exit(subject):
    expected = Path(subject).parent/'expected_native_exit.txt'
    return int(expected.read_text()) if expected.exists() else 0

BASELINE_REFUSAL = 'tests/omega/pass/borrows/runtime_nested_array_range_record_domains_exit/main.omg'
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
            row = execute(name, subject, 'terminal', ['inspect-terminal','--machine', entry_machine(subject),'--output',str(artifact)])
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
        summary.append(dict(subject=subject,mode=mode,baseline_exit=old['exit'],candidate_exit=new['exit'],equal=equal, classification='baseline_refusal' if subject == BASELINE_REFUSAL else 'acceptance'))
OUTPUT.write_text(json.dumps(dict(source_hashes={str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for subject in SUBJECTS + CONTROL_SUBJECTS for path in (ROOT/subject).parent.rglob('*.omg')},binary_hashes={name:hashlib.sha256(path.read_bytes()).hexdigest() for name,path in BINARIES.items()},records=records,summary=summary),indent=2)+'\n')
print(json.dumps(summary,indent=2),flush=True)
assert all(row['equal'] for row in summary)
assert all(row['candidate_exit'] == (1 if row['mode']=='reject' else 0) for row in summary if row['mode'] != 'both' and row['classification'] == 'acceptance')

assert all(f"native exit {expected_exit(row['subject'])}" in row['stderr'] and f"interp exit: {expected_exit(row['subject'])}" in row['stderr'] for row in records if row['mode']=='both' and row['subject'] != BASELINE_REFUSAL)
for row in records:
    expected = Path(row['subject']).parent/'expected.txt'
    if row['mode'] == 'reject' and expected.exists():
        fragments = [line.strip() for line in expected.read_text().splitlines() if line.strip()]
        assert all(fragment in row['stderr'] for fragment in fragments), row


for row in records:
    if row['subject'] == BASELINE_REFUSAL:
        assert row['exit'] == (200 if row['mode'] == 'both' else 1)
        assert row['stderr'].count('cannot prove default-domain field requirement') == 4
        assert 'native exit' not in row['stderr'] and 'interp exit:' not in row['stderr']

# Newly integrated case-tag behavior is validated on the current producer;
# the historical producer predates that repair and is not its oracle.
case_records=[]
for place in ('call_result','target_body'):
    for invalid in (False,True):
        subject=f'samples/cli/facts/case_payload_exclusive_loan/{place}/'+('invalid/' if invalid else '')+'main.omg'
        command=[str(BINARIES['candidate']),*(['--check'] if invalid else ['run','--both']),subject]
        result=subprocess.run(command,capture_output=True,text=True,timeout=90)
        row=dict(command=command,subject=subject,exit=result.returncode,stdout=result.stdout,stderr=result.stderr)
        case_records.append(row)
        (ROOT/'build/lowering-landing/current-main/case-validation.json').write_text(json.dumps(case_records,indent=2)+'\n')
        assert result.returncode==(1 if invalid else 0),row
        assert ('while local borrow `selected` is still active' in result.stderr) if invalid else ('native exit 0' in result.stderr and 'interp exit: 0' in result.stderr),row
