from pathlib import Path
import hashlib,importlib.util,json
ROOT=Path(__file__).resolve().parent
REPO=next(p for p in ROOT.parents if (p/'AGENTS.md').is_file())
def load(p):return json.loads(p.read_text())
def main():
    manifest=load(ROOT/'manifest.json')
    for name,digest in manifest.items():assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name
    source=load(ROOT/'source-identity.json');assert hashlib.sha256((ROOT/'instrumentation.patch').read_bytes()).hexdigest()==source['patch_sha256']
    patch=load(ROOT/'patch-check.json');assert patch['base']==source['base'] and patch['isolated_index'] and patch['exit_code']==0
    spec=importlib.util.spec_from_file_location('copy_runner',ROOT/'handle_cost_run.py');h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)
    identities={r['producer']:r for r in load(ROOT/'identities.json')};assert all(int(r['build_profile']['opt_level'])>0 for r in identities.values())
    expected=[label for _,label,_ in h.SCHEDULE]+['handles-counts-a','handles-counts-b','handles-verified']
    assert sorted(p.name for p in (ROOT/'raw').iterdir())==sorted(expected)
    counts=[];programs=set();timed={};controls={}
    summary=load(ROOT/'summary.json')
    for label in expected:
        folder=ROOT/'raw'/label;r=load(folder/'result.json');assert r['exit_code']==0
        programs.add(r['program_sha256']);identity=identities[r['producer']]
        assert r['compiler_sha256']==identity['compiler_sha256'] and r['accepted_lock_sha256']==identity['accepted_lock_sha256']
        stderr=(folder/'stderr.txt').read_text();batches=h.h.parse(stderr);hist,cal=h.supplemental(stderr,batches)
        assert (batches,hist,cal)==(r['scans'],r['copy_histograms'],r['copy_calibrations'])
        assert not r['phase_profile']
        if label=='handles-verified':
            assert r['terminal_stdout_agrees'] and r['retained_image_sha256']==h.h.IMAGE
            output=(folder/'stdout.txt').read_bytes();assert output==(folder/'terminal.stdout').read_bytes() and len(output)==463
            assert hashlib.sha256(output).hexdigest()==r['guest_stdout_sha256']
        else:
            assert r['cache_mode']=='fresh private cache' and r['image_sha256']==h.h.IMAGE
            assert load(folder/'runtime/result.json')['exit_code']==0
            assert b'A bottomless dark room near the dungeon heart.' in (folder/'runtime/stdout.txt').read_bytes()
        if r['copy_profile']=='off':assert not batches and not hist and not cal
        else:
            main_batch=max(batches,key=lambda b:b['machines']);e=main_batch['entries']
            assert (main_batch['machines'],main_batch['summary']['roots'],main_batch['summary']['replays'])==(202,418,173)
            for name,calls in [('replay',173),('meet',14270),('retain_snapshot',28540),('cow_shared',14456),('cow_unique',9324),('snapshot_filter',23780)]:assert e[name]['calls']==calls
            assert e.get('cow_weak',dict(calls=0))['calls']==0
            if r['copy_profile']=='counts':
                assert not hist and not cal;counts.append(batches)
                assert e['cow_shared']['rows_a']==e['cow_shared']['cloned']==5076178 and e['cow_shared']['detached']==14456
                assert e['cow_unique']['rows_a']==2109502 and e['cow_unique']['cloned']==0
                assert e['snapshot_filter']['rows_a']==7185680
                assert e['retain_snapshot']['rows_a']==8267548 and e['retain_snapshot']['same']==4760 and e['retain_snapshot']['removed']==311696
            else:
                assert len(cal)==1 and len(hist)==sum(len(b['entries']) for b in batches)
                s=summary[label];assert s['inclusive_ms']=={k:v['inclusive_ns']/1e6 for k,v in e.items()}
                assert s['exclusive_ms']=={k:v['exclusive_ns']/1e6 for k,v in e.items()}
                assert s['wall_seconds']==r['wall_seconds'] and s['cpu_seconds']==r['cpu_seconds']
                shared=e['cow_shared'];hist_row=next(row['stats'] for row in hist if row['batch']==batches.index(main_batch) and row['name']=='cow_shared')
                timed[label]=dict(shared_cow_ms=shared['inclusive_ns']/1e6,unique_make_mut_ms=e['cow_unique']['inclusive_ns']/1e6,
                    filter_ms=e['snapshot_filter']['inclusive_ns']/1e6,snapshot_self_ms=e['retain_snapshot']['exclusive_ns']/1e6,
                    meet_self_ms=e['meet']['exclusive_ns']/1e6,source_ms=main_batch['summary']['wall_ns']/1e6,
                    copy_mean_ns=shared['inclusive_ns']/shared['calls'],copy_min_ns=hist_row['min_ns'],copy_max_ns=hist_row['max_ns'],
                    calibration_interval_ns=cal[0]['interval_ns']/cal[0]['iterations'],calibration_gross_ns=cal[0]['wall_ns']/cal[0]['iterations'],
                    clock_reference_for_shared_calls_ms=cal[0]['interval_ns']/cal[0]['iterations']*shared['calls']/1e6,
                    source_fraction_percent=s['copy_fraction_of_source_percent'],compile_fraction_percent=s['copy_fraction_of_compile_percent'])
        if label in summary:
            s=summary[label]
            outer=max((v for v in r['reconciliation'] if v['operation']=='memberships'),key=lambda v:v['machines'])
            assert s['source_replay_ms']==outer['metrics']['source_contract_queries_ns']/1e6
            assert s['wall_seconds']==r['wall_seconds'] and s['cpu_seconds']==r['cpu_seconds']
            controls[label]=dict(wall_seconds=r['wall_seconds'],cpu_seconds=r['cpu_seconds'],source_ms=s['source_replay_ms'])
    assert len(counts)==2 and counts[0]==counts[1] and len(programs)==1 and len(timed)==3
    print(json.dumps(dict(observations=len(expected),manifest_entries=len(manifest),timed=timed,controls=controls),indent=2))
if __name__=='__main__':main()
