from collections import Counter
from pathlib import Path
import hashlib,importlib.util,json,os,re,statistics

ROOT=next(parent for parent in Path(__file__).resolve().parents if (parent/'AGENTS.md').is_file())
spec=importlib.util.spec_from_file_location('established_measure',ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/cache_diagnostics_measure.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
OUT=ROOT/'build/proof-profile'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'

def parse(text):
    active=set();windows=0;hot=Counter();global_counts=Counter()
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]=='SHARE_SOURCE_BEGIN':assert f[1] not in active;active.add(f[1]);windows+=1
        elif f[0]=='SHARE_SOURCE_END':active.remove(f[1])
        elif f[0]=='SHARE_ROWS':
            _,thread,incoming,output,reused=f;incoming,output,reused=map(int,[incoming,output,reused])
            assert 0<=reused<=min(incoming,output)
            row=dict(requests=1,incoming_rows=incoming,output_rows=output,reused_rows=reused,reconstructed_rows=output-reused)
            global_counts.update(row)
            if thread in active:hot.update(row)
    assert not active
    return dict(windows=windows,hot=dict(hot),global_counts=dict(global_counts))

assert parse('SHARE_SOURCE_BEGIN\tt\nSHARE_ROWS\tt\t3\t4\t2\nSHARE_SOURCE_END\tt')['hot']['reconstructed_rows']==2
assert parse('ordinary stderr')['hot']=={}

def run(producer,label,counts=False,comparison=False):
    for name in ['OMEGA_EDGE_CACHE','OMEGA_EDGE_REBIND_PROFILE','OMEGA_PAYLOAD_ENTRY_PROFILE','OMEGA_REBIND_DELTA_COUNTS','OMEGA_PREMISE_PROBE']:
        os.environ.pop(name,None)
    if counts:os.environ['OMEGA_UNCHANGED_FACT_COUNTS']='1'
    else:os.environ.pop('OMEGA_UNCHANGED_FACT_COUNTS',None)
    result=module.measure(producer,label,'counts' if counts else 'timing',comparison=comparison)
    folder=OUT/label;stderr=(folder/'stderr.txt').read_text();parsed=parse(stderr)
    if counts:
        assert parsed['windows']==418
        assert parsed['hot']['requests']==7683 and parsed['hot']['incoming_rows']==3140756 and parsed['hot']['output_rows']==3146452,parsed
    else:assert parsed['windows']==0
    result.update(sharing_counts=counts,sharing=parsed)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip())
        stdout=(folder/'stdout.txt').read_bytes();assert stdout==(kept/'terminal.stdout').read_bytes()
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n')
    if counts:print('SHARING_COUNTS',json.dumps(parsed),flush=True)
    return result

def summarize():
    pairs=[]
    for letter in 'abcdef':
        pair=dict(pair=letter)
        for kind in ['base','candidate']:
            record=json.loads((OUT/('share-'+kind+'-'+letter)/'result.json').read_text())
            main=max((row for row in record['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
            pair[kind]=dict(wall_seconds=record['wall_seconds'],cpu_seconds=record['cpu_seconds'],source_replay_ms=main['metrics']['source_contract_queries_ns']/1e6,
                membership_ms=main['wall_ns']/1e6,peak_working_set_bytes=record['peak_working_set_bytes'],peak_commit_bytes=record['peak_commit_bytes'])
        pair['replay_change_percent']=(pair['candidate']['source_replay_ms']/pair['base']['source_replay_ms']-1)*100
        pairs.append(pair)
    result=dict(pairs=pairs,medians={kind:{metric:statistics.median(pair[kind][metric] for pair in pairs) for metric in pairs[0][kind]} for kind in ['base','candidate']})
    (OUT/'unchanged-sharing/summary.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('SHARING_SUMMARY',json.dumps(result),flush=True)
    return result

if __name__=='__main__':
    first=run('unchanged-sharing','share-counts',counts=True)
    for ordinal,letter in enumerate('abcdef'):
        cases=[('payload','share-base-'+letter),('unchanged-sharing','share-candidate-'+letter)]
        if ordinal%2:cases.reverse()
        for producer,label in cases:run(producer,label)
    second=run('unchanged-sharing','share-counts-b',counts=True)
    assert first['sharing']==second['sharing']
    run('unchanged-sharing','share-verified',comparison=True)
    summarize()
