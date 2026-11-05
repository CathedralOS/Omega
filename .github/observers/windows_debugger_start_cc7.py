"""Inspect exact retained Windows symbols at the debugger's initial break only."""
from pathlib import Path
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import time


def sha(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def marked_section(text, begin, end):
    lines = text.splitlines()
    starts = [index for index, line in enumerate(lines) if line.strip() == begin]
    ends = [index for index, line in enumerate(lines) if line.strip() == end]
    assert len(starts) == len(ends) == 1 and starts[0] < ends[0]
    return '\n'.join(lines[starts[0] + 1:ends[0]])


def main():
    recipe_path = Path(sys.argv[1]).resolve()
    recipe = json.loads(recipe_path.read_text())
    root = Path.cwd().resolve()
    output = root / 'build/atomic-windows-debugger-start-cc7'
    output.mkdir(parents=True, exist_ok=False)
    report = {'recipe_sha256': sha(recipe_path), 'passed': False,
              'classification': 'Debugger-start and local-symbol feasibility only; no source workload, runtime qualification or CPU sampling.'}
    cli = pdb = None
    try:
        assert os.name == 'nt' and platform.machine().lower() in ('amd64', 'x86_64')
        git = lambda *args: subprocess.check_output(['git', *args], text=True).strip()
        assert git('rev-parse', 'HEAD') == recipe['source']
        assert git('rev-parse', 'HEAD^{tree}') == recipe['tree']
        assert not git('status', '--porcelain')
        assert os.environ['OMEGA_DEBUG_START_CACHE_HIT'] == 'true'
        assert os.environ['OMEGA_DEBUG_START_CACHE_KEY'] == recipe['cache']['key']
        observer = os.environ['OMEGA_DEBUG_START_OBSERVER']
        assert observer == os.environ['GITHUB_SHA'] and observer != recipe['source']
        origin = json.loads(subprocess.check_output(
            ['gh', 'api', 'repos/CathedralOS/Omega/actions/runs/' + str(recipe['origin_run'])],
            text=True, timeout=15))
        artifact = json.loads(subprocess.check_output(
            ['gh', 'api', 'repos/CathedralOS/Omega/actions/artifacts/' + str(recipe['origin_artifact']['id'])],
            text=True, timeout=15))
        assert origin['status'] == 'completed' and origin['head_sha'] == recipe['source']
        assert artifact['workflow_run']['id'] == recipe['origin_run']
        assert artifact['workflow_run']['head_sha'] == recipe['source'] and not artifact['expired']
        for field in ('id', 'name', 'size_in_bytes', 'digest'):
            assert artifact[field] == recipe['origin_artifact'][field]
        (output / 'origin-run.json').write_text(json.dumps(origin, indent=2) + '\n')
        (output / 'origin-artifact.json').write_text(json.dumps(artifact, indent=2) + '\n')
        for path, expected in recipe['compiler_and_workflow_objects'].items():
            assert git('rev-parse', 'HEAD:' + path) == expected
        report.update(source=recipe['source'], tree=recipe['tree'], observer=observer,
                      original_producer=recipe['producer'], target='windows_x86_64',
                      cache=recipe['cache'], host=platform.platform())

        cli = Path('D:/ot/debug/omega.exe').resolve()
        pdb = cli.with_suffix('.pdb')
        assert cli.is_file() and pdb.is_file() and not cli.is_symlink() and not pdb.is_symlink()
        assert sha(cli) == recipe['producer']['cli_profile']['sha256']
        pdb_sha = sha(pdb)
        report.update(cli=str(cli), cli_sha256=sha(cli), pdb=str(pdb), pdb_sha256=pdb_sha,
                      pdb_bytes=pdb.stat().st_size)
        # Explicit local paths and a narrow environment leave no symbol-server
        # configuration or Actions credentials for the created debugger process.
        keys = ('SYSTEMROOT', 'WINDIR', 'PATH', 'PATHEXT', 'TEMP', 'TMP',
                'USERPROFILE', 'LOCALAPPDATA', 'APPDATA', 'COMSPEC')
        child_env = {key: value for key, value in os.environ.items() if key.upper() in keys}
        profile = subprocess.run([str(cli), '--build-profile'], stdin=subprocess.DEVNULL,
                                 capture_output=True, timeout=10, env=child_env)
        (output / 'build-profile.stdout').write_bytes(profile.stdout)
        (output / 'build-profile.stderr').write_bytes(profile.stderr)
        assert profile.returncode == 0
        assert json.loads(profile.stdout) == recipe['producer']['cli_profile']['profile']

        candidates = [Path('C:/Program Files (x86)/Windows Kits/10/Debuggers/x64/cdb.exe'),
                      Path('C:/Program Files/Windows Kits/10/Debuggers/x64/cdb.exe')]
        tools = [path for path in candidates if path.is_file()]
        assert len(tools) == 1, 'one existing x64 CDB is required; no installation was attempted'
        debugger = tools[0]
        commands = ('.reload /f omega.exe; .echo OMEGA_LMI_BEGIN; !lmi omega; '
                    '.echo OMEGA_LMI_END; .echo OMEGA_LMVM_BEGIN; lmvm omega; '
                    '.echo OMEGA_LMVM_END; q')
        arguments = [str(debugger), '-sins', '-noshell', '-nosqm', '-y', str(pdb.parent),
                     '-c', commands, str(cli), '--build-profile']
        report.update(debugger=str(debugger), debugger_sha256=sha(debugger),
                      arguments=arguments, debugger_commands=commands,
                      symbol_path=str(pdb.parent), debugger_wait_seconds=50,
                      cleanup_seconds=10)
        started = time.monotonic()
        timed_out = False
        with (output / 'debugger.stdout').open('wb') as stdout, (output / 'debugger.stderr').open('wb') as stderr:
            process = subprocess.Popen(arguments, stdin=subprocess.DEVNULL, stdout=stdout,
                                       stderr=stderr, cwd=output, env=child_env,
                                       creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
            report['owned_debugger_pid'] = process.pid
            try:
                process.wait(timeout=50)
            except subprocess.TimeoutExpired:
                timed_out = True
                # Only this created debugger and its descendants are targeted.
                # There is no PID attach, privilege adjustment or global capture.
                try:
                    killed = subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                                            stdin=subprocess.DEVNULL, capture_output=True,
                                            timeout=5, env=child_env)
                    (output / 'cleanup.stdout').write_bytes(killed.stdout)
                    (output / 'cleanup.stderr').write_bytes(killed.stderr)
                    report['cleanup_exit'] = killed.returncode
                finally:
                    if process.poll() is None:
                        process.kill()
                    process.wait(timeout=5)
        elapsed = time.monotonic() - started
        report.update(debugger_exit=process.returncode, debugger_seconds=round(elapsed, 3),
                      debugger_timed_out=timed_out)
        text = (output / 'debugger.stdout').read_text(errors='replace') + (output / 'debugger.stderr').read_text(errors='replace')
        assert not timed_out and elapsed <= 60 and process.returncode == 0
        lmi = marked_section(text, 'OMEGA_LMI_BEGIN', 'OMEGA_LMI_END')
        lmvm = marked_section(text, 'OMEGA_LMVM_BEGIN', 'OMEGA_LMVM_END')
        (output / 'omega-lmi.txt').write_text(lmi)
        (output / 'omega-lmvm.txt').write_text(lmvm)
        # Grade only the explicitly delimited omega module reports. A system
        # module's PDB or the echoed command line cannot satisfy this check.
        module = re.findall(r'^\s*Module:\s+(\S+)\s*$', lmi, re.M | re.I)
        lmi_pdb = bool(re.search(r'^\s*Symbol Type:\s+PDB\b', lmi, re.M | re.I))
        lmvm_pdb = bool(re.search(
            r'^\s*[0-9a-f`]+\s+[0-9a-f`]+\s+omega\s+[^\r\n]*\((?:private )?pdb symbols\)',
            lmvm, re.M | re.I))
        normalized_lmvm = lmvm.lower().replace('\\', '/')
        exact_image = str(cli).lower().replace('\\', '/') in normalized_lmvm
        exact_pdb = str(pdb).lower().replace('\\', '/') in normalized_lmvm
        report['omega_symbol_evidence'] = {
            'lmi_module_names': module, 'lmi_pdb': lmi_pdb, 'lmvm_omega_pdb': lmvm_pdb,
            'exact_image_path': exact_image, 'exact_local_pdb_path': exact_pdb,
        }
        assert [name.lower() for name in module] == ['omega']
        assert lmi_pdb and lmvm_pdb and exact_image and exact_pdb, 'matching local omega PDB loading is not demonstrated'
        assert '0x80070005' not in text.lower() and 'access is denied' not in text.lower()
        assert sha(cli) == recipe['producer']['cli_profile']['sha256'] and sha(pdb) == pdb_sha
        assert git('rev-parse', 'HEAD') == recipe['source'] and not git('status', '--porcelain')
        report.update(passed=True, local_pdb_loaded=True, postcheck_cli_sha256=sha(cli),
                      postcheck_pdb_sha256=sha(pdb))
    except Exception as error:
        report['error'] = repr(error)
    finally:
        try:
            if cli is not None and cli.is_file():
                report['postcheck_cli_sha256'] = sha(cli)
                assert report['postcheck_cli_sha256'] == recipe['producer']['cli_profile']['sha256']
            if pdb is not None and pdb.is_file():
                report['postcheck_pdb_sha256'] = sha(pdb)
                assert report['postcheck_pdb_sha256'] == report.get('pdb_sha256')
            after = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True, timeout=10).strip()
            dirty = subprocess.check_output(['git', 'status', '--porcelain'], text=True, timeout=10).strip()
            report.update(postcheck_source=after, postcheck_dirty=dirty)
            assert after == recipe['source'] and not dirty
        except Exception as error:
            report.update(passed=False, postcheck_error=repr(error))
        report['saved'] = {str(path.relative_to(output)): sha(path)
                           for path in output.rglob('*') if path.is_file()}
        (output / 'observation.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report, indent=2))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
