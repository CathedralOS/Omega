#!/usr/bin/env python3
"""Scoped local Git/runtime simulations; no agents or external services launched."""
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

import worktree_testkit as kit

TOOLS = Path(__file__).resolve().parents[1]
DISPATCH = TOOLS / 'swarm/dispatch.py'
CLAIMS = TOOLS / 'claims.py'


class DispatchTests(unittest.TestCase):
    def setUp(self):
        self.root, self.remote, self.a, self.b = kit.seed_remote_pair(self, 'omega dispatch ', 'Dispatch Test')
        for name in ('first', 'second'):
            path = self.a / 'samples/cli/facts' / name
            path.mkdir(parents=True)
            (path / 'main.omg').write_text('// original probe\n')
        spec = self.a / 'wiki/spec/contract.md'
        spec.parent.mkdir(parents=True)
        spec.write_text('Exact fact and operation; nine places required.\n')
        kit.git(self, self.a, 'add', '.')
        kit.git(self, self.a, 'commit', '-m', 'fact selection baseline')
        kit.clone_remote_pair(self, self.remote, self.a, self.b, 'Dispatch Test')
        self.base = kit.git(self, self.a, 'rev-parse', 'HEAD')
        (self.a / '.git/info/exclude').write_text('build/\n')
        self.module = kit.load_module(DISPATCH, 'dispatch_under_test')
        self.cache = self.root / 'retained-cache'
        self.cache.mkdir()
        (self.cache / 'artifact').write_bytes(b'preserved producer; validity still needs exact input checks')
        self.module.probe(self.a, [self.cache], initialize=True)

    def binding(self, path, record):
        path.write_text(json.dumps(record))
        return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())

    def packet(self, names=('first', 'second'), sessions=('one', 'two')):
        rows = []
        for name in names:
            item = 'samples/cli/facts/' + name
            rows.append(dict(item=item, remaining='Prove the original exact fact through all nine places',
                             reviewed_by='simulated parent gap review', specs=['wiki/spec/contract.md'],
                             source_inputs=self.module.qualification.source_inputs(
                                 self.a, self.base, [item, 'wiki/spec/contract.md'])))
        observed = datetime.now(timezone.utc).isoformat()
        probe = self.module.probe(self.a, [self.cache])
        session_rows = [dict(platform='codex', id=name, status='completed', available=True,
                             outcome_complete=True, observed_utc=observed,
                             runtime_fields=dict(id=['session_id'], status=['status']),
                             previous_probe=probe, current_probe=probe,
                             evidence=self.binding(self.root / (name + '.json'),
                                                   dict(session_id=name, status='completed')))
                        for name in sessions]
        return dict(base=self.base, candidates=rows, sessions=session_rows)

    def call(self, repository, *arguments, expected=0):
        result = subprocess.run([sys.executable, str(DISPATCH), '--repository', str(repository),
                                 *map(str, arguments)], env=self.environment, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return json.loads(result.stdout) if result.stdout else result.stderr

    def reserve(self, name='attempt', packet=None):
        path = self.root / (name + '.json')
        self.binding(path, packet or self.packet())
        attempt = self.root / (name + '-attempt.json')
        result = self.call(self.a, 'reserve', '--packet', path, '--attempt', attempt, '--owner', 'parent / ' + name)
        return attempt, result

    def arm(self, attempt, expected=0, observation=None):
        row = observation or json.loads(attempt.read_text())['session']['observation']
        row['observed_utc'] = datetime.now(timezone.utc).isoformat()
        path = self.root / 'refresh.json'
        self.binding(path, row)
        return self.call(self.a, 'arm', '--attempt', attempt, '--session-observation', path, expected=expected)

    def registry(self):
        result = subprocess.run([sys.executable, str(CLAIMS), '--repository', str(self.a), 'status'],
                                env=self.environment, capture_output=True, text=True, check=True)
        return json.loads(result.stdout)

    def test_concurrent_selection_dispatches_distinct_outcomes_and_sessions(self):
        packet_path = self.root / 'packet.json'
        self.binding(packet_path, self.packet())
        processes = [subprocess.Popen([sys.executable, str(DISPATCH), '--repository', str(repository),
                      'reserve', '--packet', str(packet_path), '--attempt', str(self.root / f'attempt-{index}.json'),
                      '--owner', f'parent{index}'], env=self.environment, stdout=subprocess.PIPE,
                      stderr=subprocess.PIPE, text=True) for index, repository in enumerate((self.a, self.b))]
        records = []
        for process in processes:
            output, error = process.communicate(timeout=30)
            self.assertEqual(process.returncode, 0, output + error)
            records.append(json.loads(output))
        self.assertEqual({r['phase'] for r in records}, {'reserved'})
        self.assertEqual(len({r['candidate']['item'] for r in records}), 2)
        self.assertEqual(len({r['session']['session_id'] for r in records}), 2)
        self.assertEqual(len(self.registry()['claims']), 2)

    def test_fresh_uuid_is_durable_before_registry_write_without_history_search(self):
        packet = self.packet(sessions=('one',))
        attempt = self.root / 'fresh-attempt.json'
        client = self.module.claims.Claims
        original = client.prepare_claim
        observed = []
        def prepare(registry, options, snapshot, live):
            saved = json.loads(attempt.read_text())
            self.assertEqual(saved['ticket'], options.ticket)
            self.assertEqual(saved['phase'], 'reserving')
            self.assertEqual(saved['candidate']['item'], options.item)
            self.assertEqual(saved['session']['session_id'], options.provenance['session_id'])
            self.assertTrue(options.fresh_ticket)
            def forbidden_history(*arguments, **keywords):
                raise AssertionError('fresh reservation searched history')
            with mock.patch.object(registry, 'git', side_effect=forbidden_history):
                result = original(registry, options, snapshot, live)
            observed.append(options.ticket)
            return result
        with mock.patch.object(client, 'prepare_claim', prepare):
            record = self.module.reserve(self.a, packet, attempt, 'parent / fresh')
        self.assertEqual(record['phase'], 'reserved')
        self.assertEqual(observed, [record['ticket']])

    def test_lost_reservation_response_preserves_written_ticket_for_reconciliation(self):
        attempt = self.root / 'lost-response-attempt.json'
        original = self.module.claims.Claims.coordinate
        def lose_response(registry, options):
            code = original(registry, options)
            self.assertEqual(code, 0)
            raise self.module.claims.ClaimsError('simulated lost response after publication')
        with mock.patch.object(self.module.claims.Claims, 'coordinate', lose_response):
            with self.assertRaisesRegex(self.module.RecoveryError, 'uncertain'):
                self.module.reserve(self.a, self.packet(sessions=('one',)), attempt, 'parent / lost')
        record = json.loads(attempt.read_text())
        self.assertEqual(record['phase'], 'uncertain')
        owned = [row for row in self.registry()['claims'] if row['ticket'] == record['ticket']]
        self.assertEqual(len(owned), 1)
        self.assertEqual(owned[0]['provenance']['dispatch_id'], record['ticket'])
        with self.assertRaisesRegex(self.module.RecoveryError, 'uncertain'):
            self.module.transition(self.a, attempt, 'arm')

    def test_process_interruption_after_publication_preserves_unsettled_identity(self):
        class Interrupted(BaseException):
            pass
        attempt = self.root / 'interrupted-attempt.json'
        original = self.module.claims.Claims.publish
        def interrupt(registry, expected, updated):
            self.assertTrue(original(registry, expected, updated))
            raise Interrupted()
        with mock.patch.object(self.module.claims.Claims, 'publish', interrupt):
            with self.assertRaises(Interrupted):
                self.module.reserve(self.a, self.packet(sessions=('one',)), attempt, 'parent / interrupted')
        record = json.loads(attempt.read_text())
        self.assertEqual(record['phase'], 'reserving')
        self.assertEqual([row['ticket'] for row in self.registry()['claims']], [record['ticket']])
        self.assertEqual(record['tries'], [])
        with self.assertRaisesRegex(self.module.RecoveryError, 'reconcile'):
            self.module.transition(self.a, attempt, 'arm')
        self.call(self.a, 'reserve', '--packet', self.root / 'unused-packet.json',
                  '--attempt', attempt, '--owner', 'parent / retry', expected=1)

    def test_definitely_unsent_failure_releases_own_reservation(self):
        attempt, record = self.reserve()
        self.assertEqual(record['phase'], 'reserved')
        self.call(self.a, 'cancel', '--attempt', attempt)
        self.assertEqual(self.registry()['claims'], [])
        self.call(self.a, 'reserve', '--packet', self.root / 'attempt.json', '--attempt', attempt,
                  '--owner', 'parent', expected=1)

    def test_unknown_send_preserves_reservation_and_prevents_retry(self):
        attempt, record = self.reserve(packet=self.packet(sessions=('one',)))
        self.arm(attempt)
        evidence = self.root / 'uncertain.json'
        self.binding(evidence, dict(error='transport timed out; actual outcome unknown'))
        self.call(self.a, 'uncertain', '--attempt', attempt, '--evidence', evidence)
        self.arm(attempt, expected=1)
        self.call(self.a, 'cancel', '--attempt', attempt, expected=1)
        _, next_record = self.reserve('another', self.packet(sessions=('one',)))
        self.assertEqual(next_record['phase'], 'no_work')
        self.assertEqual([r['ticket'] for r in self.registry()['claims']], [record['ticket']])

    def test_reuse_preserves_cache_and_progress_but_refuses_stale_main(self):
        (self.a / 'unpublished-progress').write_text('keep this session branch')
        before = self.module.probe(self.a, [self.cache])
        attempt, record = self.reserve()
        self.assertEqual(record['session']['action'], 'continue')
        self.assertTrue(record['session']['cache_continuity'])
        kit.git(self, self.a, 'worktree', 'add', '--detach', str(self.root / 'fresh-lane'), self.base)
        self.assertEqual(self.module.probe(self.a, [self.cache]), before)
        (self.b / 'new-main-file').write_text('main advanced')
        kit.git(self, self.b, 'add', '.')
        kit.git(self, self.b, 'commit', '-m', 'advance main')
        kit.git(self, self.b, 'push', 'origin', 'main')
        self.arm(attempt, expected=1)
        self.assertEqual(json.loads(attempt.read_text())['phase'], 'reserved')
        self.assertEqual((self.a / 'unpublished-progress').read_text(), 'keep this session branch')
        self.assertEqual(self.module.probe(self.a, [self.cache]), before)
        self.call(self.a, 'cancel', '--attempt', attempt)
        self.call(self.a, 'reserve', '--packet', self.root / 'attempt.json', '--attempt',
                  self.root / 'stale-attempt.json', '--owner', 'parent', expected=1)

    def test_arm_refreshes_expired_observation_and_refuses_running_session(self):
        attempt, record = self.reserve()
        row = record['session']['observation']
        row['status'] = 'running'
        row['evidence'] = self.binding(self.root / 'running.json', dict(session_id='one', status='running'))
        self.arm(attempt, expected=1, observation=row)
        self.assertEqual(json.loads(attempt.read_text())['phase'], 'reserved')
        persisted = json.loads(attempt.read_text())
        persisted['session']['observation']['observed_utc'] = '2000-01-01T00:00:00+00:00'
        attempt.write_text(json.dumps(persisted))  # Simulate only elapsed time in the stored old observation.
        row['status'] = 'completed'
        row['evidence'] = self.binding(self.root / 'completed.json', dict(session_id='one', status='completed'))
        self.arm(attempt, observation=row)
        self.assertEqual(json.loads(attempt.read_text())['phase'], 'sending')

    def test_peer_manual_claim_after_reservation_blocks_automatic_arm(self):
        attempt, record = self.reserve()
        result = subprocess.run([sys.executable, str(CLAIMS), '--repository', str(self.b), 'claim',
                                '--item', record['candidate']['item'], '--owner', 'explicit peer'],
                                env=self.environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.arm(attempt, expected=1)
        self.assertEqual(json.loads(attempt.read_text())['phase'], 'reserved')
        self.assertEqual(len(self.registry()['claims']), 2)

    def test_concurrent_arm_allows_only_one_parent_send(self):
        attempt, record = self.reserve()
        row = record['session']['observation']
        row['observed_utc'] = datetime.now(timezone.utc).isoformat()
        path = self.root / 'refresh.json'
        self.binding(path, row)
        processes = [subprocess.Popen([sys.executable, str(DISPATCH), '--repository', str(self.a),
                     'arm', '--attempt', str(attempt), '--session-observation', str(path)],
                     env=self.environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(2)]
        codes = []
        for process in processes:
            process.communicate(timeout=30)
            codes.append(process.returncode)
        self.assertEqual(sorted(codes), [0, 1])
        self.assertEqual(json.loads(attempt.read_text())['phase'], 'sending')

    def test_contradictory_runtime_evidence_refuses_dispatch(self):
        for envelope in [dict(session_id='other', status='completed'),
                         dict(session_id='one', status='running')]:
            packet = self.packet(sessions=('one',))
            packet['sessions'][0]['evidence'] = self.binding(self.root / 'wrong-runtime.json', envelope)
            packet_path = self.root / 'bad-packet.json'
            self.binding(packet_path, packet)
            self.call(self.a, 'reserve', '--packet', packet_path, '--attempt', self.root / 'bad-attempt.json',
                      '--owner', 'parent', expected=1)
        self.assertEqual(self.registry()['claims'], [])

    def test_provider_case_alias_cannot_double_reserve_one_session(self):
        packet = self.packet(sessions=('one',))
        packet['sessions'][0]['platform'] = 'Codex'
        attempt, record = self.reserve(packet=packet)
        self.assertEqual(record['session']['session_id'], 'codex:one')
        _, second = self.reserve(name='second', packet=self.packet(sessions=('one',)))
        self.assertEqual(second['phase'], 'no_work')
        self.arm(attempt)
        packet['sessions'][0]['platform'] = 'unrecognized'
        path = self.root / 'unknown.json'
        self.binding(path, packet)
        self.call(self.a, 'reserve', '--packet', path, '--attempt', self.root / 'unknown-attempt.json',
                  '--owner', 'parent', expected=1)

    def test_reprovisioned_session_never_promises_warm_cache(self):
        packet = self.packet()
        packet['sessions'][0]['current_probe'] = dict(machine='different machine', workspace=None, caches={})
        packet['sessions'] = packet['sessions'][:1]
        _, result = self.reserve(packet=packet)
        self.assertEqual(result['session']['action'], 'continue')
        self.assertFalse(result['session']['cache_continuity'])

    def test_filled_fact_requires_current_reviewed_failure_and_immutable_logs(self):
        for place in self.module.PLACES:
            path = self.a / 'samples/cli/facts/first' / place
            path.mkdir()
            (path / 'main.omg').write_text('completed place\n')
        kit.git(self, self.a, 'add', '.')
        kit.git(self, self.a, 'commit', '-m', 'filled row')
        kit.git(self, self.a, 'push', 'origin', 'main')
        self.base = kit.git(self, self.a, 'rev-parse', 'HEAD')
        packet = self.packet(names=('first',))
        row = packet['candidates'][0]
        stream = self.root / 'failure.log'
        stream.write_text('measured acceptance mismatch\n')
        binding = dict(path=str(stream), sha256=hashlib.sha256(stream.read_bytes()).hexdigest())
        observation = dict(revision=self.base, exit=1, invalidations=[],
                           source_inputs=row['source_inputs'], stdout=binding, stderr=binding)
        row['failure'] = self.binding(self.root / 'failure.json', observation)
        row['failure_review'] = 'simulated parent confirms semantic acceptance mismatch'
        self.assertEqual(self.module.unfinished(self.a, self.base, row)['kind'], 'reviewed_failure')
        stream.write_text('changed evidence')
        with self.assertRaisesRegex(self.module.RecoveryError, 'log changed'):
            self.module.unfinished(self.a, self.base, row)
        stream.write_text('measured acceptance mismatch\n')
        observation['invalidations'] = ['inputs changed during capture']
        row['failure'] = self.binding(self.root / 'failure.json', observation)
        with self.assertRaisesRegex(self.module.RecoveryError, 'noninvalidated'):
            self.module.unfinished(self.a, self.base, row)
        observation['invalidations'] = []
        observation['revision'] = '0' * 40
        row['failure'] = self.binding(self.root / 'failure.json', observation)
        with self.assertRaisesRegex(self.module.RecoveryError, 'noninvalidated'):
            self.module.unfinished(self.a, self.base, row)

    def test_completed_fact_not_selected_from_file_count_alone(self):
        for place in self.module.PLACES:
            path = self.a / 'samples/cli/facts/first' / place
            path.mkdir()
            (path / 'main.omg').write_text('completed place\n')
        kit.git(self, self.a, 'add', '.')
        kit.git(self, self.a, 'commit', '-m', 'filled row')
        kit.git(self, self.a, 'push', 'origin', 'main')
        self.base = kit.git(self, self.a, 'rev-parse', 'HEAD')
        _, record = self.reserve(packet=self.packet(names=('first',)))
        self.assertEqual(record['phase'], 'no_work')
        self.assertEqual(self.registry()['claims'], [])


if __name__ == '__main__':
    unittest.main()
