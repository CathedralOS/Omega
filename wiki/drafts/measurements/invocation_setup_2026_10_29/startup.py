"""Separate frozen-producer startup from previously measured compilation."""
import importlib.util,json,os,re,statistics,struct,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
DATA=ROOT/'build/invocation-setup/startup'; DATA.mkdir(parents=True)
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first)
first.EVIDENCE=DATA/'logs'; first.EVIDENCE.mkdir(); first.self_test()
producer=json.loads((ROOT/'build/native-queue-landing/candidate/producer.json').read_text())['producer']
assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
source=json.loads((ROOT/'build/native-queue-landing/candidate/build.json').read_text())['source_manifest']
assert all(first.digest(ROOT/p)==h for p,h in source.items())
rows=[]
for index in range(6):
    row,text=first.capture([producer['binary'],'--version'],f'version-{index}',dict(os.environ))
    assert row['exit']==0 and text.startswith('omega '),row
    rows.append(row)
binary=Path(producer['binary']).read_bytes(); pe=struct.unpack_from('<I',binary,0x3c)[0]
assert binary[pe:pe+4]==b'PE\0\0'
count=struct.unpack_from('<H',binary,pe+6)[0]; optional=struct.unpack_from('<H',binary,pe+20)[0]
sections=[]
for index in range(count):
    off=pe+24+optional+index*40
    sections.append({'name':binary[off:off+8].rstrip(b'\0').decode(),'virtual_bytes':struct.unpack_from('<I',binary,off+8)[0],'file_bytes':struct.unpack_from('<I',binary,off+16)[0]})
observations=json.loads((ROOT/'build/windows-ancestry-postqueue/capture/comparison/results.json').read_text())
gaps={}
for subject in first.SUBJECTS:
    values=[]
    for row in observations['single']:
        if row['subject']==subject and row['flavor']=='baseline':
            text=Path(row['log']).read_text(); inside=float(re.search(r'([\d.]+) ms  total elapsed',text)[1])
            values.append({'process_ms':row['wall_ms'],'cli_ms':inside,'outside_cli_ms':row['wall_ms']-inside})
    gaps[subject]=values
assert first.digest(producer['binary'])==producer['sha256']
assert all(first.digest(ROOT/p)==h for p,h in source.items())
result={'producer':producer,'controller_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source_manifest':source,'rows':rows,'sections':sections,'prior_compile_gaps':gaps,'protocol':'Six version-only processes, first visit retained separately; unchanged O3 producer and helper self-check. Diagnostic startup attribution, not a compiler speedup or an additive bound. Prior timings are original post-queue baseline observations.'}
(DATA/'results.json').write_text(json.dumps(result,indent=2)+'\n')
print('startup warm median ms',statistics.median(r['wall_ms'] for r in rows[1:]))
print('startup CPU medians', {k:statistics.median(r['root_cpu'][k] for r in rows[1:]) for k in rows[0]['root_cpu']})
print('PE sections',sections)
