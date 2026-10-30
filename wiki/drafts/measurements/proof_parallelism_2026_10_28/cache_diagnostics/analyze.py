"""Verify the retained diagnostics without running any compiler."""
from pathlib import Path
import ast,gzip,hashlib,json
root=Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name

def read(path):
    data=path.read_bytes() if path.exists() else gzip.decompress(path.with_name(path.name+'.gz').read_bytes())
    return data

def load(label,name='result.json'):
    return json.loads(read(root/'raw'/label/name))

identities={row['producer']:row for row in json.loads((root/'identities.json').read_text())}
assert all(int(row['build_profile']['opt_level'])>0 for row in identities.values())
image='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
folders=sorted((root/'raw').iterdir());assert len(folders)==18
for folder in folders:
    d=load(folder.name);assert d['exit_code']==0 and not d['phase_profile'],folder
    assert d['compiler_sha256']==identities[d['producer']]['compiler_sha256']
    assert d['accepted_lock_sha256']==identities[d['producer']]['accepted_lock_sha256']
    for row in d['reconciliation']:
        assert sum(value for key,value in row['metrics'].items() if key.endswith('_ns'))<=row['wall_ns']
    if 'image_sha256' in d:
        assert d['image_sha256']==image and d['cache_mode']=='fresh private cache'
        assert json.loads(read(folder/'runtime/result.json'))['exit_code']==0
        assert b'A bottomless dark room near the dungeon heart.' in read(folder/'runtime/stdout.txt')
    else:
        assert d['terminal_stdout_agrees'] and d['retained_image_sha256']==image
        assert read(folder/'stdout.txt')==read(folder/'terminal.stdout')
        assert b'interp exit: 0' in read(folder/'stderr.txt') and b'DECLINED' not in read(folder/'stderr.txt')
    if d.get('cache_detail'):
        detail=d['cache_detail'];assert detail==load(folder.name,'cache-detail.json')
        assert detail['source_windows']==418 and detail['query_rows']==173
        q=detail['query_metrics']
        if d['edge_cache']=='on':
            assert detail['true_hits']==q['same_allocation_checks']-q['cold_entries']
            assert detail['misses']==q['cold_entries']+q.get('changed_allocation_misses',0)==q['transform_calls']
            assert detail['true_hits']+detail['misses']==q['requests']
        else:assert q['transform_calls']==q['requests']

one=load('cache-count-on-a','cache-detail.json');two=load('cache-count-on-b','cache-detail.json');assert one==two
q=one['query_metrics'];off=load('cache-count-off','cache-detail.json')['query_metrics']
assert (one['true_hits'],one['misses'],q['cold_entries'],q['changed_allocation_misses'])==(24,7659,1537,6122)
assert q['equal_contents_misses']+q['different_contents_misses']==q['changed_allocation_misses']
for key in ['requests','requested_input_rows','returned_output_rows','appended_output_rows','contained_requests','contained_output_rows','append_cow','retire_ambient_cow','retire_shared_cow','cow_row_handles']:
    assert q[key]==off[key],key
assert q['wrapped_output_rows']-off['appended_output_rows']==1683415
for mode in ['on','off']:
    expected=q if mode=='on' else off
    for letter in 'ab':
        measured=load(f'cache-cost-{mode}-{letter}','cache-detail.json')['query_metrics']
        for key,value in expected.items():
            if key not in ['equal_contents_misses','different_contents_misses']:assert measured.get(key,0)==value,key
for p in root.glob('*.py'):ast.parse(p.read_text(),filename=str(p))
print('Verified',len(manifest),'artifact digests and 18 observations; cold/hit/miss identities, repeated counts, COW and native/Terminal output agree.')
print('Source replay:',one['true_hits'],'real hits /',q['requests'],'requests;',one['avoided_input_rows'],'/',q['requested_input_rows'],'input rows avoided.')
