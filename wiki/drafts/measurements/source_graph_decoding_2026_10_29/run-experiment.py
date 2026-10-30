import pathlib,subprocess,os,sys,json,hashlib,shutil
root=pathlib.Path.cwd();out=root/'build/performance/source-decoding';sys.path.insert(0,str(root/'tools'));import sample_native_gate as gate
bench=out/'omega-bench.exe';commands=[];os.environ['OMEGA_LIBRARY_ROOT']=str(root/'source/library')
def save(path,value):path.write_text(json.dumps(value,indent=2),encoding='utf-8')
def run(label,args,env=None,accepted=(0,)):
 args=[str(arg) for arg in args];print('RUN '+label,flush=True);p=subprocess.run(args,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE);(out/(label+'.stdout')).write_bytes(p.stdout);(out/(label+'.stderr')).write_bytes(p.stderr);commands.append({'label':label,'command':args,'exit_code':p.returncode});save(out/'experiment-commands.json',commands);print('EXIT '+str(p.returncode)+' '+p.stdout.decode(errors='replace')[:160],flush=True)
 if p.returncode not in accepted:raise RuntimeError(label+': '+p.stderr.decode(errors='replace')[-2500:])
 return p

def graphs(cache):return {str(path.relative_to(cache)):{'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()} for path in cache.rglob('source-graphs/*.rec')}
plans=[];identities={}
for name in ['length','square']:
 first=out/f'{name}-a';second=out/f'{name}-b'
 authored=lambda path:{str(file.relative_to(path)):hashlib.sha256(file.read_bytes()).hexdigest() for file in path.rglob('*') if file.is_file() and 'build' not in file.relative_to(path).parts and file.name!='omega.lock'}
 assert authored(first)==authored(second),(name,'authored copies differ');identities[name]={'authored':authored(first)}
 for role,binary in [('a','omega-before.exe'),('b','omega-after.exe')]:
  cache=out/f'cache-{name}-{role}';cache.mkdir(exist_ok=True);os.environ['LOCALAPPDATA']=str(cache)
  if role=='b':
   reason,detail=gate.accept_review(str(out/binary),str(out/f'{name}-{role}'),'windows_x86_64',120);save(out/f'prepare-{name}-{role}.json',{'reason':reason,'detail':detail});assert reason is None,detail
  env=os.environ.copy()
  for warmup in range(2):
   directory=out/f'prepare-native-{name}-{role}-{warmup}';directory.mkdir(exist_ok=False)
   run(f'prepare-native-{name}-{role}-{warmup}',[out/binary,'--build-dir',directory,'--target','windows_x86_64',out/f'{name}-{role}/main.omg'],env)
  identities[name][role]={'lock_sha256':hashlib.sha256((out/f'{name}-{role}/omega.lock').read_bytes()).hexdigest(),'graphs':graphs(cache)}
 p={'schema':'omega-performance-plan/1','name':name+' matched compiler before/after','working_directory':str(root),'inputs':[str(out/f'{name}-{role}'/filename) for role in ['a','b'] for filename in ['main.omg','build.omg','omega.lock']]+['source/library'],'baseline':{'program':str(out/'omega-before.exe'),'environment':{'LOCALAPPDATA':str(out/f'cache-{name}-a')}},'candidate':{'program':str(out/'omega-after.exe'),'environment':{'LOCALAPPDATA':str(out/f'cache-{name}-b')}},'warmups':2,'pairs':20,'timeout_ms':60000,'measurement':{'kind':'compiler','source':str(first/'main.omg'),'candidate_source':str(second/'main.omg'),'library_root':'source/library','target':'windows_x86_64','expected':{'exit_code':0,'stdout':'','stderr':''},'require_identical_artifacts':True}}
 path=out/(name+'-comparison.json');save(path,p);plans.append(str(path))
save(out/'cache-identities-before.json',identities);save(out/'comparison-suite.json',{'schema':'omega-performance-suite/1','plans':plans})
run('comparison-suite',[bench,'suite',out/'comparison-suite.json',out/'comparison-suite'])
for index in range(2):run('check-'+str(index),[bench,'check',out/f'comparison-suite/case-{index:02}','0'],accepted=(0,2,3))
for name,plan in zip(['length','square'],plans):
 for trial in range(3):
  for role in (['baseline','candidate'] if trial%2==0 else ['candidate','baseline']):run(f'phase-{name}-{trial}-{role}',[bench,'profile',plan,role,'phase',out/f'phase-{name}-{trial}-{role}'])
for name in ['length','square']:
 for role in ['a','b']:
  after=graphs(out/f'cache-{name}-{role}');assert after==identities[name][role]['graphs'],(name,role,'cache payload changed during measurement')
save(out/'cache-identities-after.json',{name:{role:graphs(out/f'cache-{name}-{role}') for role in ['a','b']} for name in ['length','square']})
for role,binary in [('a','omega-before.exe'),('b','omega-after.exe')]:
 env=os.environ.copy();env['LOCALAPPDATA']=str(out/f'cache-length-{role}')
 run('length-both-'+role,[out/binary,'run','--both','--target','windows_x86_64',out/f'length-{role}/main.omg'],env)
 invalid=out/f'length-{role}/invalid/main.omg'
 refusal=run('length-invalid-'+role,[out/binary,'--check','--target','windows_x86_64',invalid],env,accepted=(1,))
 assert b'compiler identity changed' not in refusal.stderr and b'cannot prove index' in refusal.stderr,refusal.stderr.decode(errors='replace')
# Corrupt only an owned candidate cache after all timing observations. The
# owner must refuse its digest, recompile, and preserve the authored result.
cache=out/'cache-square-b';record=next(cache.rglob('source-graphs/*.rec'));original=record.read_bytes();corrupt=bytearray(original);corrupt[-1]^=1
try:
 record.write_bytes(corrupt);env=os.environ.copy();env['LOCALAPPDATA']=str(cache);env['OMEGA_PHASE_PROFILE']='1';directory=out/'corrupt-cache-recovery';directory.mkdir()
 p=run('corrupt-cache-recovery',[out/'omega-after.exe','--build-dir',directory,'--target','windows_x86_64',out/'square-b/main.omg'],env)
 assert b'different payload digest' in p.stderr
 image=next(directory.glob('*.exe'));expected=next(json.loads(line)['sample']['artifact_sha256'] for line in (out/'comparison-suite/case-01/journal.jsonl').read_text().splitlines() if json.loads(line)['event']=='sample');assert hashlib.sha256(image.read_bytes()).hexdigest()==expected;run('corrupt-cache-native',[image],env)
finally:record.write_bytes(original)
print('ALL EXPERIMENTS COMPLETE',flush=True)
