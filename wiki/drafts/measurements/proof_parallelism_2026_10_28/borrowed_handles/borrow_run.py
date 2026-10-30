from collections import Counter
from pathlib import Path
import hashlib,importlib.util,json,os,re,statistics,subprocess

ROOT=next(parent for parent in Path(__file__).resolve().parents if (parent/'AGENTS.md').is_file())
spec=importlib.util.spec_from_file_location('established_measure',ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/cache_diagnostics_measure.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
OUT=ROOT/'build/proof-profile'
IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
PRODUCERS=dict(original='payload',eager='unchanged-sharing',borrowed='borrowed-handles')
ORDER=[['original','eager','borrowed'],['eager','borrowed','original'],['borrowed','original','eager'],
    ['borrowed','eager','original'],['original','borrowed','eager'],['eager','original','borrowed']]

def parse(text):
    active=set();windows=0;hot=Counter();all_counts=Counter()
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]=='BORROW_SOURCE_BEGIN':assert f[1] not in active;active.add(f[1]);windows+=1
        elif f[0]=='BORROW_SOURCE_END':active.remove(f[1])
        elif f[0]=='BORROW_ROWS':
            _,thread,incoming,output,borrowed,append=f;incoming,output,borrowed,append=map(int,[incoming,output,borrowed,append])
            assert 0<=borrowed<=min(incoming,output) and append in [0,1]
            row=dict(requests=1,incoming_rows=incoming,output_rows=output,borrowed_rows=borrowed,owned_rows=output-borrowed,
                appending_requests=append,contained_requests=1-append,
                cloned_handles=borrowed*append,avoided_handle_clones=borrowed*(1-append))
            all_counts.update(row)
            if thread in active:hot.update(row)
    assert not active
    return dict(windows=windows,hot=dict(hot),global_counts=dict(all_counts))

assert parse('BORROW_SOURCE_BEGIN\tt\nBORROW_ROWS\tt\t3\t4\t2\t0\nBORROW_SOURCE_END\tt')['hot']['avoided_handle_clones']==2
assert parse('BORROW_ROWS\tt\t3\t4\t2\t1')['global_counts']['cloned_handles']==2
assert parse('ordinary stderr')['hot']=={}

def run(kind,label,counts=False,comparison=False):
    for name in ['OMEGA_EDGE_CACHE','OMEGA_EDGE_REBIND_PROFILE','OMEGA_PAYLOAD_ENTRY_PROFILE','OMEGA_REBIND_DELTA_COUNTS','OMEGA_PREMISE_PROBE','OMEGA_UNCHANGED_FACT_COUNTS']:
        os.environ.pop(name,None)
    if counts:os.environ['OMEGA_BORROWED_FACT_COUNTS']='1'
    else:os.environ.pop('OMEGA_BORROWED_FACT_COUNTS',None)
    result=module.measure(PRODUCERS[kind],label,'counts' if counts else 'timing',comparison=comparison)
    folder=OUT/label;stderr=(folder/'stderr.txt').read_text();parsed=parse(stderr)
    if counts:
        hot=parsed['hot'];assert parsed['windows']==418
        assert hot['requests']==7683 and hot['incoming_rows']==3140756 and hot['output_rows']==3146452
        assert hot['borrowed_rows']==3085153 and hot['owned_rows']==61299,parsed
        assert hot['cloned_handles']+hot['avoided_handle_clones']==hot['borrowed_rows']
    else:assert parsed['windows']==0
    result.update(borrowing_counts=counts,borrowing=parsed)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip())
        stdout=(folder/'stdout.txt').read_bytes();assert stdout==(kept/'terminal.stdout').read_bytes()
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n')
    if counts:print('BORROW_COUNTS',json.dumps(parsed),flush=True)
    return result

def summarize():
    rounds=[]
    for letter,order in zip('abcdef',ORDER):
        trial=dict(round=letter,order=order)
        for kind in PRODUCERS:
            record=json.loads((OUT/('borrow-'+kind+'-'+letter)/'result.json').read_text())
            main=max((row for row in record['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
            trial[kind]=dict(wall_seconds=record['wall_seconds'],cpu_seconds=record['cpu_seconds'],source_replay_ms=main['metrics']['source_contract_queries_ns']/1e6,
                membership_ms=main['wall_ns']/1e6,peak_working_set_bytes=record['peak_working_set_bytes'],peak_commit_bytes=record['peak_commit_bytes'])
        for reference in ['original','eager']:
            trial['borrowed_vs_'+reference+'_percent']={metric:(trial['borrowed'][metric]/trial[reference][metric]-1)*100 for metric in ['source_replay_ms','wall_seconds','cpu_seconds']}
        rounds.append(trial)
    result=dict(rounds=rounds,medians={kind:{metric:statistics.median(trial[kind][metric] for trial in rounds) for metric in rounds[0][kind]} for kind in PRODUCERS})
    (OUT/'borrowed-handles/summary.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('BORROW_SUMMARY',json.dumps(result),flush=True)
    return result

if __name__=='__main__':
    old={row['producer']:row for row in json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/unchanged_sharing/identities.json').read_text())}
    identities=[]
    for producer in PRODUCERS.values():
        folder=OUT/'producers'/producer
        profile=json.loads(subprocess.check_output([str(folder/'omega.exe'),'--build-profile']))
        identity=dict(producer=producer,compiler_sha256=hashlib.sha256((folder/'omega.exe').read_bytes()).hexdigest(),
            accepted_lock_sha256=hashlib.sha256((folder/'omega.lock').read_bytes()).hexdigest(),build_profile=profile)
        assert int(profile['opt_level'])>0
        if producer in old:assert identity==old[producer]
        identities.append(identity)
    (OUT/'borrowed-handles/identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    first=run('borrowed','borrow-counts-a',counts=True)
    for letter,order in zip('abcdef',ORDER):
        for kind in order:run(kind,'borrow-'+kind+'-'+letter)
    second=run('borrowed','borrow-counts-b',counts=True)
    assert first['borrowing']==second['borrowing']
    run('borrowed','borrow-verified',comparison=True)
    summarize()
