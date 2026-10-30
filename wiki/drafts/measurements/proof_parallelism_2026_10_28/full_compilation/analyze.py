from pathlib import Path
import hashlib,importlib.util,json,re
HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('timeline',HERE/'full_profile_run.py')
h=importlib.util.module_from_spec(spec);spec.loader.exec_module(h)

def detail(events,cells,category,selector):
    candidates=[(e,selector(e['name'])) for e in events if selector(e['name'])]
    totals={}
    for cell in cells:
        if cell['category']!=category:continue
        a,b=cell['start'],cell['end']
        relevant=[(e,n) for e,n in candidates if e['start']<b and a<e['end']]
        points=sorted({a,b}|{max(a,e['start']) for e,_ in relevant}|{min(b,e['end']) for e,_ in relevant})
        for left,right in zip(points,points[1:]):
            active=[(e,n) for e,n in relevant if e['start']<=left and right<=e['end']]
            leaves=[(e,n) for e,n in active if not any(e['start']<=o['start'] and o['end']<=e['end'] and (e['start'],e['end'])!=(o['start'],o['end']) for o,_ in active)]
            names=sorted({n for _,n in leaves});name=' + '.join(names) if names else 'untraced remainder'
            totals[name]=totals.get(name,0)+right-left
    assert sum(totals.values())==sum(c['end']-c['start'] for c in cells if c['category']==category)
    return totals

CHECK=['05.c.prepare_typed_trees','05.d.2.prepare_typed_program','05.d.3.build_check_facts','05.d.4_6.quotients_borrows_identities','05.d.7.check_checked_facts','05.d.8_10.finalize_execution_identities','05.d.11_13.selections_custody_types_boundaries']
SELECTORS={
 'source and frontend preparation':lambda n:n if n.startswith('frontend.') else None,
 'typed preparation and checking':lambda n:n if n in CHECK else None,
 'Terminal lowering':lambda n:n if n in ['05.01_lower_selected_machine','05.10_reconcile_and_residences'] else None,
 'retained graph persistence':lambda n:n if n in ['cache.encode.preparation','cache.encode.sources','cache.encode.checks','cache.stage_hash_and_write','cache.prune'] else None,
 'package orchestration and cleanup':lambda n:n if n.startswith('orchestration.') else None,
}

def self_check():
    e=[dict(name='outer',start=5,end=25),dict(name='inner',start=12,end=18)]
    c=[dict(start=10,end=20,category='x')]
    assert detail(e,c,'x',lambda n:n)=={'outer':4,'inner':6}
    assert detail([],c,'x',lambda n:n)=={'untraced remainder':10}
self_check()

def analyze():
    for identity_name,patch_name in [('source-identity.json','instrumentation.patch'),('coarse-source-identity.json','coarse.patch')]:
        identity=json.loads((HERE/identity_name).read_text())
        assert hashlib.sha256((HERE/patch_name).read_bytes()).hexdigest()==identity['patch_sha256']
    identities=json.loads((HERE/'identities.json').read_text())+[json.loads((HERE/'refined-identity.json').read_text())]
    identities={x['producer']:x for x in identities}
    for x in identities.values():assert int(x['build_profile']['opt_level'])>0
    summary={}
    for folder in sorted((HERE/'raw').iterdir()):
        r=json.loads((folder/'result.json').read_text());stderr=(folder/'stderr.txt').read_text()
        assert r['exit_code']==0 and r['wall_seconds']>0 and r['cpu_seconds']>0
        identity=identities[r['producer']]
        for key in ['compiler_sha256','accepted_lock_sha256']:assert r[key]==identity[key],(folder,key)
        assert r['program_sha256']=='b98c24db839027800ada039e525f355e80f4cccaf22079bfb68d8a6b48105077'
        if (folder/'terminal.stdout').exists():
            assert r['terminal_stdout_agrees'] and r['retained_image_sha256']==h.IMAGE
            assert (folder/'stdout.txt').read_bytes()==(folder/'terminal.stdout').read_bytes()
            stdout=(folder/'stdout.txt').read_bytes()
        else:
            assert r['image_sha256']==h.IMAGE
            runtime=json.loads((folder/'runtime/result.json').read_text());assert runtime['exit_code']==0
            stdout=(folder/'runtime/stdout.txt').read_bytes()
        assert len(stdout)==463 and b'A bottomless dark room near the dungeon heart.' in stdout
        events,counts=h.parse(stderr)
        result={k:r[k] for k in ['producer','wall_seconds','cpu_seconds','cache_mode','phase_profile']}
        if events:
            assert r['full_profile'] and events==r['full_events'] and counts==r['full_counts']
            p=h.partition(events);assert p==r['full_partition']
            assert not any(n.startswith('overlap:') for n in p['disjoint_ns'])
            assert p['cli_ns']<=r['wall_seconds']*1e9
            result.update(cli_ns=p['cli_ns'],disjoint_ns=p['disjoint_ns'],outside_cli_seconds=r['wall_seconds']-p['cli_ns']/1e9)
            result['detail_ns']={cat:detail(events,p['cells'],cat,sel) for cat,sel in SELECTORS.items()}
            counter={c['name']:sum(x['amount'] for x in counts if x['name']==c['name']) for c in counts}
            result['counts']=counter
            if 'warm' in folder.name:
                assert r['warm_graph_loaded'] and counter['source.graph.loaded']==1 and counter['source.graph.prepared_hits']==2
                assert 'source.graph.retained' not in counter and 'retained graph persistence' not in p['disjoint_ns']
            else:
                assert counter['source.graph.cold']==1 and counter['source.graph.retained']==1
                assert sum(v for k,v in counter.items() if k.endswith('.bytes'))==230268241
        else:assert not r.get('full_profile',False)
        summary[folder.name]=result
    assert len(summary)==22
    assert sum((HERE/'raw'/n/'terminal.stdout').is_file() for n in summary)==2
    return summary

if __name__=='__main__':
    manifest=json.loads((HERE/'manifest.json').read_text())
    actual={p.relative_to(HERE).as_posix() for p in HERE.rglob('*') if p.is_file() and p.name!='manifest.json' and '__pycache__' not in p.parts}
    assert actual==set(manifest)
    for name,digest in manifest.items():assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
    summary=analyze();assert summary==json.loads((HERE/'summary.json').read_text())
    for name in ['full-refined-a','full-refined-b','full-refined-c','full-refined-warm-a','full-refined-warm-b']:
        r=summary[name];print(name,round(r['wall_seconds'],3),'s')
        for cat,items in r['detail_ns'].items():
            print(' ',cat,{k:round(v/1e9,4) for k,v in items.items()})
    print('Verified',len(manifest),'file digests and all 22 observations; disjoint CLI and child partitions, producer identities, artifact/oracle and both-engine agreement.')
