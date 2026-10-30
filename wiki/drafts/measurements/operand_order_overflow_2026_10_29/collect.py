"""Verify exact-source outcomes and retain the bounded overflow experiment."""
import hashlib,json,re,shutil,statistics,subprocess
from pathlib import Path

root=Path.cwd()
base=root/'build/operand-order-overflow'
record=root/'wiki/drafts/measurements/operand_order_overflow_2026_10_29'
def read(path):return json.loads(path.read_text())
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def copy(source,destination):
    destination.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(source,destination)
assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()=='643ffc583f8ef53be014b04b15b77f3ec8447431'
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==(base/'candidate.patch').read_bytes()
inputs=read(base/'inputs.json')
for name,digest in inputs['std_inputs'].items():assert sha(root/name)==digest
baseline=read(base/'cases/baseline-update/result.json')
assert baseline['exit']==3221225725 and baseline['stack_overflow']
assert sha(Path(baseline['command'][0]))==baseline['producer_sha256']
for case in ['baseline-check','baseline-update']:
    for p in (base/case).glob('*'):
        if p.is_file():copy(p,record/'harness-layout-errors'/case/p.name)
for case in ['baseline-update','minimal-debug','original-debug','inline-single-evaluation','pinned-baseline']:
    directory=base/'cases'/case
    for name in ['result.json','output.txt','debugger.txt','main.omg']:
        if (directory/name).is_file():copy(directory/name,record/'reproduction'/case/name)
    for p in (directory/'input').glob('*.omg'):copy(p,record/'reproduction'/case/'input'/p.name)
minimal=read(base/'cases/inline-single-evaluation/result.json')
pinned=read(base/'cases/pinned-baseline/result.json')
assert minimal['exit']==pinned['exit']==3221225725 and minimal['stack_overflow'] and pinned['stack_overflow']
assert sha(root/'tests/omega/pass/calls/selected_contract_equation_reentry_exit/main.omg')==pinned['source_sha256']
quality=read(base/'quality.json')
native=[r for r in quality if '--timings' in r['command']]
both=[r for r in quality if '--both' in r['command']]
rejects=[r for r in quality if r.get('matched')]
assert len(native)==len(both)==5 and len(rejects)==6
assert all(r['exit']==r['native_exit']==0 for r in native)
assert all(r['exit']==0 and 'DECLINED' not in Path(r['transcript']).read_text() for r in both)
assert all(r['exit']!=0 and all(e in Path(r['transcript']).read_text() for e in r['expected']) for r in rejects)
agreements=[]
control_inputs={}
for row in quality:
    assert sha(Path(row['command'][0]))==row['producer_sha256']
    fixture=row['fixture']
    source=root/'tests/omega'/fixture
    assert sha(source/'main.omg')==row['source_sha256']
    transcript=Path(row['transcript'])
    copy(transcript,record/'quality'/source.name/transcript.name)
    control_inputs[fixture]={p.name:sha(p) for p in source.iterdir() if p.suffix in ['.omg','.txt']}
    for name,digest in control_inputs[fixture].items():
        copy(source/name,record/'inputs'/fixture/name)
        if name.endswith('.omg'):assert sha(transcript.parent/'input'/name)==digest
    if '--both' in row['command']:
        match=re.search(r'build dir kept: (.+)',transcript.read_text())
        assert match
        directory=Path(match.group(1).strip())
        native_row=next(r for r in native if r['fixture']==fixture)
        assert sha(directory/'omega-program.exe')==native_row['artifact_sha256']
        agreements.append(dict(fixture=fixture,native_sha256=sha(directory/'omega-program.exe'),terminal_sha256=sha(directory/'program.oterm'),directory=str(directory),exit=row['exit']))
rows=read(base/'comparison/runs.json')
metadata=read(base/'comparison/metadata.json')
assert len(rows)==36 and all(r['exit']==r['native_exit']==0 for r in rows)
assert (base/'comparison/compiler.patch').read_bytes()==(base/'candidate.patch').read_bytes()
summary={'baseline_original_seconds':baseline['seconds'],'baseline_minimized_seconds':minimal['seconds'],'baseline_pinned_seconds':pinned['seconds'],'native_compiles':41,'terminal_native_agreements':5,'static_rejects':6,'comparison':{}}
for label,producer in metadata['producers'].items():assert sha(Path(producer['path']))==producer['sha256']
for subject,source in metadata['sources'].items():
    assert len({r['artifact_sha256'] for r in rows if r['subject']==subject})==1
    for name,key in [('main.omg','sha256'),('build.omg','build_sha256')]:
        path=Path(source['path']).with_name(name)
        assert sha(path)==source[key]
        copy(path,record/'inputs/comparison'/subject/name)
    result={}
    for variant in 'AB':
        values=[r['wall_ms'] for r in rows if r['subject']==subject and r['variant']==variant and r['kind']=='measured']
        assert len(values)==4
        result[variant]={'wall_ms':values,'median_ms':statistics.median(values),'range_ms':[min(values),max(values)]}
    result['median_change_percent']=100*(result['B']['median_ms']/result['A']['median_ms']-1)
    result['ranges_overlap']=max(result['A']['range_ms'][0],result['B']['range_ms'][0])<=min(result['A']['range_ms'][1],result['B']['range_ms'][1])
    summary['comparison'][subject]=result
for row in rows:
    label=f"{row['subject']}-{row['index']:02}-{row['variant']}-{row['kind']}"
    copy(base/'comparison'/label/'output.txt',record/'comparison'/(label+'.txt'))
for name in ['runs.json','metadata.json']:copy(base/'comparison'/name,record/'comparison'/name)
for name in ['inputs.json','quality.json','candidate.patch','candidate-build.log','reproduce.py','debug.py','minimize.py','candidate-chain.py','quality.py','pin-baseline.py']:
    copy(base/name,record/name)
for name,value in [('control-inputs.json',control_inputs),('agreements.json',agreements),('summary.json',summary)]:
    (record/name).write_text(json.dumps(value,indent=2))
print(json.dumps(summary,indent=2))
