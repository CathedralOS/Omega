from pathlib import Path
import gzip,hashlib,importlib.util,json
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('timing',HERE/'terminal_breakdown_run.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)

def stream(folder,name):
    p=folder/name
    return p.read_bytes() if p.exists() else gzip.decompress(p.with_suffix(p.suffix+'.gz').read_bytes())

def analyze():
    identities={i['producer']:i for i in json.loads((HERE/'identities.json').read_text())}
    for i in identities.values():assert int(i['build_profile']['opt_level'])>0
    for identity_name,patch_name in [('source-identity.json','instrumentation.patch'),('timing-source-identity.json','timing.patch')]:
        identity=json.loads((HERE/identity_name).read_text())
        assert hashlib.sha256((HERE/patch_name).read_bytes()).hexdigest()==identity['patch_sha256']
    summary={};count_rosters=[]
    for folder in sorted((HERE/'raw').iterdir()):
        r=json.loads((folder/'result.json').read_text());assert r['exit_code']==0
        assert r['wall_seconds']>0 and r['cpu_seconds']>0
        for key in ['compiler_sha256','accepted_lock_sha256']:assert r[key]==identities[r['producer']][key]
        assert r['program_sha256']=='b98c24db839027800ada039e525f355e80f4cccaf22079bfb68d8a6b48105077'
        if (folder/'terminal.stdout').is_file():
            assert r['terminal_stdout_agrees'] and r['retained_image_sha256']==h.IMAGE
            stdout=stream(folder,'stdout.txt');assert stdout==(folder/'terminal.stdout').read_bytes()
        else:
            assert r['image_sha256']==h.IMAGE
            runtime=json.loads((folder/'runtime/result.json').read_text());assert runtime['exit_code']==0
            stdout=stream(folder/'runtime','stdout.txt')
        assert len(stdout)==463 and b'A bottomless dark room near the dungeon heart.' in stdout
        p=h.parse(stream(folder,'stderr.txt').decode('utf-8'));assert p==r['attribution']
        result={k:r[k] for k in ['producer','wall_seconds','cpu_seconds','peak_working_set_bytes','cache_mode','reconciliation_mode','entry_detail']}
        if r['reconciliation_mode']!='off':
            assert p['windows']==({'REFERENCE':418,'SOURCE':418} if r['entry_detail'] else {'REFERENCE':418})
            assert p['cache']==({'loaded':1,'prepared_hit':2} if 'warm' in folder.name else {'cold':1})
            m=p['membership_parts'];result.update(totals_ns=p['totals_ns'],parts=m,graphs=p['graphs'],cache=p['cache'],replay_details=p['replay_details'])
            assert len(p['graphs'])==3
            assert all(g['scope']=='REFERENCE' and g['module_machines']==g['graph_machines']==202 for g in p['graphs'])
            if 'demands' in p:
                result['demands']=p['demands']
                assert p['demands']==[dict(scope='REFERENCE',mappings=1,shared_leaves=0,projected_candidates=0)]*3
            disjoint={k:v for k,v in m.items() if k.endswith('_ns') and k not in ['reference_graph_ns','reference_other_ns']}
            assert sum(disjoint.values())==p['totals_ns']['memberships']
            assert m['reference_graph_ns']+m['reference_other_ns']==m['reference_queries_ns']
            if r['entry_detail']:
                assert not p['replay_details']['REFERENCE']
                assert p['replay_details']['SOURCE']['queries']==173
                assert p['replay_details']['SOURCE']['ENTRY_INIT_calls']==96
                assert p['replay_details']['SOURCE']['steps']==15219 and p['replay_details']['SOURCE']['edges']==17444
            if r['reconciliation_mode']=='counts':
                roster=[(row['operation'],row['machines'],{k:v for k,v in row['metrics'].items() if not k.endswith('_ns')}) for row in p['rows']]
                count_rosters.append(roster)
                for row in p['rows']:
                    c=row['metrics']
                    if row['operation']=='memberships' and c:
                        assert c['fixed_point_rounds']==c['candidate_machines']+c.get('changing_rounds',0)
                        assert c['fixed_point_rounds']==c['reference_context_constructions']
                        assert c['reference_queries']==c['eligible_operations']
        else:assert not p['rows'] and not p['graphs'] and not p['cache']
        summary[folder.name]=result
    assert len(summary)==16 and len(count_rosters)==4 and all(r==count_rosters[0] for r in count_rosters)
    assert sum((HERE/'raw'/name/'terminal.stdout').is_file() for name in summary)==2
    return summary

if __name__=='__main__':
    manifest=json.loads((HERE/'manifest.json').read_text())
    actual={p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts}
    assert actual==set(manifest)
    for name,digest in manifest.items():assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
    summary=analyze();assert summary==json.loads((HERE/'summary.json').read_text())
    for name,r in summary.items():
        if r['reconciliation_mode']=='timing':
            print(name,round(r['wall_seconds'],3),'s', {k:round(v/1e6,3) for k,v in r['parts'].items() if k.endswith('_ns')})
    print('Verified',len(manifest),'file digests, 16 observations, exact nested/disjoint accounting, source/producer identity, cold/warm markers, native outputs and engine agreement.')
