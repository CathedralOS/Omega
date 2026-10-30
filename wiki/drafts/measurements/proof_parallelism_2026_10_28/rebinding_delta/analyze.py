"""Verify raw artifacts and the two bounded real-program delta experiments."""
from pathlib import Path
import gzip,hashlib,json,statistics,ast
root=Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name

def read(path):return path.read_bytes() if path.exists() else gzip.decompress(path.with_name(path.name+'.gz').read_bytes())
def record(label):return json.loads(read(root/'raw'/label/'result.json'))
identities={row['producer']:row for row in json.loads((root/'identities.json').read_text())}
expected='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
folders=sorted((root/'raw').iterdir());assert len(folders)==24
for folder in folders:
    d=record(folder.name);assert d['exit_code']==0 and not d['phase_profile']
    assert d['compiler_sha256']==identities[d['producer']]['compiler_sha256']
    assert d['accepted_lock_sha256']==identities[d['producer']]['accepted_lock_sha256']
    assert int(identities[d['producer']]['build_profile']['opt_level'])>0
    for row in d['reconciliation']:
        assert sum(value for key,value in row['metrics'].items() if key.endswith('_ns'))<=row['wall_ns']
    if 'image_sha256' in d:
        assert d['image_sha256']==expected and d['cache_mode']=='fresh private cache'
        assert json.loads(read(folder/'runtime/result.json'))['exit_code']==0
        assert b'A bottomless dark room near the dungeon heart.' in read(folder/'runtime/stdout.txt')
    else:
        assert d['terminal_stdout_agrees'] and d['retained_image_sha256']==expected
        assert read(folder/'terminal.stdout')==read(folder/'stdout.txt')
        assert b'interp exit: 0' in read(folder/'stderr.txt') and b'DECLINED' not in read(folder/'stderr.txt')

baseline=json.loads((root.parent/'payload_replay/raw/payload-counts/result.json').read_text())
def counts(d):return [[r['operation'],r['machines'],{k:v for k,v in r['metrics'].items() if not k.endswith('_ns')}] for r in d['reconciliation']]
for name in ['delta-counts','delta-sorted-counts']:
    d=record(name);assert counts(d)==counts(baseline)
    active=set();windows=0;requests=before=after=empty=0
    for line in read(root/'raw'/name/'stderr.txt').decode().splitlines():
        f=line.split('\t')
        if f[0]=='DELTA_SOURCE_BEGIN':assert f[1] not in active;active.add(f[1]);windows+=1
        elif f[0]=='DELTA_SOURCE_END':active.remove(f[1])
        elif f[0]=='REBIND_DELTA' and f[1] in active:
            a,b=int(f[2]),int(f[3]);assert 0<=b<=a
            requests+=1;before+=a;after+=b;empty+=b==0
    assert not active and windows==418
    assert (requests,before,after,empty)==(7683,3146452,61299,6921)
    assert d['delta_metrics']['hot']==dict(requests=requests,computed_rows=before,allocated_rows=after,empty_results=empty)
for prefix,letters in [('delta','abcd'),('delta-sorted','abcdef')]:
    summary=json.loads((root/(prefix+'-summary.json')).read_text())
    assert [row['pair'] for row in summary['pairs']]==list(letters)
    for row in summary['pairs']:
        values=[]
        for kind in ['base','candidate']:
            d=record(prefix+'-'+kind+'-'+row['pair'])
            measured=max((r for r in d['reconciliation'] if r['operation']=='memberships'),key=lambda r:r['machines'])['metrics']['source_contract_queries_ns']/1e6
            assert measured==row[kind]['source_replay_ms'];values.append(measured)
        assert (values[1]/values[0]-1)*100==row['replay_change_percent']
    for kind in ['base','candidate']:
        for metric,value in summary['medians'][kind].items():assert statistics.median(row[kind][metric] for row in summary['pairs'])==value
    print(prefix,'paired source-replay changes:',[round(row['replay_change_percent'],1) for row in summary['pairs']])
for path in root.glob('*.py'):ast.parse(path.read_text(),filename=str(path))
print('Verified',len(manifest),'artifact identities and 24 observations; exact counts, unchanged work rosters and native/Terminal outputs agree.')
