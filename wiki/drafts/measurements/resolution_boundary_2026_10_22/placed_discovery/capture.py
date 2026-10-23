"""Compare preserved discovery with a diagnostic bypass, retaining all outcomes."""
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

scripts = Path(__file__).resolve().parent
sys.path.insert(0, str(Path('tools/benchmark').resolve()))
from benchmark import capture_compile, parse_timings, require_optimized, run_measured

spec = importlib.util.spec_from_file_location('boundary_capture', scripts.parent/'capture.py')
boundary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boundary)
output = Path('build/placed-discovery')
binary = (output/'omega-instrumented').resolve()
baseline = Path('build/resolution-measurement/omega-baseline').resolve()
for producer in (baseline, binary):
    require_optimized(producer)
assert not subprocess.check_output(['git','diff','HEAD','--','omega-rust','source/library','samples','tests/omega'])
subjects = [
    ('length', 'samples/cli/facts/length/local/main.omg', 'positive'),
    ('dyn', 'samples/cli/facts/dyn_dispatch_selector_payload_bound/local/main.omg', 'positive'),
    ('record', 'samples/cli/facts/nested_nine_level_record_field_bound/local/main.omg', 'positive'),
    ('returned', 'tests/omega/pass/layouts/placed_reference_results/returned/main.omg', 'positive'),
    ('forwarded', 'tests/omega/pass/layouts/placed_reference_results/forwarded/main.omg', 'positive'),
    ('wrong_backing', 'tests/omega/fail/layouts/placed_reference_results/wrong_backing/main.omg', 'invalid'),
    ('overlapping_loans', 'tests/omega/fail/layouts/placed_reference_results/overlapping_loans/main.omg', 'invalid'),
    ('exact_policy_owner', 'tests/omega/fail/layouts/placed_policy_requires_exact_attached_owner/main.omg', 'invalid'),
    ('result_authority', 'tests/omega/fail/layouts/placed_factory_result_cannot_upgrade_source/main.omg', 'invalid'),
    *[(name, str(scripts.relative_to(Path.cwd())/'witnesses'/name/'main.omg'), 'baseline_failure')
      for name in ('inline_result','discarded_result','explicit_discard')],
]

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

input_paths = sorted({path for _, subject, _ in subjects for path in Path(subject).parent.rglob('*.omg')})
inputs = {str(path):digest(path) for path in input_paths}
metadata = dict(started_utc=datetime.now(timezone.utc).isoformat(),
    base=(output/'base.txt').read_text().strip(),
    checkout_revision=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),
    library_tree=subprocess.check_output(['git','rev-parse','HEAD:source/library'],text=True).strip(),
    baseline_sha256=digest(baseline), diagnostic_sha256=digest(binary), inputs=inputs,
    instrumentation_sha256=digest(output/'instrumentation.patch'))
records=[]

def save():
    (output/'observations.json').write_text(json.dumps(dict(metadata=metadata,records=records),indent=2)+'\n')

for round_number in range(2):
    selected = subjects if round_number==0 else subjects[:3]
    for name, subject, kind in selected:
        modes = ['baseline','full','skip'] if round_number==0 else ['skip','full']
        for mode in modes:
            with tempfile.TemporaryDirectory(prefix='omega-placed-discovery-') as temporary:
                command=[str(baseline if mode=='baseline' else binary),'--timings','--target','macos_arm64',
                         '--build-dir',temporary,str(Path(subject).resolve())]
                if kind=='invalid':
                    command=[str(baseline if mode=='baseline' else binary),'--timings','--check',str(Path(subject).resolve())]
                keys=['OMEGA_RESOLUTION_MEASURE','OMEGA_PLACED_DISCOVERY_MEASURE','OMEGA_PLACED_DISCOVERY_MODE']
                previous={key:os.environ.get(key) for key in keys}
                for key in keys: os.environ.pop(key,None)
                if mode!='baseline':
                    os.environ.update(OMEGA_RESOLUTION_MEASURE='1',OMEGA_PLACED_DISCOVERY_MEASURE='1',
                                      OMEGA_PLACED_DISCOVERY_MODE=mode)
                try:
                    code, wall_ms, rss, text=capture_compile(command,Path.cwd())
                finally:
                    for key,value in previous.items():
                        if value is None: os.environ.pop(key,None)
                        else: os.environ[key]=value
                timings,published=parse_timings(text)
                row=dict(name=name,subject=subject,kind=kind,mode=mode,round=round_number,command=command,
                         exit=code,wall_ms=wall_ms,peak_rss_bytes=rss,load_average=os.getloadavg(),
                         timings_ms=timings,output=text,
                         placement_trace=[line for line in text.splitlines() if line.startswith('PLACEDMEASURE\t')])
                records.append(row);save()
                if mode!='baseline':
                    row['trace']=boundary.traces(text)
                if code==0 and published:
                    artifact=Path(published)
                    row['native_sha256']=digest(artifact)
                    row['native_bytes']=artifact.stat().st_size
                    row['native_exit']=run_measured([str(artifact)],Path.cwd())[0]
                save()
            print(name,mode,round_number,code,row.get('native_exit'),round(wall_ms,3),flush=True)

assert inputs=={str(path):digest(path) for path in input_paths}
assert digest(baseline)==metadata['baseline_sha256'] and digest(binary)==metadata['diagnostic_sha256']
metadata['completed_utc']=datetime.now(timezone.utc).isoformat()
metadata['checks']=dict(inputs_unchanged=True,producers_unchanged=True,observations_complete=True)
save()
