"""Exact normalization equality, native output, dual engines and invalid controls."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile, parse_timings, require_optimized, run_measured, TIMING_LINE
output=Path('build/resolved-normalization')
mode=sys.argv[1]
assert mode in ('candidate','instrumented')
binary=(output/('omega-'+mode)).resolve()
baseline=Path('build/resolution-measurement/omega-baseline').resolve()
for path in (binary,baseline): require_optimized(path)
subjects=[
 ('length','samples/cli/facts/length/local/main.omg',True),
 ('dyn','samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg',True),
 ('record','samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg',True),
 ('returned','tests/omega/pass/layouts/placed_reference_results/returned/main.omg',True),
 ('forwarded','tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg',True),
 ('wrong_backing','tests/omega/fail/layouts/placed_reference_results/wrong_backing/main.omg',False),
 ('overlapping_loans','tests/omega/fail/layouts/placed_reference_results/overlapping_loans/main.omg',False),
 ('exact_policy_owner','tests/omega/fail/layouts/placed_policy_requires_exact_attached_owner/main.omg',False),
 ('result_authority','tests/omega/fail/layouts/placed_factory_result_cannot_upgrade_source/main.omg',False),
 ('length_invalid','samples/cli/facts/length/local/invalid/main.omg',False),
]
def digest(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def clean(text):
    return '\n'.join(line for line in text.splitlines() if not TIMING_LINE.match(line) and not line.startswith('NORMALIZATION\t'))
inputs={str(path):digest(path) for _,subject,_ in subjects for path in Path(subject).parent.rglob('*.omg')}
records=[]
metadata=dict(base=(output/'base.txt').read_text().strip(),mode=mode,started_utc=datetime.now(timezone.utc).isoformat(),inputs=inputs,
    candidate_patch_sha256=digest(output/'candidate.patch'),library_tree=subprocess.check_output(['git','rev-parse','HEAD:source/library'],text=True).strip(),
    binaries={str(path):digest(path) for path in (binary,baseline)})
def save(): (output/(mode+'-observations.json')).write_text(json.dumps(dict(metadata=metadata,records=records),indent=2)+'\n')
for index,(name,subject,positive) in enumerate(subjects):
    order=[('baseline',baseline),(mode,binary)] if index%2==0 else [(mode,binary),('baseline',baseline)]
    for producer,path in order:
        if mode=='instrumented' and producer=='baseline': continue
        if producer=='instrumented': os.environ['OMEGA_NORMALIZATION_VERIFY']='1'
        else: os.environ.pop('OMEGA_NORMALIZATION_VERIFY',None)
        with tempfile.TemporaryDirectory(prefix='omega-normalization-') as temporary:
            command=[str(path),'--timings','--target','macos_arm64','--build-dir',temporary,str(Path(subject).resolve())]
            code,wall,rss,text=capture_compile(command,Path.cwd())
            stages,published=parse_timings(text)
            row=dict(subject=name,path=subject,producer=producer,positive=positive,command=command,exit=code,wall_ms=wall,rss=rss,load=os.getloadavg(),timings=stages,output=text)
            records.append(row);save()
            assert (code==0)==positive, row
            if producer=='instrumented':
                row['normalizations']=[dict(operation=parts[1],removed_clone_rows=int(parts[2]),equal=parts[3]=='equal') for line in text.splitlines() if line.startswith('NORMALIZATION\t') for parts in [line.split('\t')]]
                assert row['normalizations'] and all(x['equal'] for x in row['normalizations']),row
            if positive:
                assert published,row
                artifact=Path(published)
                row['native_sha256']=digest(artifact)
                row['native_exit'],_,_=run_measured([str(artifact)],Path.cwd())
                assert row['native_exit']==0,row
            save(); print(name,producer,code,round(wall,2),flush=True)
    if mode=='candidate':
        pair=[r for r in records if r['subject']==name]
        assert len(pair)==2
        if positive: assert pair[0]['native_sha256']==pair[1]['native_sha256'],pair
        else: assert clean(pair[0]['output'])==clean(pair[1]['output']),pair
    if positive and mode=='candidate':
        os.environ.pop('OMEGA_NORMALIZATION_VERIFY',None)
        command=[str(binary),'run','--both',subject]
        run=subprocess.run(command,capture_output=True,text=True,stdin=subprocess.DEVNULL)
        row=dict(subject=name,producer=mode,command=command,both=True,exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        records.append(row);save()
        assert run.returncode==0 and 'native exit 0' in run.stderr and 'interp exit: 0' in run.stderr,row
assert inputs=={path:digest(path) for path in inputs}
assert metadata['binaries']=={path:digest(path) for path in metadata['binaries']}
metadata['complete']=True;save()
