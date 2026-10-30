import os,subprocess,sys,shutil,json
from pathlib import Path
root=Path.cwd();out=root/'build/flow-evaluation';record=root/'wiki/drafts/measurements/flow_evaluation_2026_10_29';scope=root/'omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/flow'
paths=[scope/'expression.rs',scope/'exits.rs',scope/'exits/guards.rs',scope/'expression/dispatch_copies.rs'];original={p:p.read_bytes() for p in paths}
os.environ['MBX_CACHE_DIR']=str(root/'build/conjunction-flow/recovery-cache');os.environ['MBX_LEARNED_INCREMENTAL']='0'
patch=subprocess.check_output(['git','diff','--','omega-rust']);(record/'profiler.patch').write_bytes(patch)
def run(name,command):
    print(name,flush=True)
    with (out/(name+'.log')).open('w',encoding='utf-8') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
def build(name):
    run(name,['mbx','build','-p','omega','--locked','--target-dir','build/pass-producer'])
    (out/name).mkdir(exist_ok=True);shutil.copyfile(root/'build/pass-producer/debug/omega.exe',out/name/'omega.exe')
run('unit-tests',['mbx','nextest','run','-p','phase-profile','--locked','--target-dir','build/pass-producer'])
try:
    targets={paths[0]:['new','expression_result','evaluated_operand_as','capture_operand','operand_is_view','capture_place_operand','changed_operand_sources','filter_captured_sources','retain_observation_result'],paths[1]:['append_state_exit_facts'],paths[2]:['append_guard_context'],paths[3]:['dispatch_copy_value']}
    for p,names in targets.items():
        text=original[p].decode()
        for name in names:
            token='fn '+name+'(';assert text.count(token)==1,(p,name)
            at=text.index('{',text.index(token))+1
            text=text[:at]+'\n        let _detail_guard = phase_profile::guard("05.flow_detail.'+name+'");'+text[at:]
        p.write_text(text,encoding='utf-8',newline='')
    (record/'diagnostic.patch').write_bytes(subprocess.check_output(['git','diff','--',*[str(p) for p in paths]]))
    build('indexed-diagnostic')
finally:
    for p,s in original.items():p.write_bytes(s)
assert subprocess.check_output(['git','diff','--','omega-rust'])==patch
run('indexed-screen',[sys.executable,'wiki/drafts/measurements/spill_loop_2026_10_28/ablate.py','build/flow-evaluation/diagnostic/omega.exe','build/flow-evaluation/indexed-diagnostic/omega.exe','build/flow-evaluation/indexed-screen'])
rows=json.loads((out/'indexed-screen/runs.json').read_text());assert len({r['artifact_sha256'] for r in rows})==1
build('candidate')
assert subprocess.check_output(['git','diff','--','omega-rust'])==patch
run('comparison',[sys.executable,'wiki/drafts/measurements/checking_simplification_2026_10_28/compare.py','build/conjunction-flow/candidate/omega.exe','build/flow-evaluation/candidate/omega.exe','build/flow-evaluation/comparison'])
print('All local profiler checks completed.',flush=True)
