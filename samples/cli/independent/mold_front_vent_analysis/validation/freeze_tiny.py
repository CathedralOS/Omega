"""Materialize expectations from the already frozen reference before runtime runs."""
import hashlib
import json
from reference import ROOT,tiny_inputs,parse,simulate,render
folder=ROOT/'build/tiny-fixtures'
folder.mkdir(parents=True,exist_ok=True)
rows=[]
for number,text in enumerate(tiny_inputs()):
    label=f'tiny_{number:03d}'
    expected=render(simulate(*parse(text))).encode()
    (folder/f'{label}.in').write_text(text)
    (folder/f'{label}.expected').write_bytes(expected)
    rows.append(dict(name=label,input_sha256=hashlib.sha256(text.encode()).hexdigest(),
                     stdout_sha256=hashlib.sha256(expected).hexdigest()))
(folder/'manifest.json').write_text(json.dumps(rows,indent=2)+'\n')
print(f'Materialized {len(rows)} expected reports from frozen reference')
