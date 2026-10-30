"""Validate the fixed-Dungeon reconstruction-sharing experiment from artifacts."""
from pathlib import Path
import ast,hashlib,importlib.util,json,statistics
root=Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name
spec=importlib.util.spec_from_file_location('sharing_parser',root/'share_run.py')
parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
identities={row['producer']:row for row in json.loads((root/'identities.json').read_text())}
summary=json.loads((root/'summary.json').read_text())
image='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
folders=sorted((root/'raw').iterdir());assert len(folders)==15
baseline=json.loads((root.parent/'payload_replay/raw/payload-counts/result.json').read_text())
def counters(result):return [[row['operation'],row['machines'],{key:value for key,value in row['metrics'].items() if not key.endswith('_ns')}] for row in result['reconciliation']]
def record(label):return json.loads((root/'raw'/label/'result.json').read_text())
for folder in folders:
    result=record(folder.name);identity=identities[result['producer']]
    assert result['exit_code']==0 and not result['phase_profile']
    assert result['compiler_sha256']==identity['compiler_sha256'] and result['accepted_lock_sha256']==identity['accepted_lock_sha256']
    assert int(identity['build_profile']['opt_level'])>0
    assert result['sharing']==parser.parse((folder/'stderr.txt').read_text())
    if result['sharing_counts']:assert counters(result)==counters(baseline)
    else:assert result['sharing']['windows']==0
    if 'image_sha256' in result:
        assert result['image_sha256']==image and result['cache_mode']=='fresh private cache'
        assert json.loads((folder/'runtime/result.json').read_text())['exit_code']==0
        assert b'A bottomless dark room near the dungeon heart.' in (folder/'runtime/stdout.txt').read_bytes()
    else:
        assert result['terminal_stdout_agrees'] and result['retained_image_sha256']==image
        assert (folder/'stdout.txt').read_bytes()==(folder/'terminal.stdout').read_bytes()
        assert b'interp exit: 0' in (folder/'stderr.txt').read_bytes() and b'DECLINED' not in (folder/'stderr.txt').read_bytes()
first=record('share-counts')['sharing'];assert first==record('share-counts-b')['sharing']
assert first['hot']==dict(requests=7683,incoming_rows=3140756,output_rows=3146452,reused_rows=3085153,reconstructed_rows=61299)
assert [row['pair'] for row in summary['pairs']]==list('abcdef')
for pair in summary['pairs']:
    for kind in ['base','candidate']:
        result=record('share-'+kind+'-'+pair['pair'])
        main=max((row for row in result['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
        assert pair[kind]['source_replay_ms']==main['metrics']['source_contract_queries_ns']/1e6
        for metric in ['wall_seconds','cpu_seconds','peak_working_set_bytes','peak_commit_bytes']:assert pair[kind][metric]==result[metric]
    assert pair['replay_change_percent']==(pair['candidate']['source_replay_ms']/pair['base']['source_replay_ms']-1)*100
for kind in ['base','candidate']:
    for metric,value in summary['medians'][kind].items():assert value==statistics.median(pair[kind][metric] for pair in summary['pairs'])
for path in root.glob('*.py'):ast.parse(path.read_text(),filename=str(path))
print('Verified',len(manifest),'artifact identities, fifteen observations, exact repeated sharing counts, unchanged work rosters and native/Terminal outcomes.')
print('Paired replay changes:',[round(row['replay_change_percent'],2) for row in summary['pairs']])
print(json.dumps(summary['medians'],indent=2))
