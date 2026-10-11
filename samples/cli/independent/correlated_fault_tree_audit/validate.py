"""Sequential public CLI checks against pre-execution frozen byte oracles."""
from pathlib import Path
import argparse,hashlib,json,os,re,shutil,subprocess,time
parser=argparse.ArgumentParser()
parser.add_argument('--omega',required=True)
parser.add_argument('--route',choices=['check','native','terminal','invalid'],required=True)
arguments=parser.parse_args()
project=Path(__file__).resolve().parent
output=project/'build'/arguments.route
output.mkdir(parents=True,exist_ok=True)
environment=os.environ.copy()
repository=project.parents[3]
environment['OMEGA_LIBRARY_ROOT']=str(repository/'source/library')
records=[]
def run(name,command,stdin=b''):
    started=time.monotonic()
    try:
        completed=subprocess.run(command,input=stdin,capture_output=True,env=environment,timeout=300)
    except subprocess.TimeoutExpired as failure:
        completed=subprocess.CompletedProcess(command,124,failure.stdout or b'',(failure.stderr or b'')+b'\nVALIDATION TIMEOUT\n')
    (output/f'{name}.stdout').write_bytes(completed.stdout)
    (output/f'{name}.stderr').write_bytes(completed.stderr)
    record={'case':name,'command':command,'exit':completed.returncode,'seconds':time.monotonic()-started}
    records.append(record)
    return completed,record
if arguments.route=='check':
    result,record=run('application',[arguments.omega,'--check',str(project/'main.omg')])
    record['accepted']=result.returncode==0
elif arguments.route=='invalid':
    result,record=run('bounds',[arguments.omega,'--check',str(project/'controls/intended_invalid/main.omg')])
    # Package/custody rejection is explicitly not a bounds-rule witness.
    text=result.stderr.decode(errors='replace')+result.stdout.decode(errors='replace')
    expected=(project/'controls/intended_invalid/expected.txt').read_text().strip()
    record['accepted']=result.returncode!=0 and expected in text and not any(fragment in text for fragment in ['cache custody','pending review','cannot open private','package review'])
else:
    frozen=json.loads((project/'data/frozen_oracle.json').read_text())
    native_image=None
    for name,expected in frozen['cases'].items():
        stdin=(project/'data'/f'{name}.in').read_bytes()
        stdout=(project/'data'/f'{name}.out').read_bytes()
        assert hashlib.sha256(stdin).hexdigest()==expected['input_sha256']
        assert hashlib.sha256(stdout).hexdigest()==expected['output_sha256']
        if native_image is not None:
            command=[str(native_image)]
        else:
            command=[arguments.omega,'run']
            if arguments.route=='terminal': command+=['--both']
            if arguments.route=='native': command+=['--keep']
            command+=[str(project/'main.omg')]
        result,record=run(name,command,stdin)
        record['accepted']=result.returncode==expected['exit'] and result.stdout==stdout
        if arguments.route=='terminal':
            # The CLI exposes termination comparison, not interpreted stdout.
            # Require a real agreement, and retain the complete route diagnostics.
            record['accepted']=record['accepted'] and f"interp exit: {expected['exit']}".encode() in result.stderr and b'DIVERGENCE' not in result.stderr and b'DECLINED' not in result.stderr
        if arguments.route=='native' and native_image is None and record['accepted']:
            match=re.search(rb'build dir kept: (.+)',result.stderr)
            assert match, 'Public CLI did not retain native products'
            directory=Path(os.fsdecode(match.group(1)))
            candidates=[]
            for file in directory.rglob('*'):
                if file.is_file() and os.access(file,os.X_OK):
                    header=file.read_bytes()[:18]
                    if header[:4]==b'\x7fELF' and int.from_bytes(header[16:18],'little') in (2,3): candidates.append(file)
            assert len(candidates)==1, candidates
            native_image=output/'application'
            shutil.copy2(candidates[0],native_image)
            record['retained_build_dir']=str(directory)
            record['native_image_sha256']=hashlib.sha256(native_image.read_bytes()).hexdigest()
        print(name,record['exit'],record['accepted'],flush=True)
        if not record['accepted']: break
(output/'results.json').write_text(json.dumps(records,indent=2)+'\n')
print(json.dumps(records,indent=2))
raise SystemExit(0 if all(r['accepted'] for r in records) else 1)
