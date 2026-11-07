"""A fail control passes only when its own documented bounds rule rejects."""
import argparse
import json
import subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('compiler')
options=parser.parse_args()
command=[options.compiler,'--check','--target','linux_x86_64',str(ROOT/'controls'/'bounds'/'main.omg')]
observed=subprocess.run(command,capture_output=True,timeout=300)
text=(observed.stdout+observed.stderr).decode(errors='replace')
errors=[line.strip() for line in text.splitlines() if line.strip().startswith('error:')]
expected=(ROOT/'controls'/'bounds'/'expected.txt').read_text().strip()
ok=observed.returncode==1 and len(errors)>=1 and all(expected in error for error in errors)
record={'command':command,'exit':observed.returncode,'diagnostics':text,'correct_rule_rejected':ok}
(ROOT/'build').mkdir(exist_ok=True)
(ROOT/'build'/'control-result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
raise SystemExit(0 if ok else 1)
