"""Run supported public Terminal comparison sequentially and pin native stdout.
The CLI compares process termination, not Terminal stdout; record that boundary.
"""
import argparse
import json
import subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('compiler')
parser.add_argument('--output',type=Path,default=ROOT/'build'/'terminal-results.json')
options=parser.parse_args()
manifest=json.loads((ROOT/'validation'/'frozen.json').read_text())
results=[]
for name in ('founder_ambiguous','shared_ancestry','impossible_trio','invalid_self_parent'):
    case=manifest['cases'][name]
    command=[options.compiler,'run','--both','--keep',str(ROOT/'main.omg')]
    observed=subprocess.run(command,input=(ROOT/'fixtures'/f'{name}.stdin').read_bytes(),capture_output=True,timeout=300)
    expected=(ROOT/'fixtures'/f'{name}.stdout').read_bytes()
    stderr=observed.stderr.decode(errors='replace')
    agreement=f'interp exit: {case["exit"]}' in stderr
    declined='interp: DECLINED' in stderr
    output_matches=observed.stdout==expected and observed.returncode==case['exit']
    results.append({'case':name,'command':command,'exit':observed.returncode,'native_oracle_matches':output_matches,
                    'terminal_exit_agrees':agreement,'declined':declined,'stderr':stderr})
    print(json.dumps(results[-1]))
options.output.parent.mkdir(parents=True,exist_ok=True)
options.output.write_text(json.dumps({'terminal_scope':'Public CLI compares termination only; frozen full stdout checks concern native output.','results':results},indent=2)+'\n')
raise SystemExit(0 if all(item['native_oracle_matches'] and item['terminal_exit_agrees'] for item in results) else 1)
