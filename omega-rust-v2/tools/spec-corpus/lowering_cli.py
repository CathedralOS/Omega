"""Replay the original inputs of observed L5 source-helper refusals at the package CLI."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory

from package_cli_build import ROOT, build

LOG = 'lowering-cli-build.log'
DEVICE = re.compile(r'CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|CLOCK\$|(?:COM|LPT)[0-9¹²³]', re.I | re.ASCII)


def program(rows):
    """Validate the entire file/directory namespace before creating anything."""
    if not isinstance(rows, list) or not rows:
        raise ValueError('missing source files')
    files, namespace = {}, {}
    for pair in rows:
        if not isinstance(pair, list) or len(pair) != 2 or not all(isinstance(s, str) for s in pair):
            raise ValueError('invalid source file record')
        name, source = pair
        parts = name.split('/')
        for i, part in enumerate(parts, 1):
            if (not part or part.endswith(('.', ' ')) or re.search(r'[\\<>:"|?*\x00-\x1f]', part)
                    or DEVICE.fullmatch(part.partition('.')[0].rstrip(' '))):
                raise ValueError(f'unsafe package path: {name!r}')
            path = '/'.join(parts[:i])
            entry = (path, i == len(parts))
            prior = namespace.setdefault(path.casefold(), entry)
            if prior != entry or path in files:
                raise ValueError(f'colliding package path: {name!r}')
        files[name] = source.encode('utf-8')
    return files


def expectation(fixture, replay, files):
    site = fixture['location']
    name, start, end = site['file'], site['byte_start'], site['byte_end']
    if site['package'] != 'app' or name not in files:
        raise ValueError('refusal lacks local app-package source')
    if not isinstance(fixture['files'], str) or list(files) != fixture['files'].split(','):
        raise ValueError('ordered replay filenames differ from native fixture')
    message = replay['message']
    if not isinstance(message, str) or not message:
        raise ValueError('missing native message')
    normalized = re.sub(r'[\r\n\t]', ' ', message)  # only the native outcome field flattens whitespace
    if fixture['outcome'] != f'refused: {normalized} at {name}':
        raise ValueError('original message disagrees with fixture outcome')
    source = files[name]
    if not all(type(site[k]) is int for k in ('byte_start', 'byte_end', 'line', 'byte_column')) or not 0 <= start <= end <= len(source):
        raise ValueError('invalid source range')
    prefix = source[:start]
    prefix.decode('utf-8')  # both endpoints must delimit a UTF-8 slice
    source[start:end].decode('utf-8')
    line, col = 1 + prefix.count(b'\n'), start - prefix.rfind(b'\n')
    if (line, col) != (site['line'], site['byte_column']):
        raise ValueError('native coordinates disagree with original source bytes')
    root_message = replay.get('root_message')
    if not isinstance(root_message, str) or not root_message:
        raise ValueError('missing native diagnostic under the CLI package identity')
    authored = replay.get('expected_refusal')
    if not isinstance(authored, dict):
        raise ValueError('missing independently specified L5 refusal')
    if (any(type(authored.get(k)) is not int for k in ('byte_start', 'byte_end'))
            or (authored.get('file'), authored.get('byte_start'), authored.get('byte_end'), authored.get('message'))
            != (name, start, end, message)):
        raise ValueError('native diagnostic disagrees with the independently specified file, byte range or message')
    if authored.get('cli_message') != root_message:
        raise ValueError('CLI package reference disagrees with the independently specified message')
    return f"{name}:{line}:{col}: lowering: {authored['cli_message']} (bytes {start}..{end})\n"


def program_digest(rows):
    """SHA-256 of json.dumps of the [file, text] rows, the digest replay records already use."""
    return hashlib.sha256(json.dumps(rows).encode()).hexdigest()


def materialize(root, files):
    """Create a validated `program` namespace in an empty directory, refusing any existing entry."""
    root = Path(root)
    created = {root}
    for name, data in files.items():
        path = root / name
        for parent in reversed(path.relative_to(root).parents):
            folder = root / parent
            if folder not in created:
                folder.mkdir()  # never reuse an on-disk alias
                created.add(folder)
        with path.open('xb') as out:
            out.write(data)


def compare(exe, directory, env, files, expected, row):
    with TemporaryDirectory(dir=directory, prefix='lowering-cli-') as temp:
        root = Path(temp)
        materialize(root, files)
        for action, code, stderr in [('prepare', 0, ''), ('lower', 1, expected)]:
            args = [str(exe), 'source', action, '--', *files]
            check = row[action] = {'args': args, 'expected_exit': code, 'expected_stdout': '',
                                   'expected_stderr': stderr, 'matched': False}
            try:
                run = subprocess.run(args, cwd=root, env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=30)
                check.update(exit=run.returncode, stdout=run.stdout.decode('utf-8', 'backslashreplace'),
                             stderr=run.stderr.decode('utf-8', 'backslashreplace'),
                             matched=(run.returncode == code and not run.stdout and run.stderr == stderr.encode('utf-8')))
            except (OSError, subprocess.TimeoutExpired) as error:
                check['error'] = str(error)
    if not all(row[action]['matched'] for action in ('prepare', 'lower')):
        raise ValueError('package command did not reproduce the native refusal; see command results')


def finish(report, directory, environment):
    if report.get('last_layer') != 'L5':
        return
    directory = Path(directory)
    rows, origins, build_record = [], Counter(), {}
    ledger = report['lowering_cli'] = {'results': rows, 'build': build_record}
    exe = None
    for index, fixture in enumerate(report['package_fixtures']):
        if not fixture['outcome'].startswith('refused: '):
            continue
        origin = fixture.get('origin_status', 'no origin reported')
        origins[origin] += 1
        replay = fixture.get('replay')
        entry = replay.get('entry') if isinstance(replay, dict) else None
        row = dict(fixture_index=index, target=fixture['target'], test=fixture['test'], entry=entry, origin_status=origin)
        rows.append(row)
        try:
            if entry not in ('source', 'prepared'):
                raise ValueError('missing source replay or invalid entry')
            if origin != 'observed':
                row['status'] = 'native origin not credited'
                continue
            files = program(replay.get('files'))
            # The exact package the commands compile; a later replay checks the recorded files against it.
            row['program_sha256'] = program_digest(replay['files'])
            expected = expectation(fixture, replay, files)
            row['message_changed_with_package_identity'] = replay['message'] != replay['root_message']
            if not build_record:
                try:
                    exe = build(directory, environment, build_record, LOG)
                except (OSError, ValueError) as error:
                    build_record['error'] = str(error)
                    report['measurement_errors'].append(f'L5 package command build: {error}')
            if exe is None:
                row['status'] = 'build unavailable'
                continue
            compare(exe, directory, environment, files, expected, row)
            row['status'] = 'verified'
        except (KeyError, TypeError, OSError, ValueError) as error:
            row.update(status='unverified', error=str(error))
            report['measurement_errors'].append(f"L5 package command fixture {index} ({row['target']}::{row['test']}): {error}")
    ledger.update(native_dispositions=dict(origins), dispositions=dict(Counter(r['status'] for r in rows)))


def summary(report):
    if 'lowering_cli' not in report:
        return []
    ledger = report['lowering_cli']
    counts = ', '.join(f'{n} {state}' for state, n in sorted(ledger['dispositions'].items()))
    return ['', '### L5 package-command replay', '',
            f"{len(ledger['results'])} native refusal records: {counts or 'none'}. "
            'Verified means the native refusal matched the fixture’s expected file, byte span and message, the package prepared silently, and `source lower` returned the expected diagnostic under its own package identity. '
            '[Commands, inputs and native dispositions](evidence.json).']
