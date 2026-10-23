"""Diagnostic full-versus-projected selection, retaining actual producer output."""
import hashlib, json, os, subprocess, sys, time
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
from compiler_profile import require_optimized
folder=Path('build/placed-result-selection')
binary=(folder/'omega-instrumented').resolve()
require_optimized(binary)
subjects=[
 ('target_return','build/placed-result-selection/witnesses/target_return/main.omg'),
 ('nested','tests/omega/pass/layouts/placed_reference_results/nested/main.omg'),
 ('returned','tests/omega/pass/layouts/placed_reference_results/returned/main.omg'),
 ('forwarded','tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg'),
 ('length','samples/cli/facts/length/local/main.omg'),
 ('dyn','samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg'),
 ('record','samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg'),
 ('wrong_backing','tests/omega/fail/layouts/placed_reference_results/wrong_backing/main.omg'),
 ('overlapping_loans','tests/omega/fail/layouts/placed_reference_results/overlapping_loans/main.omg'),
 ('exact_policy_owner','tests/omega/fail/layouts/placed_policy_requires_exact_attached_owner/main.omg'),
 ('result_authority','tests/omega/fail/layouts/placed_factory_result_cannot_upgrade_source/main.omg'),
 ('invalid_length','samples/cli/facts/length/local/invalid/main.omg'),
 ('policy_owner','tests/omega/pass/layouts/placed_policy_owner_selection/main.omg'),
 ('call_custody','tests/omega/pass/layouts/placed_view_call_custody/main.omg'),
]
record=dict(base=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
 binary=str(binary), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
 patch_sha256=hashlib.sha256((folder/'instrumentation.patch').read_bytes()).hexdigest(), observations=[])
for name,subject in subjects:
 if not Path(subject).exists():
  record['observations'].append(dict(name=name,subject=subject,missing=True));continue
 for mode in ('full','compare'):
  environment=os.environ.copy(); environment.pop('OMEGA_CALL_RESULT_EXPERIMENT',None)
  if mode=='compare':environment['OMEGA_CALL_RESULT_EXPERIMENT']='1'
  command=[str(binary),'--check',str(Path(subject).resolve())]
  start=time.monotonic()
  result=subprocess.run(command,env=environment,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  row=dict(name=name,subject=subject,mode=mode,command=command,exit=result.returncode,
   output=result.stdout,wall_seconds=time.monotonic()-start,
   traces=[line for line in result.stdout.splitlines() if line.startswith('CALLRESULT\t')])
  record['observations'].append(row)
  (folder/'observations.json').write_text(json.dumps(record,indent=2)+'\n')
  print(name,mode,'exit',result.returncode,'comparisons',len(row['traces']),
    'equal',sum('\tequal=true\t' in line for line in row['traces']),
    'different',sum('\tequal=false\t' in line for line in row['traces']),
    'errors',sum('candidate_error=' in line for line in row['traces']),flush=True)
(folder/'observations.json').write_text(json.dumps(record,indent=2)+'\n')
