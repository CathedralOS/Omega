"""Check preserved premise diagnostics without running the compiler."""
from pathlib import Path
import ast,hashlib,importlib.util,json
root=Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name
spec=importlib.util.spec_from_file_location('premise_parser',root/'premise_run.py')
parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
identities={row['producer']:row for row in json.loads((root/'identities.json').read_text())}
summary=json.loads((root/'summary.json').read_text())
expected='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
folders=sorted((root/'raw').iterdir());assert len(folders)==7
baseline=json.loads((root.parent/'payload_replay/raw/payload-counts/result.json').read_text())
def counters(result):return [[row['operation'],row['machines'],{key:value for key,value in row['metrics'].items() if not key.endswith('_ns')}] for row in result['reconciliation']]
for folder in folders:
    result=json.loads((folder/'result.json').read_text())
    assert result['exit_code']==0 and not result['phase_profile']
    identity=identities[result['producer']]
    assert result['compiler_sha256']==identity['compiler_sha256']
    assert result['accepted_lock_sha256']==identity['accepted_lock_sha256']
    assert int(identity['build_profile']['opt_level'])>0
    stderr=(folder/'stderr.txt').read_text()
    assert result['premise']==parser.parse(stderr)
    main=max((row for row in result['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
    assert main['machines']==202
    assert summary[folder.name]['source_replay_ms']==main['metrics']['source_contract_queries_ns']/1e6
    if result['premise_mode']=='counts':
        assert counters(result)==counters(baseline)
        hot=result['premise']['hot'];assert hot['requests']==7683 and hot['rows']==3140756
    elif result['premise_mode']=='timing':
        hot=result['premise']['hot_cost'];assert hot['rows']==3140756
        assert sum(value for key,value in hot.items() if key.endswith('_ns'))<=main['metrics']['source_contract_queries_ns']
    if 'image_sha256' in result:
        assert result['image_sha256']==expected and result['cache_mode']=='fresh private cache'
        assert json.loads((folder/'runtime/result.json').read_text())['exit_code']==0
        assert b'A bottomless dark room near the dungeon heart.' in (folder/'runtime/stdout.txt').read_bytes()
    else:
        assert result['terminal_stdout_agrees'] and result['retained_image_sha256']==expected
        assert (folder/'stdout.txt').read_bytes()==(folder/'terminal.stdout').read_bytes()
        assert b'interp exit: 0' in (folder/'stderr.txt').read_bytes()
        assert b'DECLINED' not in (folder/'stderr.txt').read_bytes()
first=json.loads((root/'raw/premise-counts-a/result.json').read_text())['premise']
second=json.loads((root/'raw/premise-counts-b/result.json').read_text())['premise']
assert first==second
for path in root.glob('*.py'):ast.parse(path.read_text(),filename=str(path))
print('Verified',len(manifest),'artifact identities, seven observations, repeated exact counts, work rosters and native/Terminal outcomes.')
print(json.dumps(first,indent=2))
