"""Serial acceptance runner. Uses the frozen oracle, never regenerates it."""
import argparse, hashlib, json, subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--omega',required=True);p.add_argument('--native',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
out=root/'build/validation';out.mkdir(parents=True,exist_ok=True)
oracle=json.loads((root/'data/expected.json').read_text())
results=[]
for name,case in oracle['cases'].items():
    data=(root/'data'/f'{name}.stdin').read_bytes()
    assert hashlib.sha256(data).hexdigest()==case['input_sha256']
    run=subprocess.run([a.native],input=data,capture_output=True,timeout=30)
    (out/f'{name}.stdout').write_bytes(run.stdout);(out/f'{name}.stderr').write_bytes(run.stderr)
    good=run.returncode==case['exit'] and run.stdout==case['stdout'].encode() and not run.stderr
    results.append({'case':name,'route':'native','exit':run.returncode,'pass':good})
    print(name,run.returncode,'PASS' if good else 'FAIL',flush=True)
control=subprocess.run([a.omega,'--check',str(root/'controls/out_of_bounds/main.omg')],capture_output=True,timeout=180)
(out/'control.stdout').write_bytes(control.stdout);(out/'control.stderr').write_bytes(control.stderr)
message=control.stdout+control.stderr
fragment=(root/'controls/out_of_bounds/expected.txt').read_text().strip().encode()
control_good=control.returncode==1 and fragment in message and b'package acceptance' not in message and b'parse error' not in message
results.append({'case':'intended invalid bounds','route':'check','exit':control.returncode,'pass':control_good})
print('intended invalid bounds',control.returncode,'PASS' if control_good else 'FAIL',flush=True)
# --both compares status only, so native stdout is still checked separately above.
for name in ['additive','interaction','zero','pure_interaction','missing_cell','missing_repeat','insufficient','bad_token']:
    case=oracle['cases'][name]
    run=subprocess.run([a.omega,'run','--both',str(root/'main.omg')],input=(root/'data'/f'{name}.stdin').read_bytes(),capture_output=True,timeout=180)
    (out/f'{name}.both.stdout').write_bytes(run.stdout);(out/f'{name}.both.stderr').write_bytes(run.stderr)
    declined=b'DECLINED' in run.stderr
    good=run.returncode==case['exit'] and run.stdout==case['stdout'].encode() and not declined and f'interp exit: {case["exit"]}'.encode() in run.stderr
    results.append({'case':name,'route':'Terminal comparison','exit':run.returncode,'pass':good,'declined':declined})
    print(name,'Terminal',run.returncode,run.stderr.decode(errors='replace')[-500:],flush=True)
(out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
raise SystemExit(0 if all(row['pass'] for row in results) else 1)
