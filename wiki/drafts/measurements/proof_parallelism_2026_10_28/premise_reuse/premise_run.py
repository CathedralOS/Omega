from collections import Counter
from pathlib import Path
import hashlib,importlib.util,json,os,re

ROOT=next(parent for parent in Path(__file__).resolve().parents if (parent/'AGENTS.md').is_file())
spec=importlib.util.spec_from_file_location('existing_cache_measure',ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/cache_diagnostics_measure.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
measure=module.measure

OUT=ROOT/'build/proof-profile'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'

def parse(text):
    active=set(); windows=0; hot=Counter(); all_rows=Counter(); scopes=[]; cost=Counter(); all_cost=Counter()
    for line in text.splitlines():
        fields=line.split('\t')
        if fields[0]=='PREMISE_SOURCE_BEGIN':
            assert fields[1] not in active;active.add(fields[1]);windows+=1
        elif fields[0]=='PREMISE_SOURCE_END': active.remove(fields[1])
        elif fields[0] in {'PREMISE_REUSE','PREMISE_COST'}:
            tag,thread,*raw=fields
            row={key:int(value) for key,value in (item.split('=') for item in raw)}
            if tag=='PREMISE_REUSE':
                assert row['rows']==row['hits']+row['unique']
                assert 0<=row['kept_hits']<=row['kept']<=row['rows']
                assert row['unchanged']<=row['kept']
                all_rows.update(row)
                if thread in active: hot.update(row); scopes.append(row)
            else:
                all_cost.update(row)
                if thread in active:cost.update(row)
    assert not active
    return dict(windows=windows,hot=dict(hot),global_counts=dict(all_rows),hot_cost=dict(cost),global_cost=dict(all_cost),
        hot_scopes=len(scopes),peak_unique_rows_per_query=max((row['unique'] for row in scopes),default=0))

assert parse('PREMISE_SOURCE_BEGIN\tt\nPREMISE_REUSE\tt\trows=3\thits=2\tunique=1\tkept=3\tkept_hits=2\tunchanged=3\nPREMISE_SOURCE_END\tt')['hot']['hits']==2
assert parse('ordinary output')['hot']=={}

def run(producer,label,mode='',comparison=False):
    for flag in ['OMEGA_EDGE_CACHE','OMEGA_EDGE_REBIND_PROFILE','OMEGA_PAYLOAD_ENTRY_PROFILE','OMEGA_REBIND_DELTA_COUNTS']:
        os.environ.pop(flag,None)
    if mode:os.environ['OMEGA_PREMISE_PROBE']=mode
    else:os.environ.pop('OMEGA_PREMISE_PROBE',None)
    result=measure(producer,label,'counts' if mode=='counts' else 'timing',comparison=comparison)
    folder=OUT/label; stderr=(folder/'stderr.txt').read_text()
    parsed=parse(stderr)
    if mode:
        assert parsed['windows']==418,parsed
        counted=parsed['hot'] if mode=='counts' else parsed['hot_cost']
        if mode=='counts':assert counted['requests']==7683
        assert counted['rows']==3140756,counted
    else:assert not parsed['windows']
    result.update(premise_mode=mode or 'off',premise=parsed)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip())
        stdout=(folder/'stdout.txt').read_bytes();assert stdout==(kept/'terminal.stdout').read_bytes()
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2))
    print('PREMISE_SUMMARY',label,json.dumps(parsed),flush=True)
    return result

if __name__=='__main__':
    run('payload','premise-control-base')
    run('premise-probe','premise-control-off')
    first=run('premise-probe','premise-counts-a','counts')
    run('premise-probe','premise-cost-a','timing')
    run('premise-probe','premise-cost-b','timing')
    second=run('premise-probe','premise-counts-b','counts')
    assert first['premise']==second['premise']
    run('premise-probe','premise-verified',comparison=True)
