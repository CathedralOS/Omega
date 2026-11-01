from pathlib import Path
import json,subprocess,time,sys,os,hashlib,datetime
p=Path(__file__).parent;w=Path.cwd();sys.path.insert(0,str(w/'tools'));import compiler_profile
label=sys.argv[1];binary=p/(os.environ.get('VALIDATION_PRODUCER','baseline-b7-r1')+'-omega');profile=compiler_profile.require_optimized(binary);root=w/os.environ.get('VALIDATION_PROJECT','samples/gui/window_demo');inputs={str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in root.glob('*.omg')};lock=root/'omega.lock'
if lock.exists():(p/(label+'-input.omega.lock')).write_bytes(lock.read_bytes())
cmd=[str(binary),*sys.argv[2:]];t=time.monotonic();r=subprocess.run(cmd,capture_output=True,stdin=subprocess.DEVNULL,env=dict(os.environ,OMEGA_LIBRARY_ROOT=str(w/'source/library'),RUST_MIN_STACK='67108864'))
for c in ['stdout','stderr']:(p/(label+'.'+c)).write_bytes(getattr(r,c))
assert inputs=={str(f):hashlib.sha256(f.read_bytes()).hexdigest() for f in root.glob('*.omg')};assert compiler_profile.require_optimized(binary)==profile
if lock.exists():(p/(label+'-output.omega.lock')).write_bytes(lock.read_bytes())
record={'command':cmd,'producer':profile,'inputs':inputs,'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit':r.returncode,'seconds':time.monotonic()-t,'stdout':str(p/(label+'.stdout')),'stderr':str(p/(label+'.stderr'))};(p/(label+'.json')).write_text(json.dumps(record,indent=2)+'\n');print(json.dumps(record),flush=True);print((r.stdout+r.stderr).decode(errors='replace')[-5000:],flush=True)
