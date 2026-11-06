"""Prefer an existing GNU GDB; otherwise use only signed Ubuntu packages."""
from pathlib import Path
from urllib.parse import urlsplit
import hashlib
import json
import os
import shutil
import subprocess
import time
import traceback

out = Path('build/installed-entry-current-gdb-setup')
out.mkdir(parents=True, exist_ok=False)
report = dict(passed=False, uid=os.geteuid(), existing_tool=None, package_steps=[], gdb=None, phase='preflight')
try:
    assert os.geteuid() != 0
    assert hasattr(os, 'pidfd_open')
    probe = os.pidfd_open(os.getpid())
    os.close(probe)
    name = shutil.which('gdb')
    report['existing_tool'] = name
    if name is None:
        # No PPA, alternative repository, unsigned-package option or new key.
        source = Path('/etc/apt/sources.list.d/ubuntu.sources')
        keyring = Path('/usr/share/keyrings/ubuntu-archive-keyring.gpg')
        source_bytes = source.read_bytes()
        text = source_bytes.decode()
        report['phase'] = 'validate_existing_ubuntu_sources'
        report['source_inventory'] = dict(path=str(source), bytes=len(source_bytes),
            sha256=hashlib.sha256(source_bytes).hexdigest(), field_names=[], stanza_fields=[])
        key_bytes = keyring.read_bytes()
        report['keyring_inventory'] = dict(path=str(keyring), bytes=len(key_bytes),
            sha256=hashlib.sha256(key_bytes).hexdigest())
        mirrors = Path('/etc/apt/apt-mirrors.txt')
        mirror_bytes = None
        source_rows = []
        for stanza in text.strip().split('\n\n'):
            fields = {}
            for line in stanza.splitlines():
                if not line.strip() or line.lstrip().startswith('#'):
                    continue
                # Deb822 field names are case-insensitive. A narrow grammar
                # and allowlist reject trust overrides and ambiguous continuations.
                assert not line[0].isspace(), 'Ubuntu source has an unsupported continuation line'
                key, separator, value = line.partition(':')
                assert separator and key.isascii() and all(c.isalnum() or c == '-' for c in key), 'Ubuntu source field name is malformed'
                key = key.lower()
                report['source_inventory']['field_names'].append(key)
                allowed = {'types', 'uris', 'suites', 'components', 'signed-by', 'architectures', 'enabled'}
                assert key in allowed and key not in fields, 'Ubuntu source field is unknown, duplicated or changes authentication'
                fields[key] = value.strip()
            if not fields:
                continue
            report['source_inventory']['stanza_fields'].append(sorted(fields))
            assert fields.get('types') == 'deb', 'Ubuntu source Types must be deb'
            assert fields.get('enabled', 'yes').lower() == 'yes', 'Ubuntu source is disabled'
            assert fields.get('signed-by') == str(keyring), 'Ubuntu source does not use the existing archive keyring'
            uris, suites = fields.get('uris', '').split(), fields.get('suites', '').split()
            assert uris and suites, 'Ubuntu source URI or suite roster is empty'
            assert fields.get('components', '').split(), 'Ubuntu source component roster is empty'
            assert all(suite in ('noble', 'noble-updates', 'noble-backports', 'noble-security')
                       for suite in suites), 'Ubuntu source has an unsupported suite'
            for uri in uris:
                if uri == 'mirror+file:/etc/apt/apt-mirrors.txt':
                    # This exact local format and its three official endpoints
                    # come from the pinned runner image's own build/test sources.
                    if mirror_bytes is None:
                        assert mirrors.is_file(), 'Pinned local Ubuntu mirror list is missing'
                        mirror_bytes = mirrors.read_bytes()
                        assert len(mirror_bytes) <= 65536, 'Local Ubuntu mirror list is too large'
                        report['mirror_inventory'] = dict(path=str(mirrors), bytes=len(mirror_bytes),
                            sha256=hashlib.sha256(mirror_bytes).hexdigest(), entries=[])
                        official = {'http://azure.archive.ubuntu.com/ubuntu/': 'priority:1',
                            'https://archive.ubuntu.com/ubuntu/': 'priority:2',
                            'https://security.ubuntu.com/ubuntu/': 'priority:3'}
                        retained_endpoints, retained_priorities = set(), set()
                        for row in mirror_bytes.decode().splitlines():
                            if not row.strip() or row.startswith('#'):
                                continue
                            endpoint, separator, metadata = row.partition('\t')
                            assert separator and endpoint in official, 'Local mirror entry has an unsupported endpoint or format'
                            assert metadata == official[endpoint], 'Local mirror entry has unsupported priority metadata'
                            assert endpoint not in retained_endpoints and metadata not in retained_priorities, 'Local mirror list repeats an endpoint or priority'
                            retained_endpoints.add(endpoint)
                            retained_priorities.add(metadata)
                            report['mirror_inventory']['entries'].append(dict(uri=endpoint, metadata=metadata))
                        assert retained_endpoints == set(official), 'Local Ubuntu mirror list does not match all three pinned endpoints'
                    continue
                parsed = urlsplit(uri)
                assert parsed.scheme in ('http', 'https') and parsed.username is None and parsed.password is None, 'Ubuntu source URI has an unsupported transport or credentials'
                assert parsed.hostname and (parsed.hostname in ('archive.ubuntu.com', 'security.ubuntu.com', 'ports.ubuntu.com') or parsed.hostname.endswith('.archive.ubuntu.com')), 'Ubuntu source URI is not an official Ubuntu endpoint'
            source_rows.append({k: fields[k] for k in ('uris', 'suites', 'signed-by')})
        assert source_rows and keyring.is_file(), 'Ubuntu source roster or archive keyring is missing'
        report['official_sources'] = dict(path=str(source), sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
            entries=source_rows, keyring=str(keyring), keyring_sha256=hashlib.sha256(keyring.read_bytes()).hexdigest())
        options = ['-o', 'Dir::Etc::sourcelist=' + str(source), '-o', 'Dir::Etc::sourceparts=-',
            '-o', 'APT::Get::AllowUnauthenticated=false', '-o', 'Acquire::AllowInsecureRepositories=false',
            '-o', 'Acquire::Retries=0', '-o', 'Acquire::http::Timeout=15', '-o', 'Acquire::https::Timeout=15',
            '-o', 'DPkg::Lock::Timeout=10', '-o', 'Dpkg::Use-Pty=0', '-qq']
        for label, seconds, arguments in [('update', 60, ['update']),
                ('install', 120, ['install', '-y', '--no-install-recommends', 'gdb'])]:
            command = ['sudo', '--non-interactive', 'timeout', '--signal=TERM', '--kill-after=5s',
                str(seconds) + 's', 'apt-get'] + options + arguments
            report['phase'] = 'apt_' + label
            assert source.read_bytes() == source_bytes and keyring.read_bytes() == key_bytes, 'Ubuntu source or keyring changed after validation'
            assert mirror_bytes is None or mirrors.read_bytes() == mirror_bytes, 'Local Ubuntu mirror list changed after validation'
            started = time.monotonic()
            with (out / (label + '.stdout')).open('wb') as stdout, (out / (label + '.stderr')).open('wb') as stderr:
                completed = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                    env=dict(os.environ, DEBIAN_FRONTEND='noninteractive'), timeout=seconds + 15)
            row = dict(step=label, command=command, exit=completed.returncode, seconds=round(time.monotonic() - started, 3))
            report['package_steps'].append(row)
            assert completed.returncode == 0, 'Official package setup failed; no retry or alternate route'
        name = shutil.which('gdb')
        assert name, 'Official GDB installation supplied no normal executable'
    report['phase'] = 'probe_gnu_gdb'
    tool = Path(name).resolve(strict=True)
    mode = tool.stat()
    report['tool_inventory'] = dict(resolved_path=str(tool), executable=os.access(tool, os.X_OK),
        is_file=tool.is_file(), uid=mode.st_uid, gid=mode.st_gid, mode=oct(mode.st_mode), bytes=mode.st_size)
    assert tool.is_file() and os.access(tool, os.X_OK)
    version = subprocess.run([str(tool), '--version'], stdin=subprocess.DEVNULL, capture_output=True, timeout=10)
    (out / 'version.stdout').write_bytes(version.stdout)
    (out / 'version.stderr').write_bytes(version.stderr)
    assert version.returncode == 0 and version.stdout.startswith(b'GNU gdb ')
    report['gdb'] = dict(path=str(tool), sha256=hashlib.sha256(tool.read_bytes()).hexdigest(),
        version=version.stdout.decode(errors='replace'), version_exit=version.returncode)
    report['passed'] = True
except BaseException as error:
    report['error'] = dict(type=type(error).__name__, message=str(error), traceback=traceback.format_exc())
finally:
    report['limits'] = 'Existing GNU GDB first; signed official Ubuntu sources/keyring only if absent. No retry or alternate tool. Package setup alone uses sudo; GDB, controller and Source remain non-root. No kernel, sysctl, capability, ptrace permission or compiler change. Setup failure occurs before producer acquisition.'
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(protocol='installed-entry-current-gdb-v1', kind='tool_setup', **report)), flush=True)
raise SystemExit(0 if report['passed'] else 1)
