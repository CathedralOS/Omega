from cache_diagnostics_measure import measure,invoke
from pathlib import Path
from collections import Counter
import os,json,hashlib,re,gzip
ROOT=next(parent for parent in Path(__file__).resolve().parents if (parent/'AGENTS.md').exists())
OUT=ROOT/'build/proof-profile'

def parse(folder,cache,mode):
    active_queries=set();active_drops=set();query_rows=[];cleanup_rows=[];all_query_rows=[];all_cleanup_rows=[];windows=0
    stderr=folder/'stderr.txt'
    content=stderr.read_text() if stderr.exists() else gzip.decompress((folder/'stderr.txt.gz').read_bytes()).decode()
    for line in content.splitlines():
        fields=line.split('\t')
        if fields[0]=='SOURCE_QUERY_BEGIN':
            assert fields[1] not in active_queries;active_queries.add(fields[1]);windows+=1
        elif fields[0]=='SOURCE_QUERY_END':active_queries.remove(fields[1])
        elif fields[0]=='SOURCE_HISTORY_DROP_BEGIN':
            assert fields[1] not in active_drops;active_drops.add(fields[1])
        elif fields[0]=='SOURCE_HISTORY_DROP_END':active_drops.remove(fields[1])
        elif fields[0]=='REBIND':
            _,kind,thread,wall,*metrics=fields
            row=dict(wall_ns=int(wall),metrics={key:int(value) for key,value in (item.split('=') for item in metrics)})
            if kind=='query':all_query_rows.append(row)
            elif kind=='cleanup':all_cleanup_rows.append(row)
            if kind=='query' and thread in active_queries:query_rows.append(row)
            elif kind=='cleanup' and thread in active_queries|active_drops:cleanup_rows.append(row)
    assert not active_queries and not active_drops
    if mode=='off':assert not query_rows and not cleanup_rows and windows==0;return None
    assert windows==418 and len(query_rows)==173,(windows,len(query_rows))
    query=Counter();cleanup=Counter()
    for target,rows in [(query,query_rows),(cleanup,cleanup_rows)]:
        for row in rows:
            if mode=='timing':assert sum(value for key,value in row['metrics'].items() if key.endswith('_ns'))<=row['wall_ns'],row
            target.update(row['metrics'])
    global_query=Counter();global_cleanup=Counter()
    for row in all_query_rows:global_query.update(row['metrics'])
    for row in all_cleanup_rows:global_cleanup.update(row['metrics'])
    result=dict(global_query_rows=len(all_query_rows),global_query_metrics=dict(global_query),global_cleanup_metrics=dict(global_cleanup),source_windows=windows,query_rows=len(query_rows),cleanup_rows=len(cleanup_rows),query_metrics=dict(query),cleanup_metrics=dict(cleanup))
    if cache=='on':
        hits=query['same_allocation_checks']-query['cold_entries']
        misses=query['cold_entries']+query['changed_allocation_misses']
        assert hits>=0 and hits+misses==query['requests']
        assert misses==query['transform_calls']
        if mode=='counts':assert query['equal_contents_misses']+query['different_contents_misses']==query['changed_allocation_misses']
        global_hits=global_query['same_allocation_checks']-global_query['cold_entries']
        assert global_hits>=0 and global_hits+global_query['cold_entries']+global_query['changed_allocation_misses']==global_query['requests']
        assert global_query['transform_calls']==global_query['cold_entries']+global_query['changed_allocation_misses']
        result.update(global_true_hits=global_hits,global_hit_rate=global_hits/global_query['requests'] if global_query['requests'] else None,true_hits=hits,misses=misses,hit_rate=hits/query['requests'] if query['requests'] else None,
                      avoided_input_rows=query['same_allocation_input_rows']-query['cold_input_rows'])
    else:assert query['requests']==query['transform_calls']
    (folder/'cache-detail.json').write_text(json.dumps(result,indent=2))
    return result

def run(cache,mode,label,producer='edge-cache-probe',comparison=False):
    os.environ['OMEGA_EDGE_CACHE']=cache;os.environ['OMEGA_EDGE_REBIND_PROFILE']=mode
    result=measure(producer,label,'counts' if mode=='counts' else 'timing',comparison=comparison)
    folder=OUT/label;detail=parse(folder,cache,mode) if producer in ['edge-cache-probe','edge-cache-cost-probe'] else None
    result.update(edge_cache=cache,edge_rebind_profile=mode)
    if detail:result['cache_detail']=detail
    if comparison:
        retained=Path(re.search(r'build dir kept: (.+)',(folder/'stderr.txt').read_text())[1].strip())
        stdout=(folder/'stdout.txt').read_bytes();assert stdout==(retained/'terminal.stdout').read_bytes()
        image=hashlib.sha256((retained/'omega-program.exe').read_bytes()).hexdigest();assert image=='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=image,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2))
    print(label,json.dumps(detail),flush=True)
    return result

if __name__=='__main__':
    run('on','counts','cache-count-on-a')
    run('off','counts','cache-count-off')
    run('on','counts','cache-count-on-b')
    for cache,letter in [('off','a'),('on','a'),('on','b'),('off','b'),('off','c'),('on','c')]:
        run(cache,'off','cache-ab-'+cache+'-'+letter)
    run('on','timing','cache-detail-on')
    run('off','timing','cache-detail-off')
    run('off','off','cache-frozen-base',producer='payload')
    run('on','off','cache-frozen-candidate',producer='edge-rebinding')
    run('on','off','cache-runtime-verified',comparison=True)
