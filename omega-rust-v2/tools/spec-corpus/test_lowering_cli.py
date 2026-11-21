import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import lowering_cli as cli


PATH = 'mod/-main.omg'
SOURCE = 'a\r\nλx\n'
MESSAGE = 'bad\tvalue\nreason'
DIAGNOSTIC = f'{PATH}:2:3: lowering: {MESSAGE} (bytes 5..6)\n'.encode()


def fixture(**changes):
    return dict({'target': 'tests/refuse.rs', 'test': 'one', 'outcome': f'refused: bad value reason at {PATH}',
                 'files': f'z.omg,{PATH}', 'origin_status': 'observed',
                 'location': dict(package='app', file=PATH, line=2, byte_column=3, byte_start=5, byte_end=6),
                 'replay': dict(entry='source', message=MESSAGE, root_message=MESSAGE,
                                expected_refusal=dict(file=PATH, byte_start=5, byte_end=6, message=MESSAGE, cli_message=MESSAGE), files=[['z.omg', ''], [PATH, SOURCE]])}, **changes)


class ReplayTests(unittest.TestCase):
    def setUp(self):
        Path("target").mkdir(exist_ok=True)
        temp = tempfile.TemporaryDirectory(dir=Path("target"))
        self.addCleanup(temp.cleanup)
        self.directory = Path(temp.name)
        self.exe = self.directory / 'omega.exe'
        self.exe.touch()
        self.artifact = dict(reason='compiler-artifact', target=dict(name='omega', kind=['bin']),
                             manifest_path=str(cli.ROOT / 'application/Cargo.toml'), executable=str(self.exe))
        self.artifacts = [dict(self.artifact, target=dict(name='other', kind=['bin'])), self.artifact]
        self.outputs = {'prepare': (0, b'', b''), 'lower': (1, b'', DIAGNOSTIC)}
        self.packages = []
        self.env = {'key': 'value'}

    def invoke(self, args, *, cwd, env, stdin, capture_output, timeout):
        self.assertIs(env, self.env)
        self.assertEqual(stdin, subprocess.DEVNULL)
        self.assertTrue(capture_output and timeout > 0)
        if args[0] == 'mbx':
            self.assertEqual(args, ['mbx', 'build', '--locked', '-p', 'omega', '--bin', 'omega', '--message-format=json'])
            self.assertEqual(cwd, cli.ROOT)
            stdout = '\n'.join(map(json.dumps, self.artifacts)).encode()
            return subprocess.CompletedProcess(args, 0, stdout, b'build log')
        self.assertEqual(args, [str(self.exe), 'source', args[2], '--', 'z.omg', PATH])
        self.assertEqual(cwd.parent, self.directory)
        self.assertEqual((cwd / PATH).read_bytes(), SOURCE.encode())
        self.assertEqual((cwd / 'z.omg').read_bytes(), b'')
        self.packages.append(cwd)
        return subprocess.CompletedProcess(args, *self.outputs[args[2]])

    def measure(self, *fixtures):
        report = dict(last_layer='L5', package_fixtures=list(fixtures), measurement_errors=[])
        with patch.object(cli.subprocess, 'run', side_effect=self.invoke) as run:
            cli.finish(report, self.directory, self.env)
        self.calls = run.call_count
        self.assertFalse(any(p.exists() for p in self.packages))
        json.dumps(report)  # coordinator persists the report; no Path/bytes left in witnesses
        return report['lowering_cli'], report['measurement_errors']

    def test_one_build_and_fresh_ordered_byte_exact_packages(self):
        ledger, errors = self.measure(fixture(), fixture(test='two'))
        self.assertEqual(errors, [])
        self.assertEqual(self.calls, 5)
        self.assertEqual(len(set(self.packages)), 2)
        self.assertEqual(ledger['dispositions'], {'verified': 2})
        self.assertEqual(ledger['results'][1]['lower']['stderr'], DIAGNOSTIC.decode())
        self.assertIn(b'build log', (self.directory / cli.LOG).read_bytes())
        self.assertIn('2 verified', ' '.join(cli.summary({'lowering_cli': ledger})))

    def test_wrong_command_outcomes_never_verify(self):
        for action, result in [('prepare', (1, b'', DIAGNOSTIC)), ('prepare', (0, b'x', b'')),
                               ('prepare', (0, b'', b'warning')), ('lower', (0, b'', DIAGNOSTIC)),
                               ('lower', (1, b'x', DIAGNOSTIC)), ('lower', (1, b'', b'')),
                               ('lower', (1, b'', DIAGNOSTIC.replace(b'5..6', b'5..5'))),
                               ('lower', (1, b'', DIAGNOSTIC.replace(b'bad\tvalue', b'bad value')))]:
            with self.subTest(action=action, result=result), patch.dict(self.outputs, {action: result}):
                ledger, errors = self.measure(fixture())
                self.assertEqual(self.calls, 3)
                self.assertTrue(errors)
                self.assertFalse(ledger['results'][0][action]['matched'])
                (self.directory / cli.LOG).unlink()

    def test_package_names_inside_a_diagnostic_come_from_a_real_root_reference(self):
        original = fixture()
        original['replay']['message'] = 'carrier app\\Mark'
        original['replay']['expected_refusal'].update(message='carrier app\\Mark', cli_message='carrier root\\Mark')
        original['outcome'] = f'refused: carrier app\\Mark at {PATH}'
        original['replay']['root_message'] = 'carrier root\\Mark'
        with patch.dict(self.outputs, lower=(1, b'', DIAGNOSTIC.replace(MESSAGE.encode(), b'carrier root\\Mark'))):
            ledger, errors = self.measure(original)
        self.assertEqual(errors, [])
        self.assertTrue(ledger['results'][0]['message_changed_with_package_identity'])
        (self.directory / cli.LOG).unlink()
        del original['replay']['root_message']
        _, errors = self.measure(original)
        self.assertIn('missing native diagnostic', errors[0])

    def test_agreement_between_cli_and_native_cannot_validate_the_wrong_occurrence_or_message(self):
        original = fixture()
        shifted = fixture(location=dict(package='app', file=PATH, line=1, byte_column=1, byte_start=0, byte_end=1))
        self.outputs['lower'] = (1, b'', f'{PATH}:1:1: lowering: {MESSAGE} (bytes 0..1)\n'.encode())
        ledger, errors = self.measure(shifted)
        self.assertEqual(self.calls, 0)
        self.assertEqual(ledger['dispositions'], {'unverified': 1})
        self.assertIn('independently specified', errors[0])
        for key in ('file', 'message', 'cli_message'):
            with self.subTest(key=key):
                changed = fixture()
                changed['replay']['expected_refusal'][key] = 'different'
                _, errors = self.measure(changed)
                self.assertTrue(errors)
                self.assertEqual(self.calls, 0)
        del original['replay']['expected_refusal']
        _, errors = self.measure(original)
        self.assertIn('missing independently specified L5 refusal', errors[0])

    def test_unsafe_windows_names_and_file_directory_collisions(self):
        for names in [['../outside'], ['/abs'], ['C:drive'], ['dir\\escape'], ['nested/NUL.txt'], ['a:stream'],
                      ['x/aux /y'], ['con .log'], ['LPT¹.txt/a'], ['x./y'], ['a//b'], ['./name'], ['../x/y'],
                      ['A/x', 'a/y'], ['dir/X', 'dir/x'], ['dir', 'dir/x'], ['dir/x', 'dir'], ['a', 'a']]:
            with self.subTest(names=names), self.assertRaises(ValueError):
                cli.program([[n, 'text'] for n in names])
        f = fixture()
        f['replay']['files'][0][0] = '../escaped.omg'
        ledger, errors = self.measure(f)
        self.assertEqual(self.calls, 0)
        self.assertIn('unsafe package path', errors[0])
        self.assertEqual(ledger['dispositions'], {'unverified': 1})

    def test_prepared_entry_must_reproduce_its_refusal_from_source(self):
        prepared = fixture()
        prepared['replay']['entry'] = 'prepared'
        ledger, errors = self.measure(prepared, fixture(origin_status='indistinguishable repeated receipt'))
        self.assertEqual((errors, self.calls), ([], 3))
        self.assertEqual(ledger['dispositions'], {'verified': 1, 'native origin not credited': 1})
        self.assertEqual(ledger['native_dispositions'], {'observed': 1, 'indistinguishable repeated receipt': 1})

    def test_dropped_or_changed_native_evidence_blocks_before_build(self):
        f = fixture()
        bad = [fixture(replay=None), fixture(replay=dict(f['replay'], files=f['replay']['files'][1:])),
               fixture(replay=dict(f['replay'], files=f['replay']['files'][::-1])),
               fixture(replay=dict(f['replay'], message='new reason')), fixture(location=None)]
        for field, value in [('package', 'external'), ('file', 'absent'), ('line', 1), ('byte_column', 2),
                             ('byte_end', 80), ('byte_start', 4)]:
            bad.append(fixture(location=dict(f['location'], **{field: value})))
        ledger, errors = self.measure(*bad)
        self.assertEqual(self.calls, 0)
        self.assertEqual(len(errors), len(bad))
        self.assertEqual(ledger['dispositions'], {'unverified': len(bad)})

    def test_missing_unique_artifact_blocks_and_build_log_is_never_overwritten(self):
        self.artifacts = [self.artifact, self.artifact]
        ledger, errors = self.measure(fixture(), fixture(test='two'))
        self.assertEqual(self.calls, 1)
        self.assertIn('compiler-artifact', errors[0])
        self.assertEqual(ledger['dispositions'], {'build unavailable': 2})
        log = (self.directory / cli.LOG).read_bytes()
        _, errors = self.measure(fixture())
        self.assertEqual(self.calls, 0)
        self.assertTrue(errors)
        self.assertEqual((self.directory / cli.LOG).read_bytes(), log)


class RecordedExpectationTests(unittest.TestCase):
    @unittest.skipUnless(os.getenv("OMEGA_FAULT_EVIDENCE"), "set OMEGA_FAULT_EVIDENCE to replay a full readiness run")
    def test_every_verified_refusal_rejects_another_consistent_native_location(self):
        report = json.loads((Path(os.environ["OMEGA_FAULT_EVIDENCE"]) / "evidence.json").read_text(encoding="utf-8"))
        checked = 0
        for row in report["lowering_cli"]["results"]:
            if row["status"] != "verified":
                continue
            fixture = report["package_fixtures"][row["fixture_index"]]
            replay = fixture["replay"]
            files = cli.program(replay["files"])
            with self.subTest(target=row["target"], test=row["test"], index=row["fixture_index"]):
                self.assertEqual(cli.expectation(fixture, replay, files), row["lower"]["stderr"])
                site = fixture["location"]
                source = files[site["file"]]
                # Either endpoint is a valid empty UTF-8 span. Move every reported coordinate together.
                pos = len(source) if (site["byte_start"], site["byte_end"]) == (0, 0) else 0
                prefix = source[:pos]
                other = dict(site, byte_start=pos, byte_end=pos, line=prefix.count(b"\n") + 1,
                             byte_column=pos - prefix.rfind(b"\n"), occurrence="")
                self.assertNotEqual((pos, pos), (site["byte_start"], site["byte_end"]))
                with self.assertRaisesRegex(ValueError, "independently specified"):
                    cli.expectation(dict(fixture, location=other), replay, files)
                checked += 1
        self.assertGreater(checked, 0)


if __name__ == '__main__':
    unittest.main()
