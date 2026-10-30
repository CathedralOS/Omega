"""Verify exact artifacts, runtime observations and paired replay accounting."""
import hashlib,json,pathlib
root=pathlib.Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name
expected_image='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
identities={row['producer']:row for row in json.loads((root/'identities.json').read_text())}
records={}
for p in sorted((root/'raw').glob('*/result.json')):
    d=json.loads(p.read_text());records[p.parent.name]=d
    assert d['exit_code']==0 and not d['phase_profile'],p
    assert d['compiler_sha256']==identities[d['producer']]['compiler_sha256'],p
    if 'image_sha256' in d:
        assert d['image_sha256']==expected_image and d['cache_mode']=='fresh private cache',p
        runtime=p.parent/'runtime'
        assert json.loads((runtime/'result.json').read_text())['exit_code']==0,p
        assert b'A bottomless dark room near the dungeon heart.' in (runtime/'stdout.txt').read_bytes(),p
    for row in d['reconciliation']:
        assert sum(value for key,value in row['metrics'].items() if key.endswith('_ns'))<=row['wall_ns'],p
for row in json.loads((root/'summary.json').read_text()):
    values=[]
    for kind in ['base','candidate']:
        d=records['edge-ab-'+kind+'-'+row['pair']]
        m=max((r for r in d['reconciliation'] if r['operation']=='memberships'),key=lambda r:r['machines'])
        value=m['metrics']['source_contract_queries_ns']/1e6
        assert value==row[kind]['source_replay_ms']
        values.append(value)
    change=(values[1]/values[0]-1)*100
    assert change==row['replay_change_percent']
    print(row['pair'],*[round(value,2) for value in values],f'{change:+.1f}%')
comparison=records['edge-verified'];folder=root/'raw/edge-verified'
assert comparison['terminal_stdout_agrees'] and comparison['retained_image_sha256']==expected_image
assert (folder/'terminal.stdout').read_bytes()==(folder/'stdout.txt').read_bytes()
assert hashlib.sha256((folder/'stdout.txt').read_bytes()).hexdigest()==comparison['guest_stdout_sha256']
comparison=json.loads((root/'counter-comparison.json').read_text())
old=json.loads((root.parent/comparison['baseline']).read_text())
def counts(d):
    return [[r['operation'],r['machines'],{k:v for k,v in r['metrics'].items() if not k.endswith('_ns')}] for r in d['reconciliation']]
assert counts(old)==counts(records['edge-counts'])==comparison['matching_counts']
print('Verified',len(records),'observations and',len(manifest),'artifact digests; stdout and counter rosters agree.')
