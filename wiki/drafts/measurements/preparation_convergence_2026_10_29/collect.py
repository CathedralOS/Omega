import hashlib,json,re,shutil,subprocess
from pathlib import Path
root=Path.cwd(); base=root/'build/preparation-convergence'
record=root/'wiki/drafts/measurements/preparation_convergence_2026_10_29'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def copy(a,b):
    b.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(a,b)
assert subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()=='cb087c46476f255397ba2e8cc3cbba4dd978c2ea'
assert subprocess.check_output(['git','diff','HEAD','--','omega-rust'])==(base/'candidate.patch').read_bytes()
assert sha(root/'build/contract-query-publication/producer/omega.exe')=='5f0e2fc5dde252263b64b47a6dd593fb3a41c19585f78f45ccd15998b897501e'
inputs={}; summary={}
for kind in ['quality','providers']:
    rows=json.loads((base/(kind+'.json')).read_text())
    native=[r for r in rows if 'artifact_sha256' in r]
    both=[r for r in rows if '--both' in r['command']]
    rejects=[r for r in rows if r.get('matched')]
    expected=(5,5,6) if kind=='quality' else (4,4,6)
    assert (len(native),len(both),len(rejects))==expected
    summary[kind]=dict(native_compiles=len(native),terminal_agreements=len(both),static_rejects=len(rejects))
    for row in rows:
        transcript=Path(row['transcript']); text=transcript.read_text()
        source=root/'tests/omega'/row['fixture']
        assert sha(source/'main.omg')==row['source_sha256']
        assert sha(Path(row['command'][0]))==row['producer_sha256']
        for p in source.glob('*.omg'):
            assert sha(p)==sha(transcript.parent/'input'/p.name)
            inputs[str(p.relative_to(root))]=sha(p)
            copy(p,record/'inputs'/row['fixture']/p.name)
        for p in source.glob('expected*.txt'):
            inputs[str(p.relative_to(root))]=sha(p)
            copy(p,record/'inputs'/row['fixture']/p.name)
        if 'artifact_sha256' in row:assert row['exit']==row['native_exit']==0
        if row.get('matched'):assert row['exit']!=0 and all(x in text for x in row['expected'])
        if '--both' in row['command']:
            assert row['exit']==0 and 'DECLINED' not in text
            match=re.search(r'build dir kept: (.+)',text); assert match
            artifacts=Path(match.group(1).strip())
            prior=next(r for r in native if r['fixture']==row['fixture'] and r.get('variant')==row.get('variant'))
            assert sha(artifacts/'omega-program.exe')==prior['artifact_sha256']
            row['native_sha256']=sha(artifacts/'omega-program.exe'); row['terminal_sha256']=sha(artifacts/'program.oterm')
        relative=Path(kind)/transcript.parent.name/transcript.name
        copy(transcript,record/relative); row['transcript']=str(relative)
    if kind=='providers':
        for fixture in {r['fixture'] for r in native}:
            assert len({r['artifact_sha256'] for r in native if r['fixture']==fixture})==1
    (record/(kind+'.json')).write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n')
publication=json.loads((root/'wiki/drafts/measurements/operand_order_overflow_2026_10_29/publication/results.json').read_text())
for path,digest in publication['inputs'].items():assert sha(root/path)==digest
for p in (root/'source/library/std').rglob('*.omg'):inputs[str(p.relative_to(root))]=sha(p)
quality=json.loads((base/'quality.json').read_text())
old=next(r for r in publication['records'] if 'artifact_sha256' in r)
new=next(r for r in quality if r['fixture']==old['fixture'] and 'artifact_sha256' in r)
assert old['artifact_sha256']==new['artifact_sha256']
summary['first_updates']=[dict(fixture=r['fixture'],seconds=r['seconds'],exit=r['exit']) for r in quality if r['command'][1]=='update' and '--resume' not in r['command']]
summary['candidate_sha256']=sha(base/'candidate/omega.exe')
summary['base']='cb087c46476f255397ba2e8cc3cbba4dd978c2ea'
for name in ['candidate.patch','build.log','chain.py','quality.py','providers.py','model.py','model.json','collect.py']:
    copy(base/name,record/name)
(record/'inputs.json').write_text(json.dumps(inputs,indent=2),encoding='utf-8',newline='\n')
(record/'summary.json').write_text(json.dumps(summary,indent=2),encoding='utf-8',newline='\n')
print(json.dumps(summary,indent=2))
