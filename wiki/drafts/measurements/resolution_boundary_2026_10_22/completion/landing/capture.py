import hashlib,json,os,subprocess,sys
from pathlib import Path
sys.path.insert(0,str(Path('tools').resolve()))
from compiler_profile import require_optimized
folder=Path('build/deferred-completion-storage-landing');mode=sys.argv[1]
binary=(folder/('omega-candidate' if mode=='production' else 'omega-instrumented')).resolve();require_optimized(binary)
subjects=[
 ('length','samples/cli/facts/length/local/main.omg',True,0),
 ('dyn','samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg',True,0),
 ('record','samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg',True,0),
 ('returned','tests/omega/pass/layouts/placed_reference_results/returned/main.omg',True,0),
 ('forwarded','tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg',True,0),
 ('nested','tests/omega/pass/layouts/placed_reference_results/nested/main.omg',True,0),
 ('equation','tests/omega/pass/generics/runtime_machine_equation_nested_const_exit/main.omg',True,0),
 ('machine_constants','tests/omega/pass/modules/machine_constant_initializers/main.omg',False,0),
 ('computed_constants','tests/omega/pass/modules/computed_constant_initializers/main.omg',False,1),
 ('wrong_backing','tests/omega/fail/layouts/placed_reference_results/wrong_backing/main.omg',False,1),
 ('overlapping_loans','tests/omega/fail/layouts/placed_reference_results/overlapping_loans/main.omg',False,1),
 ('exact_policy_owner','tests/omega/fail/layouts/placed_policy_requires_exact_attached_owner/main.omg',False,1),
 ('result_authority','tests/omega/fail/layouts/placed_factory_result_cannot_upgrade_source/main.omg',False,1),
 ('invalid_length','samples/cli/facts/length/local/invalid/main.omg',False,1),
]
record=dict(mode=mode,binary=str(binary),sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),observations=[])
inputs=sorted(set(p for _,subject,_,_ in subjects for p in Path(subject).parent.rglob('*.omg')))
record['source_hashes']={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
record['library_tree']=subprocess.check_output(['git','rev-parse','HEAD:source/library'],text=True).strip()
env=os.environ.copy();env['OMEGA_COMPLETION_EXPERIMENT']=mode
for name,subject,native,expected in subjects:
 command=[str(binary),'run','--both',str(Path(subject).resolve())] if native else [str(binary),'--check',str(Path(subject).resolve())]
 result=subprocess.run(command,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=120)
 row=dict(name=name,subject=subject,native=native,expected_exit=expected,command=command,exit=result.returncode,output=result.stdout,
 traces=[line for line in result.stdout.splitlines() if line.startswith('COMPLETION\t')])
 record['observations'].append(row)
 (folder/(mode+'-observations.json')).write_text(json.dumps(record,indent=2)+'\n')
 print(name,mode,'exit',result.returncode,'comparisons',len(row['traces']),flush=True)
 assert result.returncode==expected,row
 if native:assert 'native exit 0' in result.stdout and 'interp exit: 0' in result.stdout,row
 if mode!='baseline':
  baseline=json.loads((folder/'baseline-observations.json').read_text())['observations'][len(record['observations'])-1]
  def clean(output):return '\n'.join(line for line in output.splitlines() if not line.startswith('COMPLETION\t'))
  assert clean(result.stdout)==clean(baseline['output']),(name,baseline['output'],result.stdout)
assert record['source_hashes']=={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
assert record['sha256']==hashlib.sha256(binary.read_bytes()).hexdigest()
print(mode,'accepted',flush=True)
