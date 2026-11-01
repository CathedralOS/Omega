import subprocess
from pathlib import Path
cli='/home/jarod/development/Omega/.agents/worktree/nov1-coord/build/swarm/nov1/producers/gen-bfcffe6bcfc/omega'
for p in sorted(Path('build/swarm/nov1/probes/result_contract').glob('*/main.omg')):
 r=subprocess.run([cli,'--check',str(p)],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=30)
 p.with_suffix('.check.stdout').write_text(r.stdout);p.with_suffix('.check.stderr').write_text(r.stderr)
 print(p.parent.name,r.returncode,r.stdout+r.stderr,flush=True)
