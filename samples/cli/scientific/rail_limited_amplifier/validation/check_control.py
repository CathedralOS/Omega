#!/usr/bin/env python3
"""Require the separate complete application's intended bounds rejection."""
from pathlib import Path
import json, subprocess, sys
root=Path(__file__).resolve().parents[1]
output=root/'build/evidence'
output.mkdir(parents=True,exist_ok=True)
command=[str(Path(sys.argv[1]).resolve()),'--check',str(root/'controls/invalid_bounds/main.omg')]
result=subprocess.run(command,capture_output=True,text=True,timeout=180)
text=result.stdout+result.stderr
(output/'invalid-bounds.log').write_text(text)
(output/'invalid-bounds-result.json').write_text(json.dumps({'command':command,'exit':result.returncode,'diagnostic':text},indent=2)+'\n')
errors=[line for line in text.splitlines() if 'error:' in line]
assert result.returncode==1, f'control must reject, got {result.returncode}'
assert len(errors)==1 and 'cannot prove index' in errors[0], f'wrong-reason or additional rejection: {text}'
assert 'package acceptance' not in text and 'cache custody' not in text, f'control not checked: {text}'
print('complete invalid_bounds control rejects only for intended index proof failure')
