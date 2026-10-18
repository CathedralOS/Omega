#!/usr/bin/env python3
"""Reserve reviewed unfinished outcomes before external dispatch.

Claims are the shared exclusion mechanism. An attempt file is only a durable
single-send receipt, not a queue or compiler acceptance. Runtime observations
and human gap reviews are checked reports, not authenticated identities.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import claims
import qualification
from recovery_state import atomic_json, exclusive, read_json, RecoveryError

PLACES = ('local', 'field', 'parameter', 'call_result', 'match_join', 'const_name',
          'other_state', 'other_machine', 'target_body')


def require(condition, message):
    if not condition:
        raise RecoveryError(message)


def git(repository, *arguments):
    result = subprocess.run(['git', '-C', str(repository), *arguments],
                            capture_output=True, text=True, timeout=60)
    require(result.returncode == 0, result.stderr)
    return result.stdout.strip()


def main_base(repository):
    git(repository, 'fetch', '--no-recurse-submodules', 'origin',
        'refs/heads/main:refs/remotes/origin/main')
    return git(repository, 'rev-parse', 'FETCH_HEAD')


def artifact(binding):
    payload = Path(binding['path']).read_bytes()
    require(hashlib.sha256(payload).hexdigest() == binding['sha256'], 'evidence bytes changed')
    return json.loads(payload)


def unfinished(repository, base, row):
    require(row.get('remaining') and row.get('reviewed_by'), 'reviewed remaining acceptance required')
    identities = row['source_inputs']
    require(identities and qualification.source_inputs(repository, base, identities) == identities,
            'selection source fingerprints changed')
    specs = row['specs']
    require(specs and all(p.startswith('wiki/spec/') and identities.get(p) for p in specs),
            'bind governing canonical spec blobs')
    if row.get('board'):
        require(row['board'] in claims.BOARDS and identities.get(row['board']), 'bind supported board')
        text = git(repository, 'show', base + ':' + row['board'])
        require(row['item'] in [n.strip() for marker in claims.ITEM_MARKER.findall(text)
                               for n in marker.split('/')], 'outcome no longer on board')
    else:
        item = claims.normalize_path(row['item'])
        require(item == row['item'] and item.startswith('samples/cli/facts/') and
                len(Path(item).parts) == 4 and identities.get(item), 'one existing canonical fact folder required')
        present = set(git(repository, 'ls-tree', '-r', '--name-only', base, '--', item).splitlines())
        missing = [f'{item}/{place}/main.omg' for place in PLACES
                   if f'{item}/{place}/main.omg' not in present]
        if missing:
            return dict(kind='missing_places', paths=missing)
    if not row.get('failure'):
        return None  # Nine files are not semantic acceptance; require measured failure to dispatch more work.
    failure = artifact(row['failure'])
    require(failure['revision'] == base and failure['exit'] != 0 and not failure['invalidations'],
            'fresh noninvalidated failing qualification observation required')
    require(failure['source_inputs'] == qualification.source_inputs(repository, base, failure['source_inputs'])
            and all(failure['source_inputs'].get(p) == identities[p] for p in identities),
            'failed observation does not cover selection inputs')
    for stream in ('stdout', 'stderr'):
        binding = failure[stream]
        require(hashlib.sha256(Path(binding['path']).read_bytes()).hexdigest() == binding['sha256'],
                'failed observation log changed')
    require(row.get('failure_review'), 'parent must distinguish semantic gap from timing/infrastructure failure')
    return dict(kind='reviewed_failure', receipt=row['failure'], review=row['failure_review'])


def probe(repository, caches, runtime_id=None, initialize=False):
    def identity(path):
        path = Path(path).resolve()
        if not path.exists():
            return None
        value = path.stat()
        return dict(path=str(path), device=value.st_dev, inode=value.st_ino)
    marker = Path(repository) / 'build/swarm/workspace-continuity.json'
    with exclusive(str(marker) + '.lock'):
        if initialize and not marker.exists():
            atomic_json(marker, dict(token=uuid.uuid4().hex))
        token = read_json(marker)['token'] if marker.exists() else None
    boot = Path('/proc/sys/kernel/random/boot_id')
    if runtime_id is None and boot.is_file():
        runtime_id = boot.read_text().strip()
    return dict(machine=platform.node(), runtime_id=runtime_id, workspace_token=token, workspace=identity(repository),
                caches={str(Path(p).resolve()): identity(p) for p in caches})


def session_options(packet):
    result = []
    for row in packet.get('sessions', []):
        require(isinstance(row.get('id'), str) and row['id'].strip() and
                isinstance(row.get('platform'), str) and row['platform'].strip() and ':' not in row['platform'],
                'actual namespaced runtime handle required')
        session_id = claims.session_key(row['platform'] + ':' + row['id'])
        observed = datetime.fromisoformat(row['observed_utc'])
        require(0 <= (datetime.now(timezone.utc) - observed).total_seconds() <= 300,
                'refresh actual session observations')
        envelope = artifact(row['evidence'])
        # Adapter paths come from the discovered runtime schema, not guessed IDs.
        def extracted(field):
            value = envelope
            path = row['runtime_fields'][field]
            require(isinstance(path, list) and path, 'actual runtime field path required')
            for key in path:
                value = value[key]
            return value
        require(extracted('id') == row['id'] and extracted('status') == row['status'],
                'runtime receipt contradicts selected session identity/status')
        if str(row.get('status')).lower() not in {'complete', 'completed', 'done', 'finished', 'succeeded'} or not row.get('available') or not row.get('outcome_complete') or row.get('approval_required'):
            continue
        before, current = row.get('previous_probe'), row.get('current_probe')
        verified = bool(before and current and before == current and current.get('workspace')
                        and current.get('machine') and current.get('runtime_id')
                        and current.get('workspace_token') and current.get('caches')
                        and all(current['caches'].values()))
        result.append(dict(action='continue', session_id=session_id,
                           cache_continuity=verified, observation=row))
    # Reuse completed sessions even if reprovisioned, but never promise their caches survived.
    return sorted(result, key=lambda row: not row['cache_continuity'])


def claim_call(repository, *arguments):
    result = subprocess.run([sys.executable, str(Path(claims.__file__)), '--repository',
                             str(repository), *arguments], capture_output=True, text=True)
    try:
        record = json.loads(result.stdout)
    except ValueError:
        record = dict(error=result.stderr)
    return result.returncode, record


def reserve(repository, packet, attempt, owner, allow_create=False):
    base = main_base(repository)
    require(packet['base'] == base, 'main changed; refresh selection review before reservation')
    require(git(repository, 'rev-parse', 'HEAD') == base, 'select from isolated fetched-main checkout')
    sessions = session_options(packet)
    if allow_create:
        sessions.append(dict(action='create', cache_continuity=False))
    require(sessions, 'no available completed session; inspect runtime or explicitly authorize creation')
    ticket = uuid.uuid4().hex
    record = dict(base=base, ticket=ticket, owner=owner, phase='selecting', tries=[])
    atomic_json(attempt, record)
    for row in packet['candidates']:
        gap = unfinished(repository, base, row)
        if gap is None:
            continue
        for session in sessions:
            record.update(candidate=row, gap=gap, session=session, phase='reserving')
            atomic_json(attempt, record)  # Persist identity before a possibly uncertain registry write.
            arguments = ['reserve', '--ticket', ticket, '--owner', owner, '--item', row['item'],
                         '--dispatch-id', ticket, '--note', row['remaining']]
            if row.get('board'):
                arguments += ['--board', row['board']]
            for path in row.get('paths', [row['item']]):
                arguments += ['--path', path]
            if session.get('session_id'):
                arguments += ['--session-id', session['session_id']]
                observed = session['observation']
                current = observed.get('current_probe') or {}
                for field, value in [('machine-id', current.get('machine')),
                                     ('environment-id', observed.get('environment_id')),
                                     ('workspace-id', current.get('workspace_token'))]:
                    if value:
                        arguments += ['--' + field, str(value)]
            code, receipt = claim_call(repository, *arguments)
            record['tries'].append(dict(item=row['item'], session=session, code=code, receipt=receipt))
            record['phase'] = 'reserved' if code == 0 else 'occupied' if code == 2 else 'uncertain'
            if code == 0:
                record['reserved_utc'] = datetime.now(timezone.utc).isoformat()
            atomic_json(attempt, record)
            if code != 2:
                require(code == 0, 'reservation uncertain; inspect exact ticket before retrying')
                return record
    record['phase'] = 'no_work'
    atomic_json(attempt, record)
    return record


def transition(repository, attempt, action, evidence=None, session_observation=None):
    record = read_json(attempt)
    if action == 'arm':
        require(record['phase'] == 'reserved', 'send already armed or outcome uncertain; reconcile, do not retry')
        if record['session'].get('observation'):
            require(session_observation, 'arm requires a fresh actual session observation')
            refreshed = read_json(session_observation)
            require(claims.session_key(refreshed['platform'] + ':' + refreshed['id']) == record['session']['session_id'],
                    'refresh cannot change reserved session')
            require(datetime.fromisoformat(refreshed['observed_utc']) >=
                    datetime.fromisoformat(record['reserved_utc']), 'observe runtime again after reservation')
            refreshed['previous_probe'] = record['session']['observation'].get('previous_probe')
            eligible = session_options(dict(sessions=[refreshed]))
            record.setdefault('session_observations', []).append(refreshed)
            atomic_json(attempt, record)
            require(eligible, 'reserved session is no longer completed and available')
            record['session'] = eligible[0]
        require(main_base(repository) == record['base'], 'main moved; preserve reservation and refresh gap review')
        require(unfinished(repository, record['base'], record['candidate']) == record['gap'], 'gap changed')
        reader = claims.Claims(repository, 'origin')
        registry = reader.snapshot()['record']
        owned = [r for r in claims.live_claims(registry) if r['ticket'] == record['ticket']]
        require(len(owned) == 1 and owned[0]['owner'] == record['owner'] and
                owned[0]['item'] == record['candidate']['item'] and
                owned[0].get('provenance', {}).get('session_id') == record['session'].get('session_id'),
                'exact live outcome/session reservation required')
        require(not reader.reservation_conflicts(registry, record['candidate']['item'],
                    owned[0].get('provenance', {}), record['ticket']),
                'another outcome/session owner appeared; reconcile before automatic send')
        record['phase'] = 'sending'
    elif action == 'cancel':
        require(record['phase'] == 'reserved', 'only definitely unsent reservations may auto-release')
        code, receipt = claim_call(repository, 'release', '--ticket', record['ticket'])
        require(code == 0, 'release uncertain; inspect exact registry ticket')
        record.update(phase='cancelled', release=receipt)
    else:
        require(record['phase'] == 'sending', 'actual reserved send required')
        require(evidence, 'preserve actual runtime receipt before settling')
        record.update(phase='sent' if action == 'sent' else 'uncertain', evidence=evidence,
                      runtime_receipt=artifact(evidence))
    atomic_json(attempt, record)
    return record


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path, default=Path('.'))
    sub = parser.add_subparsers(dest='command', required=True)
    selection = sub.add_parser('reserve')
    selection.add_argument('--packet', type=Path, required=True)
    selection.add_argument('--attempt', type=Path, required=True)
    selection.add_argument('--owner', required=True)
    selection.add_argument('--allow-create', action='store_true')
    observe = sub.add_parser('probe')
    observe.add_argument('--cache', action='append', default=[])
    observe.add_argument('--runtime-id')
    observe.add_argument('--initialize', action='store_true', help='create an ignored workspace continuity token once; never replace it')
    for command in ('arm', 'cancel', 'sent', 'uncertain'):
        command_parser = sub.add_parser(command)
        command_parser.add_argument('--attempt', type=Path, required=True)
        command_parser.add_argument('--evidence', type=Path)
        if command == 'arm':
            command_parser.add_argument('--session-observation', type=Path)
    options = parser.parse_args(argv)
    try:
        if options.command == 'probe':
            result = probe(options.repository, options.cache, options.runtime_id, options.initialize)
        else:
            with exclusive(str(options.attempt) + '.lock'):
                if options.command == 'reserve':
                    require(not options.attempt.exists(), 'attempt exists; reconcile it instead of repeating dispatch')
                    result = reserve(options.repository, read_json(options.packet), options.attempt,
                                     options.owner, options.allow_create)
                else:
                    binding = None
                    if options.evidence:
                        binding = dict(path=str(options.evidence.resolve()),
                                       sha256=hashlib.sha256(options.evidence.read_bytes()).hexdigest())
                    result = transition(options.repository, options.attempt, options.command, binding,
                                        getattr(options, 'session_observation', None))
        print(json.dumps(result, indent=2))
        return 0
    except (RecoveryError, OSError, ValueError, KeyError, TypeError, IndexError, claims.ClaimsError) as error:
        print('dispatch: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
