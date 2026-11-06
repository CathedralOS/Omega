"""Prefer an existing GNU GDB; otherwise use only signed Ubuntu packages."""
from pathlib import Path
from urllib.parse import urlsplit
import hashlib
import json
import os
import shutil
import subprocess
import time

out = Path('build/installed-entry-current-gdb-setup')
out.mkdir(parents=True, exist_ok=False)
report = dict(passed=False, uid=os.geteuid(), existing_tool=None, package_steps=[], gdb=None)
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
        text = source.read_text()
        source_rows = []
        for stanza in text.strip().split('\n\n'):
            fields = {}
            for line in stanza.splitlines():
                if not line.strip() or line.lstrip().startswith('#'):
                    continue
                key, separator, value = line.partition(':')
                assert separator and key not in fields
                fields[key] = value.strip()
            if not fields:
                continue
            assert fields.get('Types') == 'deb'
            assert fields.get('Signed-By') == str(keyring)
            assert all(suite in ('noble', 'noble-updates', 'noble-backports', 'noble-security')
                       for suite in fields['Suites'].split())
            for uri in fields['URIs'].split():
                parsed = urlsplit(uri)
                assert parsed.scheme in ('http', 'https') and parsed.username is None and parsed.password is None
                assert parsed.hostname in ('archive.ubuntu.com', 'security.ubuntu.com', 'ports.ubuntu.com') or parsed.hostname.endswith('.archive.ubuntu.com')
            source_rows.append({k: fields[k] for k in ('URIs', 'Suites', 'Signed-By')})
        assert source_rows and keyring.is_file()
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
            started = time.monotonic()
            with (out / (label + '.stdout')).open('wb') as stdout, (out / (label + '.stderr')).open('wb') as stderr:
                completed = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                    env=dict(os.environ, DEBIAN_FRONTEND='noninteractive'), timeout=seconds + 15)
            row = dict(step=label, command=command, exit=completed.returncode, seconds=round(time.monotonic() - started, 3))
            report['package_steps'].append(row)
            assert completed.returncode == 0, 'Official package setup failed; no retry or alternate route'
        name = shutil.which('gdb')
        assert name, 'Official GDB installation supplied no normal executable'
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
    report['error'] = dict(type=type(error).__name__, message=str(error))
finally:
    report['limits'] = 'Existing GNU GDB first; signed official Ubuntu sources/keyring only if absent. No retry or alternate tool. Package setup alone uses sudo; GDB, controller and Source remain non-root. No kernel, sysctl, capability, ptrace permission or compiler change. Setup failure occurs before producer acquisition.'
    (out / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(dict(protocol='installed-entry-current-gdb-v1', kind='tool_setup', **report)), flush=True)
raise SystemExit(0 if report['passed'] else 1)
