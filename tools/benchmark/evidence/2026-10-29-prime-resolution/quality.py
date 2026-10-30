import hashlib, importlib.util, json, pathlib, subprocess, sys, time
root = pathlib.Path(__file__).resolve().parents[2]
home = root/'build/prime-preparation-2'
spec = importlib.util.spec_from_file_location('benchmark',root/'tools/benchmark/benchmark.py')
benchmark = importlib.util.module_from_spec(spec);spec.loader.exec_module(benchmark)
cases = [
 ('pass','arithmetic/nested_clamp_resolution_exit'),
 ('pass','core/scoped_integer_call_guarantees_composed_exit'),
 ('pass','core/scoped_integer_call_exact_actual_requires_formation'),
 ('pass','proofs/constant_boolean_return_join_exit'),
 ('pass','proofs/indexed_boolean_normal_guarantee_exit'),
 ('fail','core/scoped_integer_call_wrapping_actual_requires_formation'),
 ('fail','core/scoped_integer_call_primitive_operator_requires_formation'),
 ('fail','core/scoped_integer_call_guarantee_requires_formation'),
 ('fail','core/scoped_integer_call_abstract_trait_operator_requires_formation'),
 ('fail','core/scoped_integer_call_inherited_trait_operator_requires_formation'),
 ('fail','proofs/conditional_boolean_return_is_not_first_arm'),
]
results=json.loads((home/'quality-results.json').read_text(encoding='utf-8')) if (home/'quality-results.json').exists() else []
for label in ['baseline','candidate']:
 binary=home/label/'omega.exe'
 benchmark.require_optimized(binary)
 for outcome, fixture in cases:
  source=root/'tests/omega'/outcome/fixture
  if any(row['producer']==label and row['fixture']==str(source.relative_to(root)) for row in results):continue
  project=home/'quality2'/label/source.name
  project.mkdir(parents=True,exist_ok=True)
  (project/'main.omg').write_bytes((source/'main.omg').read_bytes())
  (project/'build.omg').write_text('machine build(builder: &mut Build) {\n Build::application(builder, "quality");\n'+''.join(f' Build::bind(builder, {target}::ProgramEntry, main);\n' for target in ['windows_x86_64','linux_x86_64','macos_arm64','linux_arm64'])+'}\n',encoding='utf-8')
  observations=[]
  logs=home/'quality-observations'/label/source.name
  logs.mkdir(parents=True,exist_ok=True)
  def invoke(command,cwd=None):
   started=time.monotonic()
   completed=subprocess.run(list(map(str,command)),cwd=cwd or root,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
   text=completed.stdout.decode('utf-8','replace')
   transcript=logs/f'command-{len(observations)}.txt';transcript.write_text(text,encoding='utf-8')
   observations.append(dict(command=list(map(str,command)),exit_code=completed.returncode,seconds=time.monotonic()-started,transcript=str(transcript.relative_to(home))))
   return completed.returncode,text
  benchmark.run_plain=invoke
  prepared=True
  try:benchmark.settle_package_review(binary,project,'windows_x86_64')
  except SystemExit:prepared=False
  if outcome=='pass':
   assert prepared,observations
   code,text=invoke([binary,'run','--both','--keep',project/'main.omg'])
   assert code==0 and 'interp exit: 0' in text,text
  else:
   if prepared:code,text=invoke([binary,'--check','--target','windows_x86_64',project/'main.omg'])
   else:code,text=observations[-1]['exit_code'],(home/observations[-1]['transcript']).read_text(encoding='utf-8')
   expected=[line.strip() for line in (source/'expected.txt').read_text(encoding='utf-8').splitlines() if line.strip()]
   if fixture=='proofs/conditional_boolean_return_is_not_first_arm':
    expected=['violates required fact `value == true`','structurally false']
   assert code!=0 and all(fragment in text for fragment in expected),text
  results.append(dict(producer=label,fixture=str(source.relative_to(root)),source_sha256=hashlib.sha256((source/'main.omg').read_bytes()).hexdigest(),outcome=outcome,observations=observations))
  (home/'quality-results.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
  print(label,outcome,fixture,'OK',flush=True)
