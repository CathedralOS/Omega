from pathlib import Path
import json,os,subprocess,sys
p=Path(__file__).parent
producer="case-bound-applied-r3"
records=[]
def run(name,root,stage,arguments,expected_exit,markers):
 label="case-controls-r3-"+name+"-"+stage
 env=dict(os.environ,VALIDATION_PROJECT=str(root),VALIDATION_PRODUCER=producer)
 subprocess.run([sys.executable,str(p/"run_cli.py"),label,*arguments],env=env,stdout=subprocess.DEVNULL,check=True)
 x=json.loads((p/(label+".json")).read_text());text=Path(x["stdout"]).read_text()+Path(x["stderr"]).read_text()
 expected=x["exit"]==expected_exit and all(m in text for m in markers)
 row=dict(name=name,stage=stage,exit=x["exit"],seconds=x["seconds"],expected=expected,markers=markers,record=str(p/(label+".json")))
 records.append(row);(p/"case-controls-r3-results.json").write_text(json.dumps(records,indent=2)+"\n");print(json.dumps(row),flush=True)
 if not expected:print(text[-3500:],flush=True)
 return expected
base=Path("samples/cli/facts/case_dependent_default_domain/controls")
for name,marker in [("missing_type_equation","match arms produce incompatible values"),("other_default_argument","cannot prove requires contract for call width_range"),("stale_write","default domain is still FALSE")]:
 root=base/name;run(name,root,"reject",["--check",str(root/"main.omg")],1,[marker])
root=Path("tests/omega/fail/dependent/case_where_payload_write_exit_open")
run("case_write",root,"reject",["--check",str(root/"main.omg")],1,["default domain is still FALSE"])
for name,root,expected_exit in [("restoration",base/"restoration",0),("zero_case",Path("tests/omega/pass/dependent/zero_case_predicate_entry_exit"),65)]:
 if run(name,root,"update",["update","--project",str(root),"--target","macos_arm64","--offline"],0,[]):
  run(name,root,"native",["run","--both",str(root/"main.omg")],expected_exit,["native exit "+str(expected_exit),"interp exit: "+str(expected_exit)])
assert all(r["expected"] for r in records)
