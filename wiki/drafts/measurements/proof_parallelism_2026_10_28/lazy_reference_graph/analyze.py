from pathlib import Path
import gzip,hashlib,importlib.util,json
HERE=Path(__file__).resolve().parent
def module(name):
    spec=importlib.util.spec_from_file_location(name,HERE/(name+'.py'));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
pure=module('lazy_reference_run');counts=module('lazy_reference_counts')
def read(folder,name):
    p=folder/name
    return p.read_bytes() if p.exists() else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())
def observations(report):
    return [{k:r[k] for k in ['fixture','tier','status','expected_satisfied','facts','diagnostics']} for r in report['observations']]

def analyze():
    identities=json.loads((HERE/'identities.json').read_text())+json.loads((HERE/'counter-identities.json').read_text())
    identities={x['producer']:x for x in identities}
    assert len(identities)==4 and all(int(x['build_profile']['opt_level'])>0 for x in identities.values())
    for identity_name,patch_name in [('candidate-source.json','candidate.patch'),('diagnostic-source.json','diagnostic.patch')]:
        identity=json.loads((HERE/identity_name).read_text());assert hashlib.sha256((HERE/patch_name).read_bytes()).hexdigest()==identity['patch_sha256']
    rows={};stdout_hashes=set();engine_checks=0;ordinary=0
    for folder in sorted((HERE/'raw').iterdir()):
        r=json.loads((folder/'result.json').read_text());assert r['exit_code']==0 and r['wall_seconds']>0 and r['cpu_seconds']>0
        for key in ['compiler_sha256','accepted_lock_sha256']:assert r[key]==identities[r['producer']][key],(folder,key)
        assert r['program_sha256']=='b98c24db839027800ada039e525f355e80f4cccaf22079bfb68d8a6b48105077'
        stderr=read(folder,'stderr.txt').decode('utf-8')
        if (folder/'terminal.stdout').is_file():
            engine_checks+=1;assert r['terminal_stdout_agrees'] and r['retained_image_sha256']==pure.IMAGE
            stdout=read(folder,'stdout.txt');assert stdout==(folder/'terminal.stdout').read_bytes()
        else:
            ordinary+=1;assert r['image_sha256']==pure.IMAGE
            runtime=json.loads((folder/'runtime/result.json').read_text());assert runtime['exit_code']==0
            stdout=read(folder/'runtime','stdout.txt')
        assert len(stdout)==463 and b'A bottomless dark room near the dungeon heart.' in stdout
        stdout_hashes.add(hashlib.sha256(stdout).hexdigest())
        if 'graph_counts' in r:
            c=counts.parse(stderr);assert c==r['graph_counts']
            if 'verified' in folder.name:assert c==dict(prepared_calls=[],graph_builds=[])
            elif r['producer']=='terminal-breakdown':assert c['graph_builds']==[[202,202]]*3 and not c['prepared_calls']
            else:assert c['prepared_calls']==[202]*4 and not c['graph_builds']
        else:
            assert r['producer'] in pure.PRODUCERS.values()
            assert not any(line.startswith(('RECONCILE\t','REFERENCE_','SOURCE_QUERY_','FULL\t','LAZY_REFERENCE_')) for line in stderr.splitlines())
        if r.get('warm_cache_from'):
            seed=json.loads((HERE/'raw'/r['warm_cache_from']/'result.json').read_text());assert seed['producer']==r['producer'] and seed['compiler_sha256']==r['compiler_sha256']
        rows[folder.name]=r
    assert len(rows)==31 and ordinary==28 and engine_checks==3 and len(stdout_hashes)==1
    summary=pure.summarize(HERE/'raw');assert summary==json.loads((HERE/'summary.json').read_text())
    assert summary['warm']['wall_wins']==6 and summary['warm']['cpu_wins']==6
    assert summary['cold']['wall_wins']==4 and not summary['promotion_supported']
    quality=json.loads((HERE/'quality-summary.json').read_text());assert quality['baseline_candidate_equal'] and not quality['golden_recorded']
    for leg,total in [('positive',4),('negative',3)]:
        candidate=json.loads((HERE/('quality-'+leg+'.json')).read_text());baseline=json.loads((HERE/('baseline-quality-'+leg+'.json')).read_text())
        assert observations(candidate)==observations(baseline) and len(candidate['observations'])==total
        for report in [candidate,baseline]:assert not report['oracle_failures'] and not report['infrastructure_error'] and report['exit_code']==1
        if leg=='positive':
            assert all(o['status']=='built' and o['facts'].get('exit')=='0' and o['facts'].get('interpreter-exit')=='0' and o['facts'].get('interpreter')=='agrees' for o in candidate['observations'])
        else:assert all(o['status']=='rejected' and o['expected_satisfied'] for o in candidate['observations'])
    for name in ['corpus-identity.json','baseline-corpus-identity.json']:
        assert int(json.loads((HERE/name).read_text())['build_profile']['opt_level'])>0
    return summary

if __name__=='__main__':
    manifest=json.loads((HERE/'manifest.json').read_text());actual={p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts}
    assert set(manifest)==actual
    for name,digest in manifest.items():assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
    summary=analyze()
    for mode in ['cold','warm']:print(mode,{k:v for k,v in summary[mode].items() if k!='pairs'})
    print('Verified',len(manifest),'file digests,31 observations,28 native runs,3 engine comparisons,graph build counts,balanced pairs and exact baseline-equivalent focused corpus outcomes. Promotion:',summary['promotion_supported'])
