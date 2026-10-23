import hashlib,json,subprocess,sys,os
from pathlib import Path
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile,parse_timings,require_optimized,TIMING_LINE
output=Path('build/resolution-integration-final')
binary=(output/'omega-candidate').resolve()
require_optimized(binary)
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def clean(text):return '\n'.join(line for line in text.splitlines() if not TIMING_LINE.match(line) and not line.startswith('REBASE\t'))
subjects=[
 ('length','samples/cli/facts/length/local/main.omg',True),
 ('dyn','samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg',True),
 ('record','samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg',True),
 ('returned','tests/omega/pass/layouts/placed_reference_results/returned/main.omg',True),
 ('forwarded','tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg',True),
 ('wrong_backing','tests/omega/fail/layouts/placed_reference_results/wrong_backing/main.omg',False),
 ('overlapping_loans','tests/omega/fail/layouts/placed_reference_results/overlapping_loans/main.omg',False),
 ('exact_policy_owner','tests/omega/fail/layouts/placed_policy_requires_exact_attached_owner/main.omg',False),
 ('result_authority','tests/omega/fail/layouts/placed_factory_result_cannot_upgrade_source/main.omg',False),
 ('length_invalid','samples/cli/facts/length/local/invalid/main.omg',False),
]

setup=json.loads((output/'setup.json').read_text())
previous=json.loads(Path('wiki/drafts/measurements/resolution_boundary_2026_10_22/rebase/candidate-observations.json').read_text())['records']
observations=[]
for row in setup:
    prior=next(r for r in previous if r['subject']==row['name'] and r['producer']=='candidate' and not r.get('both'))
    if row['positive']:
        assert digest(row['native'])==row['native_sha256']==prior['native_sha256'],row
        command=[row['native']]
        native=subprocess.run(command,capture_output=True,text=True,stdin=subprocess.DEVNULL,timeout=30)
        observations.append(dict(name=row['name'],command=command,native_exit=native.returncode,stdout=native.stdout,stderr=native.stderr))
        assert native.returncode==0,observations[-1]
        command=[str(binary),'run','--both',row['subject']]
        both=subprocess.run(command,capture_output=True,text=True,stdin=subprocess.DEVNULL,timeout=120)
        observations.append(dict(name=row['name'],command=command,both_exit=both.returncode,stdout=both.stdout,stderr=both.stderr))
        assert both.returncode==0 and 'native exit 0' in both.stderr and 'interp exit: 0' in both.stderr,observations[-1]
    else:
        assert clean(row['output'])==clean(prior['output']),row
        invalid=subprocess.run(row['command'],capture_output=True,text=True,stdin=subprocess.DEVNULL,timeout=120)
        observations.append(dict(name=row['name'],command=row['command'],reject_exit=invalid.returncode,stdout=invalid.stdout,stderr=invalid.stderr))
        assert invalid.returncode!=0 and clean(invalid.stdout+invalid.stderr)==clean(prior['output']),observations[-1]
    (output/'validation.json').write_text(json.dumps(observations,indent=2)+'\n')
    print(row['name'],'accepted oracle',flush=True)
print('5 native executions, 5 native/Terminal agreements, 5 unchanged rejection diagnostics; 5 native images identical to previous candidate')
