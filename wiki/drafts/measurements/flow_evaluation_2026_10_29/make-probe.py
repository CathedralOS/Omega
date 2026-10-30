import json,re,subprocess
from pathlib import Path
out=Path('build/flow-evaluation');p=Path('omega-rust/psi/foundation/phase-profile/src/lib.rs')
(out/'phase-baseline.rs').write_bytes(subprocess.check_output(['git','show','HEAD:'+p.as_posix()]))
r=json.loads((out/'diagnostic-screen/runs.json').read_text())[1]
rows=[]
for key in r['stages']:
    m=re.fullmatch(r'\s*(\d+) x  (.+)',key)
    if m:rows.append((m.group(2),int(m.group(1))))
assert len(rows)>100,len(rows)
(out/'probe-roster.json').write_text(json.dumps(rows,indent=2))
source='''#[path = "phase-baseline.rs"]\nmod phase_profile;\nfn main() {\nlet roster: &[(&str,u64)] = &[\n'''+''.join('('+json.dumps(n)+','+str(c)+'),\n' for n,c in rows)+'''];
for (name, _) in roster {phase_profile::count(name,0);}
let started=std::time::Instant::now();
for _ in 0..10 {for (name,count) in roster {for _ in 0..*count {phase_profile::count(std::hint::black_box(name),1);}}}
let elapsed=started.elapsed().as_secs_f64()*1000.0;
let rows=phase_profile::snapshot();
assert_eq!(rows.len(),roster.len());
for (name,count) in roster {assert_eq!(rows.iter().find(|r|r.name==*name).unwrap().count,count*10);}
println!("ms={elapsed} events={} rows={}",roster.iter().map(|(_,count)|count).sum::<u64>()*10,rows.len());
}
'''
(out/'probe-baseline.rs').write_text(source)
subprocess.run(['rustc','--edition','2024','--crate-name','profile_probe','-C','opt-level=1','-A','dead_code',str(out/'probe-baseline.rs'),'-o',str(out/'probe-baseline.exe')],check=True)

import os,hashlib
(out/'phase-candidate.rs').write_bytes(p.read_bytes())
(out/'probe-candidate.rs').write_text(source.replace('phase-baseline.rs','phase-candidate.rs'))
subprocess.run(['rustc','--edition','2024','--crate-name','profile_probe','-C','opt-level=1','-A','dead_code',str(out/'probe-candidate.rs'),'-o',str(out/'probe-candidate.exe')],check=True)
env=os.environ.copy();env['OMEGA_PHASE_PROFILE']='1';results=[]
for index,variant in enumerate('ABBAAB'):
    path=out/('probe-baseline.exe' if variant=='A' else 'probe-candidate.exe')
    result=subprocess.run([str(path)],env=env,capture_output=True,text=True,check=True)
    results.append({'index':index,'variant':variant,'output':result.stdout,'sha256':hashlib.sha256(path.read_bytes()).hexdigest()})
    print(variant,result.stdout,flush=True)
(out/'probe-runs.json').write_text(json.dumps(results,indent=2))
