from pathlib import Path
import hashlib,importlib.util,json,os,re,subprocess

ROOT=next(parent for parent in Path(__file__).resolve().parents if (parent/'AGENTS.md').is_file())
spec=importlib.util.spec_from_file_location('established_measure',ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/cache_diagnostics/cache_diagnostics_measure.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
OUT=ROOT/'build/proof-profile';IMAGE='3b29e826721e2740316df29272f36a9265afb23756d40baa6d0630a7f60f20b8'

def fields(raw):return {key:int(value) for key,value in (item.split('=') for item in raw)}
def parse(text):
    batches=[]
    for line in text.splitlines():
        f=line.split('\t')
        if f[0]=='SCAN_BATCH':
            _,mode,machines,*raw=f
            batches.append(dict(mode=mode,machines=int(machines),summary=fields(raw),entries={},queries=[]))
        elif f[0] in ['SCAN','SCAN_QUERY']:
            assert batches,'missing batch header'
            tag,mode,machines,*raw=f;batch=batches[-1]
            assert (mode,int(machines))==(batch['mode'],batch['machines'])
            if tag=='SCAN':
                name,*raw=raw;assert name not in batch['entries'];batch['entries'][name]=fields(raw)
            else:batch['queries'].append(fields(raw))
    for batch in batches:
        root=batch['entries'].get('source',dict(calls=0,inclusive_ns=0))
        assert root['calls']==batch['summary']['roots']
        assert root['inclusive_ns']==batch['summary']['wall_ns']
        assert sum(row['exclusive_ns'] for row in batch['entries'].values())==root['inclusive_ns']
        assert all(0<=row['exclusive_ns']<=row['inclusive_ns'] for row in batch['entries'].values())
    return batches

assert parse('ordinary output')==[]
assert parse('SCAN_BATCH\tcoarse\t1\troots=1\treplays=0\twall_ns=10\nSCAN\tcoarse\t1\tsource\tcalls=1\tinclusive_ns=10\texclusive_ns=10')[0]['summary']['wall_ns']==10

def run(producer,label,mode='off',comparison=False):
    for name in ['OMEGA_EDGE_CACHE','OMEGA_EDGE_REBIND_PROFILE','OMEGA_PAYLOAD_ENTRY_PROFILE','OMEGA_REBIND_DELTA_COUNTS','OMEGA_PREMISE_PROBE','OMEGA_UNCHANGED_FACT_COUNTS','OMEGA_BORROWED_FACT_COUNTS']:
        os.environ.pop(name,None)
    if mode=='off':os.environ.pop('OMEGA_REPLAY_SCAN_PROFILE',None)
    else:os.environ['OMEGA_REPLAY_SCAN_PROFILE']=mode
    result=module.measure(producer,label,'counts' if mode=='counts' else 'timing',comparison=comparison)
    folder=OUT/label;stderr=(folder/'stderr.txt').read_text();batches=parse(stderr)
    if mode=='off':assert not batches
    else:
        assert batches and all(batch['mode']==mode for batch in batches)
        assert [batch['machines'] for batch in batches]==[row['machines'] for row in result['reconciliation'] if row['operation']=='memberships']
        main=max(batches,key=lambda batch:batch['machines'])
        assert main['machines']==202 and main['summary']['roots']==418 and main['summary']['replays']==173,main['summary']
        assert main['entries']['replay']['calls']==173
        assert main['entries']['position']['calls']==15219
        assert main['entries']['edge']['calls']==17444
        outer=max((row for row in result['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
        assert main['summary']['wall_ns']<=outer['metrics']['source_contract_queries_ns']
        if mode=='counts':
            old=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/payload_replay/raw/payload-counts/result.json').read_text())
            def counters(record):return [[row['operation'],row['machines'],{key:value for key,value in row['metrics'].items() if not key.endswith('_ns')}] for row in record['reconciliation']]
            assert counters(result)==counters(old)
    result.update(scan_mode=mode,scans=batches)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip())
        stdout=(folder/'stdout.txt').read_bytes();assert stdout==(kept/'terminal.stdout').read_bytes()
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==IMAGE
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=IMAGE,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8',newline='\n')
    if batches:
        main=max(batches,key=lambda batch:batch['machines'])
        print('SCAN_SUMMARY',label,main['summary'],sorted([(name,row['exclusive_ns']/1e6) for name,row in main['entries'].items()],key=lambda row:row[1],reverse=True),flush=True)
    return result

def summarize():
    result={}
    for folder in sorted(OUT.glob('scans-*')):
        if not (folder/'result.json').is_file():continue
        record=json.loads((folder/'result.json').read_text())
        if 'scan_mode' not in record:continue
        main=max((row for row in record['reconciliation'] if row['operation']=='memberships'),key=lambda row:row['machines'])
        result[folder.name]=dict(producer=record['producer'],mode=record['scan_mode'],wall_seconds=record['wall_seconds'],cpu_seconds=record['cpu_seconds'],
            source_replay_ms=main['metrics']['source_contract_queries_ns']/1e6,membership_ms=main['wall_ns']/1e6,
            peak_working_set_bytes=record['peak_working_set_bytes'],peak_commit_bytes=record['peak_commit_bytes'])
    (OUT/'scans-profile/summary.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    return result

if __name__=='__main__':
    old=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/unchanged_sharing/identities.json').read_text())[0]
    identities=[]
    for producer in ['payload','scans-profile']:
        folder=OUT/'producers'/producer
        identity=dict(producer=producer,compiler_sha256=hashlib.sha256((folder/'omega.exe').read_bytes()).hexdigest(),
            accepted_lock_sha256=hashlib.sha256((folder/'omega.lock').read_bytes()).hexdigest(),
            build_profile=json.loads(subprocess.check_output([str(folder/'omega.exe'),'--build-profile'])))
        assert int(identity['build_profile']['opt_level'])>0
        if producer=='payload':assert identity==old
        identities.append(identity)
    (OUT/'scans-profile/identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    cases=[('scans-profile','scans-coarse-a','coarse'),('payload','scans-original-a','off'),('scans-profile','scans-off-a','off'),
        ('payload','scans-original-b','off'),('scans-profile','scans-off-b','off'),('scans-profile','scans-coarse-b','coarse'),
        ('scans-profile','scans-off-c','off'),('scans-profile','scans-coarse-c','coarse'),('payload','scans-original-c','off')]
    for case in cases:run(*case)
    run('scans-profile','scans-fine-a','fine')
    first=run('scans-profile','scans-counts-a','counts')
    run('scans-profile','scans-fine-b','fine')
    second=run('scans-profile','scans-counts-b','counts')
    assert first['scans']==second['scans']
    run('scans-profile','scans-verified','off',comparison=True)
    summarize()
