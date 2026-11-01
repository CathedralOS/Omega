from pathlib import Path
import json,subprocess,sys,time,hashlib
p=Path(__file__).parent
selection=json.loads((p/"case-corpus-selection.json").read_text())
runner=p/"case-bound-corpus-r4-omega"
records=[]
for leg,flags in [("check",[]),("both",["--both"])]:
 command=[sys.executable,"tools/corpus_gate.py","--runner",str(runner),"--record",*flags,"--filter",selection["filter"],"--write-moves",str(p/("case-corpus-r4-"+leg+"-moves.txt")),"--json",str(p/("case-corpus-r4-"+leg+"-gate.json"))]
 start=time.monotonic();r=subprocess.run(command,capture_output=True,stdin=subprocess.DEVNULL)
 for stream in ["stdout","stderr"]:(p/("case-corpus-r4-"+leg+"."+stream)).write_bytes(getattr(r,stream))
 row=dict(leg=leg,command=command,exit=r.returncode,seconds=time.monotonic()-start,fixture_count=selection["count"],source=subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),runner_sha256=hashlib.sha256(runner.read_bytes()).hexdigest())
 records.append(row);(p/"case-corpus-r4-results.json").write_text(json.dumps(records,indent=2)+"\n");print(json.dumps(row),flush=True)
 print((r.stdout+r.stderr).decode(errors="replace")[-2500:],flush=True)
 assert r.returncode==0
