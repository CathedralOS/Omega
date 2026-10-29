from pathlib import Path
import hashlib,importlib.util,json,os,re,subprocess
ROOT=next(p for p in Path(__file__).resolve().parents if (p/'AGENTS.md').is_file())
spec=importlib.util.spec_from_file_location('scan_harness',ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/scans_run.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
OUT=ROOT/'build/proof-profile';WORK=OUT/'handle-cost'
LIMITS=[100,250,500,1000,2000,4000,8000,16000,32000,64000,128000,256000,1000000,4000000,16000000]
SCHEDULE=[('payload','handles-original-a','off'),('handle-cost','handles-off-a','off'),('handle-cost','handles-timed-a','fine'),
    ('handle-cost','handles-off-b','off'),('handle-cost','handles-timed-b','fine'),('payload','handles-original-b','off'),
    ('handle-cost','handles-timed-c','fine'),('payload','handles-original-c','off'),('handle-cost','handles-off-c','off')]

def supplemental(text,batches):
    current=-1;histograms=[];calibrations=[]
    for line in text.splitlines():
        fields=line.split('\t')
        if fields[0]=='SCAN_BATCH':current+=1
        elif fields[0]=='COPY_CALIBRATION':calibrations.append(h.fields(fields[1:]))
        elif fields[0]=='COPY_HIST':
            assert current>=0
            _,mode,machines,name,*values=fields
            assert (mode,int(machines))==(batches[current]['mode'],batches[current]['machines'])
            stats=h.fields(values);entry=batches[current]['entries'][name]
            bins=[stats['b'+str(n)] for n in range(16)]
            assert sum(bins)==entry['calls']
            assert stats['min_ns']*entry['calls']<=entry['inclusive_ns']<=stats['max_ns']*entry['calls']
            lower=sum(count*(0 if n==0 else LIMITS[n-1]+1) for n,count in enumerate(bins))
            upper=sum(count*(LIMITS[n] if n<15 else stats['max_ns']) for n,count in enumerate(bins))
            assert lower<=entry['inclusive_ns']<=upper
            histograms.append(dict(batch=current,name=name,stats=stats))
    for c in calibrations:
        assert c['iterations']==20000 and 0<=c['interval_ns']<=c['wall_ns']
        assert c['min_ns']*c['iterations']<=c['interval_ns']<=c['max_ns']*c['iterations']
    return histograms,calibrations

assert supplemental('ordinary output',[])==([],[])

def run(producer,label,mode='off',comparison=False):
    for flag in ['OMEGA_EDGE_CACHE','OMEGA_EDGE_REBIND_PROFILE','OMEGA_PAYLOAD_ENTRY_PROFILE','OMEGA_REBIND_DELTA_COUNTS','OMEGA_PREMISE_PROBE','OMEGA_UNCHANGED_FACT_COUNTS','OMEGA_BORROWED_FACT_COUNTS']:
        os.environ.pop(flag,None)
    if mode=='off':os.environ.pop('OMEGA_REPLAY_SCAN_PROFILE',None)
    else:os.environ['OMEGA_REPLAY_SCAN_PROFILE']=mode
    result=h.module.measure(producer,label,'counts' if mode=='counts' else 'timing',comparison=comparison)
    folder=OUT/label;stderr=(folder/'stderr.txt').read_text();batches=h.parse(stderr)
    histograms,calibrations=supplemental(stderr,batches)
    if mode=='off':assert not batches and not histograms and not calibrations
    else:
        assert all(b['mode']==mode for b in batches)
        main=max(batches,key=lambda b:b['machines']);e=main['entries']
        assert main['machines']==202 and main['summary']['roots']==418 and main['summary']['replays']==173
        for name,calls in [('replay',173),('meet',14270),('retain_snapshot',28540),('cow_shared',14456),('cow_unique',9324),('snapshot_filter',23780)]:assert e[name]['calls']==calls,(name,e[name])
        assert e.get('cow_weak',dict(calls=0))['calls']==0
        outer=max((r for r in result['reconciliation'] if r['operation']=='memberships'),key=lambda r:r['machines'])
        assert main['summary']['wall_ns']<=outer['metrics']['source_contract_queries_ns']
        if mode=='fine':
            assert len(calibrations)==1 and len(histograms)==sum(len(b['entries']) for b in batches)
        else:
            assert not calibrations and not histograms
            assert e['retain_snapshot']['rows_a']==8267548 and e['retain_snapshot']['same']==4760
            assert e['cow_shared']['rows_a']==e['cow_shared']['cloned']==5076178 and e['cow_shared']['detached']==14456
            assert e['snapshot_filter']['rows_a']==7185680 and e['cow_unique']['rows_a']==2109502
            assert e['retain_snapshot']['removed']==311696
            old=json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/raw/scans-counts-a/result.json').read_text())
            def counters(record):return [[r['operation'],r['machines'],{k:v for k,v in r['metrics'].items() if not k.endswith('_ns')}] for r in record['reconciliation']]
            assert counters(result)==counters(old)
    result.update(copy_profile=mode,scans=batches,copy_histograms=histograms,copy_calibrations=calibrations)
    if comparison:
        kept=Path(re.search(r'build dir kept: (.+)',stderr)[1].strip());stdout=(folder/'stdout.txt').read_bytes()
        assert stdout==(kept/'terminal.stdout').read_bytes() and len(stdout)==463
        assert hashlib.sha256((kept/'omega-program.exe').read_bytes()).hexdigest()==h.IMAGE
        (folder/'terminal.stdout').write_bytes(stdout)
        result.update(terminal_stdout_agrees=True,retained_image_sha256=h.IMAGE,guest_stdout_sha256=hashlib.sha256(stdout).hexdigest())
    (folder/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    if batches:
        print('COPY_SUMMARY',label,main['summary'],{k:round(v['inclusive_ns']/1e6,4) for k,v in e.items()},calibrations,flush=True)
    return result

def summarize():
    rows={}
    for producer,label,mode in SCHEDULE:
        r=json.loads((OUT/label/'result.json').read_text())
        outer=max((r for r in r['reconciliation'] if r['operation']=='memberships'),key=lambda r:r['machines'])
        entry=dict(mode=mode,producer=producer,wall_seconds=r['wall_seconds'],cpu_seconds=r['cpu_seconds'],source_replay_ms=outer['metrics']['source_contract_queries_ns']/1e6,
            peak_working_set_bytes=r['peak_working_set_bytes'],peak_commit_bytes=r['peak_commit_bytes'])
        if mode=='fine':
            main=max(r['scans'],key=lambda b:b['machines']);e=main['entries'];cal=r['copy_calibrations'][0]
            entry.update(inclusive_ms={k:v['inclusive_ns']/1e6 for k,v in e.items()},exclusive_ms={k:v['exclusive_ns']/1e6 for k,v in e.items()},
                calibration_interval_ns_per_scope=cal['interval_ns']/cal['iterations'],calibration_gross_ns_per_scope=cal['wall_ns']/cal['iterations'])
            entry['copy_fraction_of_source_percent']=e['cow_shared']['inclusive_ns']/main['summary']['wall_ns']*100
            entry['copy_fraction_of_compile_percent']=e['cow_shared']['inclusive_ns']/1e9/r['wall_seconds']*100
        rows[label]=entry
    (WORK/'summary.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('SUMMARY',json.dumps(rows),flush=True)
    return rows

if __name__=='__main__':
    identities=[]
    for producer in ['payload','handle-cost']:
        folder=OUT/'producers'/producer
        profile=json.loads(subprocess.check_output([str(folder/'omega.exe'),'--build-profile']))
        assert int(profile['opt_level'])>0
        identities.append(dict(producer=producer,compiler_sha256=hashlib.sha256((folder/'omega.exe').read_bytes()).hexdigest(),accepted_lock_sha256=hashlib.sha256((folder/'omega.lock').read_bytes()).hexdigest(),build_profile=profile))
    assert identities[0]==json.loads((ROOT/'wiki/drafts/measurements/proof_parallelism_2026_10_28/scans_joins/identities.json').read_text())[0]
    (WORK/'identities.json').write_text(json.dumps(identities,indent=2)+'\n',encoding='utf-8',newline='\n')
    for case in SCHEDULE:run(*case)
    a=run('handle-cost','handles-counts-a','counts');b=run('handle-cost','handles-counts-b','counts');assert a['scans']==b['scans']
    run('handle-cost','handles-verified',comparison=True)
    summarize()
