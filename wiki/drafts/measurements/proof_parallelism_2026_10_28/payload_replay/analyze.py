"""Verify retained artifacts and show paired causal timings; no compiler execution."""
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]) if len(sys.argv)>1 else pathlib.Path(__file__).resolve().parent
manifest=json.loads((root/'manifest.json').read_text())
for name,expected in manifest.items():
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name
expected_image='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'
records={}
for file in sorted((root/'raw').glob('*/result.json')):
    result=json.loads(file.read_text());records[file.parent.name]=result
    assert result['exit_code']==0,file
    if 'image_sha256' in result:assert result['image_sha256']==expected_image,file
    for row in result['reconciliation']:
        assert sum(value for name,value in row['metrics'].items() if name.endswith('_ns'))<=row['wall_ns'],file
for prefix,candidate,letters in [('payload-ab-base-','payload-ab-index-','abcde'),('rosters-ab-base-','rosters-ab-candidate-','abc')]:
    print(candidate)
    for letter in letters:
        values=[]
        for name in [prefix+letter,candidate+letter]:
            result=records[name]
            assert result['cache_mode']=='fresh private cache' and not result['phase_profile']
            row=max((row for row in result['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
            values.append(round(row['metrics']['source_contract_queries_ns']/1e6,2))
        print(letter,*values)
comparison=records['rosters-verified']
assert comparison['terminal_stdout_agrees'] and comparison['retained_image_sha256']==expected_image
print('Verified',len(manifest),'retained byte identities and',len(records),'successful observations.')
