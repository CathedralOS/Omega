from cache_diagnostics_measure import measure,ROOT
from pathlib import Path
from collections import Counter
import os,json,re,hashlib
OUT=ROOT/'build/proof-profile'

def run(producer,label,counts=False,comparison=False):
    for name in ['OMEGA_EDGE_CACHE','OMEGA_EDGE_REBIND_PROFILE','OMEGA_PAYLOAD_ENTRY_PROFILE']:
        os.environ.pop(name,None)
    if counts:os.environ['OMEGA_REBIND_DELTA_COUNTS']='1'
    else:os.environ.pop('OMEGA_REBIND_DELTA_COUNTS',None)
    result=measure(producer,label,'counts' if counts else 'timing',comparison=comparison)
    folder=OUT/label
    result['delta_counts']=counts
    if counts:
        active=set();windows=0;hot=Counter();global_counts=Counter()
        for line in (folder/'stderr.txt').read_text().splitlines():
            fields=line.split('\t')
            if fields[0]=='DELTA_SOURCE_BEGIN':assert fields[1] not in active;active.add(fields[1]);windows+=1
            elif fields[0]=='DELTA_SOURCE_END':active.remove(fields[1])
            elif fields[0]=='REBIND_DELTA':
                _,thread,before,after=fields;before,after=int(before),int(after);assert 0<=after<=before
                row=dict(requests=1,computed_rows=before,allocated_rows=after,empty_results=int(after==0))
                global_counts.update(row)
                if thread in active:hot.update(row)
        assert not active and windows==418
        assert hot['requests']==7683 and hot['computed_rows']==3146452 and hot['empty_results']==6921,hot
        result['delta_metrics']=dict(hot=hot,global_counts=global_counts,windows=windows)
        print('DELTA_COUNTS',json.dumps(result['delta_metrics']),flush=True)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',(folder/'stderr.txt').read_text())[1].strip())
        stdout=(folder/'stdout.txt').read_bytes();assert stdout==(kept/'terminal.stdout').read_bytes()
        image=hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest();assert image=='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=image,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2))
    return result

if __name__=='__main__':
    run('rebinding-delta','delta-counts',counts=True)
    for producer,label in [('payload','delta-base-a'),('rebinding-delta','delta-candidate-a'),
       ('rebinding-delta','delta-candidate-b'),('payload','delta-base-b'),
       ('payload','delta-base-c'),('rebinding-delta','delta-candidate-c'),
       ('rebinding-delta','delta-candidate-d'),('payload','delta-base-d')]:
        run(producer,label)
    run('rebinding-delta','delta-verified',comparison=True)
