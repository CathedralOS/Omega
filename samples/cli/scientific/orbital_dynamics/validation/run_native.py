"""Observe one already produced native executable; never alter package decisions."""
import json,subprocess,sys
from pathlib import Path
from check_output import ROOT,directory,frozen
executable=Path(sys.argv[1]).resolve(); output=Path(sys.argv[2]).resolve();output.mkdir(parents=True,exist_ok=True)
records='case\texit\n'
for name in sorted(frozen()['cases']):
    observed=subprocess.run([str(executable)],input=(ROOT/'data'/(name+'.stdin')).read_bytes(),capture_output=True,timeout=30)
    (output/(name+'.stdout')).write_bytes(observed.stdout);(output/(name+'.stderr')).write_bytes(observed.stderr)
    records+=f'{name}\t{observed.returncode}\n';(output/'records.tsv').write_text(records)
    print(name,observed.returncode,flush=True)
directory(output)
