"""Qualify the frozen native-target scheduling candidate without rebuilding."""
import hashlib, importlib.util, json, os
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
BASE=ROOT/'build/copy-propagation-replay'
DATA=BASE/'quality-v2'
DATA.mkdir()
import sys
sys.path.insert(0,str(ROOT/'tools'))
from compiler_profile import require_optimized
producer=json.loads((BASE/'capture/results.json').read_text(encoding='utf-8'))['producer']
assert require_optimized(producer['binary'])['sha256']==producer['sha256']
spec = importlib.util.spec_from_file_location('first', ROOT / 'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first = importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE = DATA / 'observations'
first.EVIDENCE.mkdir()
first.self_test()
subjects = dict(first.SUBJECTS)
subjects.update({
    'spill': 'samples/cli/facts/live_across_call_spill/local/main.omg',
    'callee': 'samples/cli/facts/callee_saved_across_nested_calls/local/main.omg',
    'division': 'samples/cli/facts/unsigned_division_remainder/other_machine/main.omg',
    'abi-field': 'samples/cli/facts/abi_multi_arg_spill/field/main.omg',
    'stack16': 'samples/cli/facts/sixteen_arg_stack_calling_plan/other_machine/main.omg',
    'stack-slots': 'samples/cli/facts/stack_frame_slot_isolation/local/main.omg',
})
original_subjects=dict(subjects)
private=BASE/'quality-witnesses-v2'; private.mkdir()
for subject,relative in original_subjects.items():
    source=(ROOT/relative).parent; destination=private/subject; destination.mkdir()
    for path in source.rglob('*.omg'):
        target=destination/path.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(path.read_bytes())
    for path in destination.rglob('build.omg'):
        text=path.read_text();assert 'Optimizations::enable' not in text and text.rstrip().endswith('}')
        path.write_text(text.rstrip()[:-1]+'    Optimizations::enable(&mut builder.optimizations, Optimization::CopyPropagation);\n}\n',newline='\n')
    subjects[subject]=str((destination/'main.omg').relative_to(ROOT))
sources = {str(path.relative_to(ROOT)): first.digest(path) for relative in subjects.values() for path in (ROOT / relative).parent.rglob('*.omg')}
results = {'producer': producer, 'sources': sources, 'validation': [], 'compiles': [], 'controls': []}


def retain():
    (DATA / 'results.json').write_text(json.dumps(results, indent=2) + '\n', encoding='utf-8')


def env(cache, profile=False):
    result = dict(os.environ, LOCALAPPDATA=str(cache), OMEGA_LIBRARY_ROOT=str(ROOT / 'source/library'), RAYON_NUM_THREADS='32')
    result.pop('OMEGA_PHASE_PROFILE', None)
    if profile:
        result['OMEGA_PHASE_PROFILE'] = '1'
    return result


def compile_one(subject, label, cache, profile=False):
    output = first.EVIDENCE / label
    output.mkdir()
    observation, text = first.capture([producer['binary'], '--timings', '--target', 'windows_x86_64', '--build-dir', str(output), str(ROOT / subjects[subject])], label, env(cache, profile))
    artifact = first.compiled_artifact(observation, text, output)
    execution, _ = first.capture([str(artifact)], label + '-native', dict(os.environ))
    observation.update(subject=subject, native_exit=execution['exit'], native_observation=execution)
    assert execution['exit'] == 0, observation
    return observation, text


for subject, relative in subjects.items():
    source = ROOT / relative
    cache = DATA / ('validate-' + subject)
    cache.mkdir()
    observation, text = first.capture([producer['binary'], 'run', '--both', str(source)], 'both-' + subject, env(cache))
    observation.update(subject=subject, kind='both')
    results['validation'].append(observation)
    retain()
    assert observation['exit'] == 0 and 'native exit 0' in text and 'interp exit: 0' in text, observation
    rejection = json.loads(((ROOT/original_subjects[subject]).parent / 'invalid/sample.json').read_text(encoding='utf-8'))['rejection_contains']
    assert rejection
    observation, text = first.capture([producer['binary'], '--check', str(source.parent / 'invalid/main.omg')], 'reject-' + subject, env(cache))
    observation.update(subject=subject, kind='reject', expected=rejection, diagnostic=text)
    results['validation'].append(observation)
    retain()
    assert observation['exit'] == 1 and all(fragment in text for fragment in rejection), observation
    print('validated integrated producer', subject, flush=True)
assert all(first.digest(ROOT / relative) == value for relative, value in sources.items())
assert require_optimized(producer['binary'])['sha256'] == producer['sha256']
# A direct scalar-cycle and duplicate/multi-predecessor runtime falsifier.
source=ROOT/'wiki/drafts/measurements/copy_propagation_2026_10_29/scalar-order/main.omg'
cache=DATA/'scalar-order-cache';cache.mkdir()
observation,text=first.capture([producer['binary'],'run','--both',str(source)],'both-scalar-order',env(cache))
observation.update(subject='scalar-order',kind='both');results['validation'].append(observation);retain()
assert observation['exit']==0 and 'native exit 0' in text and 'interp exit: 0' in text,observation
results['complete'] = True
retain()
