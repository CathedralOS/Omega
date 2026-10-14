"""Frozen source, record and genuine-component oracles. Never accepts project policy."""
import argparse, hashlib, json, pathlib, struct, subprocess
p=argparse.ArgumentParser();p.add_argument('--compiler',required=True);p.add_argument('--observer',required=True);p.add_argument('--output',required=True);a=p.parse_args()
repo=pathlib.Path(__file__).resolve().parents[5];out=pathlib.Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
observations=[]
def run(command, expected=0, fragments=()):
 r=subprocess.run(command,cwd=repo,stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=600)
 row=dict(command=list(map(str,command)),exit=r.returncode,stdout=r.stdout,stderr=r.stderr);observations.append(row)
 (out/'observations.json').write_text(json.dumps(observations,indent=2))
 assert r.returncode==expected,row
 for text in fragments: assert text in r.stderr,row
 return r
for path in sorted((repo/'tests/omega').glob('*/build/component_assumption_*/main.omg')):
 if path.parent.name=='component_assumption_mechanism': continue
 fail='fail' in path.parts
 expected=(path.parent/'expected.txt').read_text().splitlines() if fail else []
 run([a.compiler,'--check',str(path)],1 if fail else 0,expected)
root=repo/'samples/cli/independent/component_assumption_recording/main.omg'
run([a.observer,'records',str(root),str(out/'records')])
assert (out/'records/recorded-digest.bin').read_bytes()==bytes.fromhex('0123456789abcdef'*4)
for name in ('reborrow','user_declaration'):
 run([a.observer,'records',str(repo/f'tests/omega/pass/build/component_assumption_{name}/main.omg'),str(out/name)])
run([a.observer,'component',str(repo/'tests/omega/pass/build/component_assumption_mechanism/main.omg'),str(out/'component')])
assert (out/'component/mechanism.txt').read_text()=='1 1016 65'
expected=hashlib.sha256(b'omega-component-port-mechanism-v1'+struct.pack('<QHB',1,0x3f8,0x41)).digest()
assert expected.hex()=='911df406cf3928bc12f1df49472d755b4e66b224bb36bf88fd18aa93990b718e'
assert (out/'component/component-digest.bin').read_bytes()==expected
print('PASS: source controls, exact Build record, independently derived component digest, unchanged artifact and missing/changed policy controls')
