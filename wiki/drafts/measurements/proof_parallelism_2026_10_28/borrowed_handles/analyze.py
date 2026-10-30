"""Verify borrowed-handle diagnostics and the balanced three-producer comparison."""
from pathlib import Path
import ast,hashlib,importlib.util,json,statistics
root=Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name
spec=importlib.util.spec_from_file_location('borrow_parser',root/'borrow_run.py')
parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
identities={row['producer']:row for row in json.loads((root/'identities.json').read_text())}
summary=json.loads((root/'summary.json').read_text())
image='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
folders=sorted((root/'raw').iterdir());assert len(folders)==21
baseline=json.loads((root.parent/'payload_replay/raw/payload-counts/result.json').read_text())
def counters(result):return [[row['operation'],row['machines'],{key:value for key,value in row['metrics'].items() if not key.endswith('_ns')}] for row in result['reconciliation']]
def record(label):return json.loads((root/'raw'/label/'result.json').read_text())
for folder in folders:
    result=record(folder.name);identity=identities[result['producer']]
    assert result['exit_code']==0 and not result['phase_profile']
    assert result['compiler_sha256']==identity['compiler_sha256'] and result['accepted_lock_sha256']==identity['accepted_lock_sha256']
    assert int(identity['build_profile']['opt_level'])>0
    assert result['borrowing']==parser.parse((folder/'stderr.txt').read_text())
    if result['borrowing_counts']:assert counters(result)==counters(baseline)
    else:assert result['borrowing']['windows']==0
    if 'image_sha256' in result:
        assert result['image_sha256']==image and result['cache_mode']=='fresh private cache'
        assert json.loads((folder/'runtime/result.json').read_text())['exit_code']==0
        assert b'A bottomless dark room near the dungeon heart.' in (folder/'runtime/stdout.txt').read_bytes()
    else:
        assert result['terminal_stdout_agrees'] and result['retained_image_sha256']==image
        assert (folder/'stdout.txt').read_bytes()==(folder/'terminal.stdout').read_bytes()
        assert b'interp exit: 0' in (folder/'stderr.txt').read_bytes() and b'DECLINED' not in (folder/'stderr.txt').read_bytes()
first=record('borrow-counts-a')['borrowing'];assert first==record('borrow-counts-b')['borrowing']
hot=first['hot']
assert (hot['requests'],hot['incoming_rows'],hot['output_rows'],hot['borrowed_rows'],hot['owned_rows'])==(7683,3140756,3146452,3085153,61299)
assert hot['cloned_handles']+hot['avoided_handle_clones']==hot['borrowed_rows']
assert hot['appending_requests']+hot['contained_requests']==hot['requests']
assert [trial['round'] for trial in summary['rounds']]==list('abcdef')
assert [trial['order'] for trial in summary['rounds']]==parser.ORDER
for trial in summary['rounds']:
    for kind in parser.PRODUCERS:
        result=record('borrow-'+kind+'-'+trial['round'])
        assert result['producer']==parser.PRODUCERS[kind]
        main=max((row for row in result['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
        assert trial[kind]['source_replay_ms']==main['metrics']['source_contract_queries_ns']/1e6
        for metric in ['wall_seconds','cpu_seconds','peak_working_set_bytes','peak_commit_bytes']:assert trial[kind][metric]==result[metric]
    for reference in ['original','eager']:
        for metric,percent in trial['borrowed_vs_'+reference+'_percent'].items():assert percent==(trial['borrowed'][metric]/trial[reference][metric]-1)*100
for kind in parser.PRODUCERS:
    for metric,value in summary['medians'][kind].items():assert value==statistics.median(trial[kind][metric] for trial in summary['rounds'])
for path in root.glob('*.py'):ast.parse(path.read_text(),filename=str(path))
print('Verified',len(manifest),'artifact identities, twenty-one observations, exact repeated counts, balanced order and native/Terminal outcomes.')
print('Hot ownership counts:',hot)
for reference in ['original','eager']:
    print('Borrowed vs',reference,[{key:round(value,2) for key,value in trial['borrowed_vs_'+reference+'_percent'].items()} for trial in summary['rounds']])
print(json.dumps(summary['medians'],indent=2))
