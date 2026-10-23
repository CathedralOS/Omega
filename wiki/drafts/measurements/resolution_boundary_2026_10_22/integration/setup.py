import hashlib,json,subprocess,sys,os
from pathlib import Path
sys.path.insert(0,str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile,parse_timings,require_optimized,TIMING_LINE
output=Path('build/resolution-integration-final')
binary=(output/'omega-candidate').resolve()
require_optimized(binary)
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def clean(text):return '\n'.join(line for line in text.splitlines() if not TIMING_LINE.match(line) and not line.startswith('REBASE\t'))
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

rows=[]
for name,subject,positive in subjects:
    product=(output/'products'/name).resolve();product.mkdir(parents=True,exist_ok=True)
    command=[str(binary),'--timings','--target','macos_arm64','--build-dir',str(product),str(Path(subject).resolve())]
    code,wall,rss,text=capture_compile(command,Path.cwd())
    stages,published=parse_timings(text)
    row=dict(name=name,subject=subject,positive=positive,command=command,exit=code,output=text,wall_ms=wall)
    rows.append(row);(output/'setup.json').write_text(json.dumps(rows,indent=2)+'\n')
    assert (code==0)==positive,row
    if positive:
        assert published,row
        row['native']=str(Path(published).resolve());row['native_sha256']=digest(published)
    print(name,code,flush=True)
    (output/'setup.json').write_text(json.dumps(rows,indent=2)+'\n')
