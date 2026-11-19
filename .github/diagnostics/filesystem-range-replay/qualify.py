import hashlib,json,os,re,shutil,signal,subprocess,tempfile,time
from pathlib import Path
root=Path.cwd();out=root/'build/fs-cut-qualification';out.mkdir(parents=True,exist_ok=True)
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
digest=sha
def unit_fingerprints(directory, name):
    return [p for p in directory.iterdir() if p.name.endswith(f'-{name}.json')]

def fingerprints(file, name, sha, binary):
    directory = file.parent
    if directory.name == 'out':
        return unit_fingerprints(directory.parent / 'fingerprint', name)
    profile = directory.parent if directory.name == 'deps' else directory
    outputs = {}
    cargo = profile / '.fingerprint'
    if cargo.is_dir():
        for unit in cargo.iterdir():
            if '-' not in unit.name:
                continue
            key = unit.name.rsplit('-', 1)[1]
            for candidate in [f'lib{name}-{key}.rlib', f'{name}-{key}']:
                output = profile / 'deps' / candidate
                if output.is_file() and digest(output) == sha:
                    outputs[output] = unit_fingerprints(unit, name)
    if binary and directory.name != 'deps':
        builds = profile / 'build'
        if builds.is_dir():
            for package in builds.iterdir():
                if not package.is_dir():
                    continue
                for unit in package.iterdir():
                    output = unit / 'out' / file.name
                    if output.is_file() and digest(output) == sha:
                        outputs[output] = unit_fingerprints(unit / 'fingerprint', name)
    assert len(outputs) == 1, (file, 'does not identify one exact output', list(outputs))
    return next(iter(outputs.values()))

def artifact_receipt(artifact):
    executable = artifact.get('executable')
    file = Path(executable or next(p for p in artifact['filenames'] if p.endswith('.rlib')))
    assert str(file) in artifact['filenames']
    sha = digest(file)
    candidates = fingerprints(file, artifact['target']['name'].replace('-', '_'), sha, executable is not None)
    assert len(candidates) == 1, (file, candidates)
    fingerprint = candidates[0]
    contents = fingerprint.read_text()
    flags = json.loads(contents)['rustflags']
    allowed = {'1', '2', '3', 's', 'z'}
    assert artifact['profile']['opt_level'] in allowed
    assert all(isinstance(flag, str) and not flag.startswith('@') for flag in flags)
    for index, flag in enumerate(flags):
        if flag in ('-C', '--codegen'):
            option = flags[index+1] if index+1 < len(flags) else ''
        elif flag.startswith('-C'):
            option = flag[2:]
        elif flag.startswith('--codegen='):
            option = flag[len('--codegen='):]
        else:
            option = flag
        if option.startswith('opt-level='):
            assert option.split('=', 1)[1] in allowed
    return {'path': str(file), 'sha256': sha, 'cargo_artifact': artifact,
            'fingerprints': [{'path': str(fingerprint), 'sha256': digest(fingerprint),
                              'contents': contents, 'rustflags': flags}]}









rows=[json.loads(line) for line in (out/'cargo.jsonl').read_text().splitlines() if line.startswith('{')]
artifacts=[r for r in rows if r.get('reason')=='compiler-artifact' and r['target']['name'] in {'omega','source_compiler','compiler'}]
program,=[r for r in artifacts if r['target']['name']=='omega' and r.get('executable')]
receipts=[artifact_receipt(row) for row in artifacts];producer=Path(program['executable']);producer_digest=sha(producer)
inputs={str(p.relative_to(root)):sha(p) for p in (root/'source/library').rglob('*.omg')}
source=root/'tests/fixtures/pending/filesystem_portable_rules/main.omg'
inputs.update({str(p.relative_to(root)):sha(p) for p in [source,source.with_name('build.omg')]})
check=root/'omega-rust/psi/compilation/pipeline/01_source-compilation/src/operations/checking/src'
proof={str(p.relative_to(root)):sha(p) for p in [check/'known.rs',check/'prove.rs',check/'prove/tests.rs']}
record={'revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'production_revision':'a1a53d46674c8058496977c6b7b67e256a4d514f','status':subprocess.check_output(['git','status','--porcelain'],text=True),'binary_sha256':producer_digest,'executables':receipts,'inputs':inputs,'proof_source':proof,'host':'linux_x86_64'}
(out/'producer.json').write_text(json.dumps(record,indent=2)+'\n');(out/'source-digests.json').write_text(json.dumps(dict(inputs,**proof),indent=2)+'\n')
retained=out/'retained-omega'
if not retained.exists():os.link(producer,retained)
assert sha(retained)==producer_digest
if '--record-only' in os.sys.argv:raise SystemExit(0)
def run(label,command):
 folder=out/label;folder.mkdir();peak=0;stop=None
 with tempfile.TemporaryDirectory(prefix='omega-fs-qualified-') as directory:
  wide=Path(directory)/'omega_fs_rules_wide';wide.mkdir()
  for n in range(4200):(wide/str(n)).touch()
  count=len(list(wide.iterdir()));assert count==4200;started=time.monotonic()
  with (folder/'stdout').open('wb') as stdout,(folder/'stderr').open('wb') as stderr:
   child=subprocess.Popen(command,cwd=directory,env=os.environ.copy(),stdin=subprocess.DEVNULL,stdout=stdout,stderr=stderr,start_new_session=True)
   try:
    while child.poll() is None:
     rss=subprocess.run(['ps','-p',str(child.pid),'-o','rss='],capture_output=True,text=True).stdout.strip();peak=max(peak,int(rss or '0'))
     if peak>=4*1024*1024:stop='4GiB RSS cap';break
     if time.monotonic()-started>=600:stop='600s program cap';break
     time.sleep(.2)
   finally:
    if child.poll() is None:
     os.killpg(child.pid,signal.SIGTERM)
     try:child.wait(timeout=5)
     except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
  result={'command':command,'returncode':child.returncode,'elapsed_seconds':time.monotonic()-started,'stop_reason':stop,'initial_wide_file_count':count,'wide_exists_after':wide.exists(),'peak_rss_kib':peak}
 text=(folder/'stderr').read_text(errors='replace');result['native_exits']=re.findall(r'^native exit\s+(-?\d+)\s*$',text,re.M);result['psi_exits']=re.findall(r'^(?:interp exit:|Psi exit)\s*(-?\d+)\s*$',text,re.M)
 kept=re.findall(r'^build dir kept: (.+)$',text,re.M)
 if kept:
  kept=Path(kept[-1]);result['kept_build_dir']=str(kept)
  images=[]
  for p in kept.rglob('*'):
   if p.is_file() and p.stat().st_mode&0o111:
    with p.open('rb') as f:header=f.read(4)
    if header==b'\x7fELF':images.append(p)
  result['native_images']=[{'path':str(p),'sha256':sha(p)} for p in images]
  saved=out/'compiled-program';saved.mkdir(exist_ok=True)
  for name in ['program.oterm','native.identity.txt','terminal.stdout','terminal.stderr']:
   p=kept/name
   if p.is_file():shutil.copy2(p,saved/name)
 result_path=folder/'result.json';result_path.write_text(json.dumps(result,indent=2)+'\n');print(label,json.dumps(result),flush=True);return result
both=run('both',[str(producer),'run','--both','--keep','--verbose',str(source)])
acceptance={'combined_both_passed':both['returncode']==42 and both['native_exits']==['42'] and both['psi_exits']==['42'],'separate_identical_seeds_passed':False}
if both['stop_reason'] is None and both['native_exits']==['13'] and both['psi_exits']==['42'] and not both['wide_exists_after']:
 images=both.get('native_images',[])
 if len(images)==1:
  image=Path(images[0]['path']);saved=out/'compiled-program'/'program';shutil.copy2(image,saved);assert sha(saved)==images[0]['sha256']
  native=run('native-fresh-seed',[str(saved)])
  acceptance['separate_identical_seeds_passed']=native['returncode']==42 and native['stop_reason'] is None
  acceptance['shared_seed_observation']='The interpreter removed the external wide directory before native execution. The same retained native image was then executed against a separate identical 4200-file seed.'
 else:
  acceptance['shared_seed_observation']='Interpreter42/native13 observed, but the kept artifact did not identify one ELF image. No fresh native acceptance claimed.'
assert sha(producer)==producer_digest
assert all(sha(root/path)==expected for path,expected in dict(inputs,**proof).items())
(out/'acceptance.json').write_text(json.dumps(acceptance,indent=2)+'\n');(out/'source-after.txt').write_text(subprocess.check_output(['git','status','--porcelain'],text=True))
