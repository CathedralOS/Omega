"""Temporary frozen-input baseline; no Cargo build and no current-Main claim."""
from pathlib import Path
import hashlib, importlib.util, json, os, shutil, subprocess, sys, time
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
from compiler_profile import require_optimized
OUT = ROOT / 'build/compiler-pilot'
OUT.mkdir(parents=True, exist_ok=True)
D = OUT / 'generic-case-baseline'
D.mkdir()
INPUT = ROOT / 'build/generic-retained-input'
def h(p): return hashlib.sha256(p.read_bytes()).hexdigest()
def git(*a): return subprocess.check_output(['git', *a], cwd=ROOT, text=True).strip()
def save(name, value): (D / name).write_text(json.dumps(value, indent=2) + '\n')
candidate = git('rev-parse', 'HEAD')
assert not git('status', '--porcelain')
previous = json.loads((INPUT / 'summary.json').read_text())['actual_producer']
assert previous['revision'] == '1f3c4da0217eac2b7401cc0d3b3e0186d954e7e2'
inputs = json.loads((INPUT / 'copied-case-repair2/source-inputs.json').read_text())['files']
frozen = ROOT / 'build/verified-generic-producer-source1f3'
subprocess.run(['git', 'worktree', 'add', '--detach', str(frozen), previous['revision']], cwd=ROOT, check=True)
assert all(h(frozen / p) == value for p, value in inputs.items())
spec = importlib.util.spec_from_file_location('generic_frozen_owner', frozen / 'tools/ci/compiler_pilot.py')
owner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = owner
spec.loader.exec_module(owner)
retained = OUT / 'producer-generic-case-baseline'
retained.mkdir()
for name in ['omega', 'corpus_runner', 'corpus_runner.compiler-profile.json']:
    shutil.copy2(INPUT / Path(previous['cli']).parent.name / name, retained / name)
cli, runner = retained / 'omega', retained / 'corpus_runner'
for binary in [cli, runner]: binary.chmod(binary.stat().st_mode | 0o111)
cli_profile, runner_profile = require_optimized(cli), require_optimized(runner)
assert cli_profile['sha256'] == previous['cli_profile']['sha256']
assert runner_profile['sha256'] == previous['runner_profile']['sha256']
producer = owner.CompilerProducer(previous['revision'], previous['tree'], previous['target'], previous['toolchain'], cli.resolve(), runner.resolve(), cli_profile, runner_profile)
producer.verify()
(retained / 'producer.json').write_text(json.dumps(producer.record(), indent=2) + '\n')
for binary, profile in [(cli, cli_profile), (runner, runner_profile)]:
    Path(str(binary) + '.verified-profile.json').write_text(json.dumps(dict(source_revision=producer.revision, source_tree=producer.tree, toolchain=producer.toolchain, observation=profile), indent=2) + '\n')
os.environ['OMEGA_LIBRARY_ROOT'] = str(frozen / 'source/library')

ORIGINAL = {
  "samples/cli/facts/generic_case_subject/README.md": "0cae0aaa9ff8d0a0de3a5947f6d271043009b97ff2799bbb04590a0662ae40f4",
  "samples/cli/facts/generic_case_subject/call_result/build.omg": "91338dacf664f83ae66d918c437c95609d6b9ac98485824471534f596ba03b3b",
  "samples/cli/facts/generic_case_subject/call_result/invalid/build.omg": "91338dacf664f83ae66d918c437c95609d6b9ac98485824471534f596ba03b3b",
  "samples/cli/facts/generic_case_subject/call_result/invalid/main.omg": "f9638d8c13c0a7a93c8a1906e7013271e6de81f7af7f209ec988b9c7460c9238",
  "samples/cli/facts/generic_case_subject/call_result/main.omg": "14809fd7567867380096bd886c20ff89951bb594b6ce73bd454e9cef79b7139c",
  "samples/cli/facts/generic_case_subject/const_name/build.omg": "6d8ae48b181ad3a9a013d10491a9293b052fb17ee6af60dd89e46d1dfe586ee4",
  "samples/cli/facts/generic_case_subject/const_name/invalid/build.omg": "6d8ae48b181ad3a9a013d10491a9293b052fb17ee6af60dd89e46d1dfe586ee4",
  "samples/cli/facts/generic_case_subject/const_name/invalid/main.omg": "350826df1cba0205acb96be137fced25b81d66c461e8136347e41def0ca6e7c6",
  "samples/cli/facts/generic_case_subject/const_name/main.omg": "85a5b1c2c5fa334b7ac67ebdcd281695c1a09f5c7d2bc0c09d660127dc7749f6",
  "samples/cli/facts/generic_case_subject/field/build.omg": "ffbfb982b04a25ad24bd0763a9dd6d3f6b8ee26a575565bc74c5cebd382c908c",
  "samples/cli/facts/generic_case_subject/field/invalid/build.omg": "ffbfb982b04a25ad24bd0763a9dd6d3f6b8ee26a575565bc74c5cebd382c908c",
  "samples/cli/facts/generic_case_subject/field/invalid/main.omg": "fce19ef2de34df8179d53f77603d14e24b1ba503dd1f80951b77bff804ce6de7",
  "samples/cli/facts/generic_case_subject/field/main.omg": "5fb4bd5a75848c09b47a5932c94c19a8247f135394ae3125decbc2f1d0caf679",
  "samples/cli/facts/generic_case_subject/local/build.omg": "2725b007d6dda3394d21fa01fa38ba64e316e3cbb5deab74cfeec8a463950ad6",
  "samples/cli/facts/generic_case_subject/local/invalid/build.omg": "2725b007d6dda3394d21fa01fa38ba64e316e3cbb5deab74cfeec8a463950ad6",
  "samples/cli/facts/generic_case_subject/local/invalid/main.omg": "f197cf9755304c9d942007c88a5cbd641f13d9e1ea5be29bfd38e99e13bf067a",
  "samples/cli/facts/generic_case_subject/local/main.omg": "dbb79c689bce81b69c1474d837ccaffa1308336dd4fa5d416b81aeabbad82fd4",
  "samples/cli/facts/generic_case_subject/match_join/build.omg": "df29aad686c02d05eff8bdd9c55eebaf937e9435578d3297bf30a346d845328c",
  "samples/cli/facts/generic_case_subject/match_join/invalid/build.omg": "df29aad686c02d05eff8bdd9c55eebaf937e9435578d3297bf30a346d845328c",
  "samples/cli/facts/generic_case_subject/match_join/invalid/main.omg": "61ee4b56bb5836320cbd98149f5a9d0e707f7b5a3df882a1edeb66db5698ae2d",
  "samples/cli/facts/generic_case_subject/match_join/main.omg": "f44831a1c106f931c34d96f59e0c4f94408aec5ccaf1ea4f0069408fd009b724",
  "samples/cli/facts/generic_case_subject/other_machine/build.omg": "6c5632b678d7383b35f2716e33a7257b5e7159d05f7387909daf48e48ff5e318",
  "samples/cli/facts/generic_case_subject/other_machine/invalid/build.omg": "6c5632b678d7383b35f2716e33a7257b5e7159d05f7387909daf48e48ff5e318",
  "samples/cli/facts/generic_case_subject/other_machine/invalid/main.omg": "21e9dbb64b3fd65b2865d75972c8e882fd9dc800cb88c608af2ea14e5320fc81",
  "samples/cli/facts/generic_case_subject/other_machine/main.omg": "bb5297613d1103f8cfde7bfaa66fe26e3d7e8d61699230789df1246b1273b390",
  "samples/cli/facts/generic_case_subject/other_state/build.omg": "16da38a3493e32224e1164a0d17b3cfe82d575a984c5f880ee3cd9d99b6b86f5",
  "samples/cli/facts/generic_case_subject/other_state/invalid/build.omg": "16da38a3493e32224e1164a0d17b3cfe82d575a984c5f880ee3cd9d99b6b86f5",
  "samples/cli/facts/generic_case_subject/other_state/invalid/main.omg": "763de63a1278be34f7e522f055506f66df49643586ef056684dea859462a5bff",
  "samples/cli/facts/generic_case_subject/other_state/main.omg": "8a468535f8077d0959e09009d8dc75b4e36159031d47f67ba70fd2c2b1894f4a",
  "samples/cli/facts/generic_case_subject/parameter/build.omg": "b39c258f82ab65e82501abce4f9bf6d4cd5f89843c7b0fec6e44d21e1d06df9a",
  "samples/cli/facts/generic_case_subject/parameter/invalid/build.omg": "b39c258f82ab65e82501abce4f9bf6d4cd5f89843c7b0fec6e44d21e1d06df9a",
  "samples/cli/facts/generic_case_subject/parameter/invalid/main.omg": "1ddd44267a7d2af6e88aa28965fbeda6a54ea41e5e5abae7c546503b9459976f",
  "samples/cli/facts/generic_case_subject/parameter/main.omg": "a418e47b910966a88539fc68f6142d01f65af2a18cc5601aa0b263f752250b59",
  "samples/cli/facts/generic_case_subject/target_body/build.omg": "d4639ef4546317d95076d9d5579b60a86d9298424671aefef7c5edb59c830506",
  "samples/cli/facts/generic_case_subject/target_body/invalid/build.omg": "d4639ef4546317d95076d9d5579b60a86d9298424671aefef7c5edb59c830506",
  "samples/cli/facts/generic_case_subject/target_body/invalid/main.omg": "c75fb87bd548d8583759c240949d3ccff78c3f1540662b0e26214dc20dd7824d",
  "samples/cli/facts/generic_case_subject/target_body/main.omg": "77c5f3d8437b1cca6b70353c10d7cce72e8d84ca10efb1cf61d6f8e96e6395e9",
  "tests/omega/fail/facts/generic_case_subject_through_call_result_rejected/build.omg": "91338dacf664f83ae66d918c437c95609d6b9ac98485824471534f596ba03b3b",
  "tests/omega/fail/facts/generic_case_subject_through_call_result_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_call_result_rejected/main.omg": "4557202ecb948869d162a6514c90e8fe16e5116f6c5a1e2f98d5566c27d1b3f3",
  "tests/omega/fail/facts/generic_case_subject_through_const_name_rejected/build.omg": "6d8ae48b181ad3a9a013d10491a9293b052fb17ee6af60dd89e46d1dfe586ee4",
  "tests/omega/fail/facts/generic_case_subject_through_const_name_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_const_name_rejected/main.omg": "f01be0e5bd27d310aaa9f2c1f222a9e017f32f817646b38df055988a6aaba39b",
  "tests/omega/fail/facts/generic_case_subject_through_field_rejected/build.omg": "ffbfb982b04a25ad24bd0763a9dd6d3f6b8ee26a575565bc74c5cebd382c908c",
  "tests/omega/fail/facts/generic_case_subject_through_field_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_field_rejected/main.omg": "e8cbe4faf7cd3c612f78af1ab83cad0f7de93146432e2103db8d3f9b3740cbf2",
  "tests/omega/fail/facts/generic_case_subject_through_local_rejected/build.omg": "2725b007d6dda3394d21fa01fa38ba64e316e3cbb5deab74cfeec8a463950ad6",
  "tests/omega/fail/facts/generic_case_subject_through_local_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_local_rejected/main.omg": "12afe8ea8dea85876026287f86161d9f7dac5972b57fe52a7f11e34e379628fc",
  "tests/omega/fail/facts/generic_case_subject_through_match_join_rejected/build.omg": "df29aad686c02d05eff8bdd9c55eebaf937e9435578d3297bf30a346d845328c",
  "tests/omega/fail/facts/generic_case_subject_through_match_join_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_match_join_rejected/main.omg": "df94695bd7efb86d932aa5f1aa406fc8345c5e8fba119895f503650a27d51c95",
  "tests/omega/fail/facts/generic_case_subject_through_other_machine_rejected/build.omg": "6c5632b678d7383b35f2716e33a7257b5e7159d05f7387909daf48e48ff5e318",
  "tests/omega/fail/facts/generic_case_subject_through_other_machine_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_other_machine_rejected/main.omg": "76668bc2d18001b8a0b4f8084f321c542de29ee745bbad00af94212de28647c7",
  "tests/omega/fail/facts/generic_case_subject_through_other_state_rejected/build.omg": "16da38a3493e32224e1164a0d17b3cfe82d575a984c5f880ee3cd9d99b6b86f5",
  "tests/omega/fail/facts/generic_case_subject_through_other_state_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_other_state_rejected/main.omg": "dccbcdfd57aed0ed79e4314ef55cce06ecad2cc8f27e40a77fe565c79d839377",
  "tests/omega/fail/facts/generic_case_subject_through_parameter_rejected/build.omg": "b39c258f82ab65e82501abce4f9bf6d4cd5f89843c7b0fec6e44d21e1d06df9a",
  "tests/omega/fail/facts/generic_case_subject_through_parameter_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_parameter_rejected/main.omg": "9fd616d0d2fd1b071fa5015e60c7393f2326436e6ca1e8954fd1e07202b4d3ad",
  "tests/omega/fail/facts/generic_case_subject_through_target_body_rejected/build.omg": "d4639ef4546317d95076d9d5579b60a86d9298424671aefef7c5edb59c830506",
  "tests/omega/fail/facts/generic_case_subject_through_target_body_rejected/expected.txt": "9c5b9fe3e952a0dfe754ba7963b7ad9855aa37b04eb7f124d99f1648fc0cfc73",
  "tests/omega/fail/facts/generic_case_subject_through_target_body_rejected/main.omg": "2e08ba9c7932a09446be91ae798fa7374a84d2d51a1e51251c5ee54147a02749",
  "tests/omega/fail/generics/generic_case_subject_sibling_bounds_rejected/build.omg": "c0cb42a6e8e51335720938b975e739b356bdd06a23e82bd039545bbc9c193bdb",
  "tests/omega/fail/generics/generic_case_subject_sibling_bounds_rejected/expected.txt": "f5883317994118a49dc9424a156c72bbed2752c03f1ffd026b9d09fbb6f3f5ce",
  "tests/omega/fail/generics/generic_case_subject_sibling_bounds_rejected/main.omg": "e32ece68e69d23cf0485f5a01b5db4e266706ef152c7fa24ef6d7946f0addf58",
  "tests/omega/pass/facts/generic_case_subject_through_call_result_exit/build.omg": "91338dacf664f83ae66d918c437c95609d6b9ac98485824471534f596ba03b3b",
  "tests/omega/pass/facts/generic_case_subject_through_call_result_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_call_result_exit/main.omg": "14809fd7567867380096bd886c20ff89951bb594b6ce73bd454e9cef79b7139c",
  "tests/omega/pass/facts/generic_case_subject_through_const_name_exit/build.omg": "6d8ae48b181ad3a9a013d10491a9293b052fb17ee6af60dd89e46d1dfe586ee4",
  "tests/omega/pass/facts/generic_case_subject_through_const_name_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_const_name_exit/main.omg": "85a5b1c2c5fa334b7ac67ebdcd281695c1a09f5c7d2bc0c09d660127dc7749f6",
  "tests/omega/pass/facts/generic_case_subject_through_field_exit/build.omg": "ffbfb982b04a25ad24bd0763a9dd6d3f6b8ee26a575565bc74c5cebd382c908c",
  "tests/omega/pass/facts/generic_case_subject_through_field_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_field_exit/main.omg": "5fb4bd5a75848c09b47a5932c94c19a8247f135394ae3125decbc2f1d0caf679",
  "tests/omega/pass/facts/generic_case_subject_through_local_exit/build.omg": "2725b007d6dda3394d21fa01fa38ba64e316e3cbb5deab74cfeec8a463950ad6",
  "tests/omega/pass/facts/generic_case_subject_through_local_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_local_exit/main.omg": "dbb79c689bce81b69c1474d837ccaffa1308336dd4fa5d416b81aeabbad82fd4",
  "tests/omega/pass/facts/generic_case_subject_through_match_join_exit/build.omg": "df29aad686c02d05eff8bdd9c55eebaf937e9435578d3297bf30a346d845328c",
  "tests/omega/pass/facts/generic_case_subject_through_match_join_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_match_join_exit/main.omg": "f44831a1c106f931c34d96f59e0c4f94408aec5ccaf1ea4f0069408fd009b724",
  "tests/omega/pass/facts/generic_case_subject_through_other_machine_exit/build.omg": "6c5632b678d7383b35f2716e33a7257b5e7159d05f7387909daf48e48ff5e318",
  "tests/omega/pass/facts/generic_case_subject_through_other_machine_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_other_machine_exit/main.omg": "bb5297613d1103f8cfde7bfaa66fe26e3d7e8d61699230789df1246b1273b390",
  "tests/omega/pass/facts/generic_case_subject_through_other_state_exit/build.omg": "16da38a3493e32224e1164a0d17b3cfe82d575a984c5f880ee3cd9d99b6b86f5",
  "tests/omega/pass/facts/generic_case_subject_through_other_state_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_other_state_exit/main.omg": "8a468535f8077d0959e09009d8dc75b4e36159031d47f67ba70fd2c2b1894f4a",
  "tests/omega/pass/facts/generic_case_subject_through_parameter_exit/build.omg": "b39c258f82ab65e82501abce4f9bf6d4cd5f89843c7b0fec6e44d21e1d06df9a",
  "tests/omega/pass/facts/generic_case_subject_through_parameter_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_parameter_exit/main.omg": "a418e47b910966a88539fc68f6142d01f65af2a18cc5601aa0b263f752250b59",
  "tests/omega/pass/facts/generic_case_subject_through_target_body_exit/build.omg": "d4639ef4546317d95076d9d5579b60a86d9298424671aefef7c5edb59c830506",
  "tests/omega/pass/facts/generic_case_subject_through_target_body_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/facts/generic_case_subject_through_target_body_exit/main.omg": "77c5f3d8437b1cca6b70353c10d7cce72e8d84ca10efb1cf61d6f8e96e6395e9",
  "tests/omega/pass/generics/generic_case_subject_sibling_bounds_exit/build.omg": "c0cb42a6e8e51335720938b975e739b356bdd06a23e82bd039545bbc9c193bdb",
  "tests/omega/pass/generics/generic_case_subject_sibling_bounds_exit/expected_native_exit.txt": "9a271f2a916b0b6ee6cecb2426f0b3206ef074578be55d9bc94f6f3fe3ab86aa",
  "tests/omega/pass/generics/generic_case_subject_sibling_bounds_exit/main.omg": "3184d4176c433a9c7ffea0c2c37dc85975e52557c795ff18a6c027ccc539974f"
}
assert all(h(ROOT / p) == value for p, value in ORIGINAL.items())
save('original-program-inputs.json', ORIGINAL)
save('source-inputs.json', dict(actual_compiler_revision=producer.revision, files=inputs))
save('sdk-inputs.json', {p: value for p, value in inputs.items() if p.startswith('source/library/')})
places = ['local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body']
roster = []
for place in places:
    for positive in [True, False]:
        fixture = ('pass' if positive else 'fail') + '/facts/generic_case_subject_through_' + place + ('_exit' if positive else '_rejected')
        roster.append(fixture)
        sample = ROOT / 'samples/cli/facts/generic_case_subject' / place
        if not positive: sample = sample / 'invalid'
        pin = ROOT / 'tests/omega' / fixture
        for name in ['main.omg', 'build.omg']:
            assert (sample / name).read_bytes() == (pin / name).read_bytes()
reports = []
for label, fixtures, both_leg in [('checking', roster, False), ('both', [f for f in roster if f.startswith('pass/')], True)]:
    golden = D / (label + '-golden.txt')
    shutil.copy2(ROOT / 'tests/omega' / ('corpus_both_linux_x86_64.txt' if both_leg else 'corpus_outcomes.txt'), golden)
    command = [sys.executable, 'tools/corpus_gate.py', '--runner', str(runner), '--filter', ','.join(fixtures), '--record', '--golden', str(golden), '--json', str(D / (label + '.json'))]
    if both_leg: command.append('--both')
    started = time.monotonic()
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, stdin=subprocess.DEVNULL, timeout=600)
    (D / (label + '.stdout')).write_text(result.stdout)
    (D / (label + '.stderr')).write_text(result.stderr)
    report = json.loads((D / (label + '.json')).read_text())
    assert report['revision'] == candidate and report['dirty'] is False and report['exit_code'] in [0, 1]
    assert {row['fixture'] for row in report['observations']} == set(fixtures)
    if both_leg:
        proper = all(row['status'] == 'built' and row.get('facts', {}).get('exit') == '0' and row['facts'].get('interpreter-exit') == '0' and row['facts'].get('interpreter') == 'agrees' for row in report['observations'])
    else:
        proper = all((row['status'] == 'rejected' and row.get('expected_satisfied') is True) if row['fixture'].startswith('fail/') else row['status'] in ['checked', 'built'] for row in report['observations'])
    reports.append(dict(label=label, command=command, returncode=result.returncode, seconds=time.monotonic()-started, rows=len(fixtures), proper=proper, oracle_failures=report.get('oracle_failures'), infrastructure_error=report.get('infrastructure_error')))
    save('reports.json', reports)
producer.verify()
assert git('rev-parse','HEAD') == candidate and not git('status','--porcelain')
assert all(h(ROOT / p) == value for p, value in ORIGINAL.items())
assert all(h(frozen / p) == value for p, value in inputs.items())
summary = dict(candidate=candidate, fixed_original_warm_base='ce15d4160bff58859f8ec88982d4ef69119fd2e2', actual_producer=producer.record(), Rust_build=False, current_Main_execution_claim=False, reports=reports, passed=all(row['proper'] and not row['oracle_failures'] and not row['infrastructure_error'] for row in reports))
save('verdict.json', summary)
(OUT/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
print(json.dumps(summary), flush=True)
sys.exit(0 if summary['passed'] else 1)
