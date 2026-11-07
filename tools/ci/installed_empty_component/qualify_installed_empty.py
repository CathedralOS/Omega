"""Bind one fresh installed-empty capture to its explicit ignored codec test.

Preparation builds only terminal-codec's library test image. Qualification
consumes the installed_empty_snapshot observations already made by the full98
consumer; it never substitutes a different program, artifact or historical run.
Capture and test checkouts keep separate CI identities; both must use the
same independently reviewed compiler inputs, host, toolchain and library.
This component does not decide the full98 or whole-candidate acceptance.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import signal
import struct
import subprocess
import sys
import time

TEST = ('sections::semantic_module::reference_forwarding_tests::'
        'qualified_installed_empty_rejects_missing_forwarding_condition')
TEST_ENV = 'OMEGA_INSTALLED_EMPTY_FORWARDING_CANONICAL_ARTIFACT'
TEST_SOURCE = ('omega-rust/psi/compilation/representations/terminal-psi/codec/src/'
               'sections/semantic_module/reference_forwarding_tests.rs')
TEST_SOURCE_SHA = '6ba5e0d3edabe01771b6976df388114b2e8db7e6b60630eb8262cc961967e766'
MAIN_SHA = '4f5de7dcb8868c8760c32906917896db06c3dc327fd3ac94911d4c244e7bd45a'
BUILD_SHA = 'e8cc5b78a4e0d4c3d24790f7c350fe8fabdddab5ffbc7e9d5649498c3d9e9f87'
RELATIVE_MAIN = 'build/supplied-output-loan/installed-empty-snapshot-control/main.omg'
REASON = 'reference establishment cannot retain source membership obligations'
INPUTS = ('omega-rust', 'source/library', 'source/psi', 'source/omega',
          'bootstrap', 'tools/benchmark', 'Cargo.toml', 'Cargo.lock',
          'rust-toolchain.toml', '.cargo')
OVERRIDES = ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER',
             'RUSTC_WORKSPACE_WRAPPER', 'RUSTC', 'RUSTDOC', 'CARGO_BUILD_RUSTC')


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def sha(path):
    path = Path(path)
    require(path.is_file() and not path.is_symlink(), 'not a regular direct file: ' + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def git(root, *arguments, absent=False):
    result = subprocess.run(['git', *arguments], cwd=root, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=30)
    if result.returncode and absent:
        return None
    result.check_returncode()
    return result.stdout.strip()


def compiler_inputs(root, args):
    require(re.fullmatch('[0-9a-f]{40}', args.expected_compiler_source),
            'expected source needs a full commit SHA or product-source label')
    manifest_path, manifest_sha = args.compiler_input_roster, args.compiler_input_roster_sha256
    require((manifest_path is None) == (manifest_sha is None),
            'supply the reviewed compiler-input manifest and its pinned SHA together')
    if manifest_path is None:
        objects = {name: git(root, 'rev-parse', args.expected_compiler_source + ':' + name,
                             absent=True) for name in INPUTS}
        provenance = dict(mode='historical-commit-resolution')
    else:
        require(re.fullmatch('[0-9a-f]{64}', manifest_sha), 'input manifest needs a full SHA256')
        require(sha(manifest_path) == manifest_sha, 'reviewed compiler-input manifest changed')
        manifest = read(manifest_path)
        require(manifest.get('compiled_source') == args.expected_compiler_source,
                'reviewed manifest names another product-source label')
        objects = manifest.get('all_ten_compiler_input_objects')
        provenance = dict(mode='reviewed-explicit-object-roster',
                          manifest=str(manifest_path.resolve()), sha256=manifest_sha)
    require(isinstance(objects, dict) and set(objects) == set(INPUTS),
            'compiler-input roster must contain exactly the ten reviewed keys')
    require(all((name == '.cargo' and value is None)
                or (isinstance(value, str) and re.fullmatch('[0-9a-f]{40}', value))
                for name, value in objects.items()), 'compiler-input object is absent or malformed')
    return objects, provenance


def toolchain(root):
    result = subprocess.run(['rustc', '-Vv'], cwd=root, stdin=subprocess.DEVNULL,
                            capture_output=True, text=True, timeout=30, check=True)
    return result.stdout.strip()


def source_identity(root, args):
    expected = args.expected_compiler_source
    require(re.fullmatch('[0-9a-f]{40}', expected), 'expected source needs a full commit SHA')
    revision = git(root, 'rev-parse', 'HEAD')
    require(not git(root, 'status', '--porcelain'), 'tracked source or index is dirty')
    require(os.environ.get('GITHUB_SHA', revision) == revision, 'CI checkout is not its recorded head')
    objects, roster = compiler_inputs(root, args)
    for name, identity in objects.items():
        require(git(root, 'rev-parse', 'HEAD:' + name, absent=True) == identity,
                'current compiled input differs from expected source: ' + name)
    require(sha(root / TEST_SOURCE) == TEST_SOURCE_SHA, 'reviewed ignored test is absent or changed')
    system = {'Darwin': 'macos', 'Linux': 'linux'}.get(platform.system())
    cpu = {'aarch64': 'arm64', 'arm64': 'arm64', 'x86_64': 'x86_64', 'amd64': 'x86_64'}.get(platform.machine().lower())
    require(system and cpu, 'this proposal supports matching POSIX hosts only')
    ci_run = {key: os.environ.get(name) for key, name in (
        ('repository', 'GITHUB_REPOSITORY'), ('run_id', 'GITHUB_RUN_ID'),
        ('attempt', 'GITHUB_RUN_ATTEMPT'), ('job', 'GITHUB_JOB'))}
    return dict(revision=revision, tree=git(root, 'rev-parse', 'HEAD^{tree}'), ci_run=ci_run,
                expected_compiler_source=expected, compiler_inputs=objects,
                host=system + '_' + cpu, toolchain=toolchain(root),
                test_source_sha256=TEST_SOURCE_SHA,
                compiler_input_roster=roster,
                driver_sha256=sha(Path(__file__)))


def modules(root):
    names = [name for name in git(root, 'ls-files', '-z', '--', 'source/library').split('\0') if name]
    require(names, 'tracked bundled library is unavailable')
    files = {name: sha(root / name) for name in names}
    for name in names:
        require((root / name).resolve().is_relative_to(root / 'source/library'), 'library file escapes its root')
    actual = {p.relative_to(root).as_posix() for p in (root / 'source/library').rglob('*.omg') if p.is_file()}
    require(actual == {n for n in names if n.endswith('.omg')}, 'actual library module inventory differs')
    return files


def environment(root, profile):
    env = profile.optimized_environment()
    for name in OVERRIDES:
        require(not env.get(name), 'unreviewed compiler override: ' + name)
    for name in ('OMEGA_CALL_WRITE_TRACE', 'OMEGA_STRUCT_TRACE', 'OMEGA_COMPILER_CHECK_SITES'):
        env.pop(name, None)
    env['OMEGA_LIBRARY_ROOT'] = str(root / 'source/library')
    return env


def command(root, directory, name, arguments, env, validate, timeout):
    validate()
    require(shutil.disk_usage(directory).free >= 800_000_000, 'capture floor is unavailable')
    stdout, stderr = directory / (name + '.stdout'), directory / (name + '.stderr')
    row = dict(command=list(map(str, arguments)), timeout_seconds=timeout, timed_out=False)
    process = None
    began = time.monotonic()
    try:
        with stdout.open('xb') as out, stderr.open('xb') as err:
            process = subprocess.Popen(row['command'], cwd=root, env=env, stdin=subprocess.DEVNULL,
                                       stdout=out, stderr=err, start_new_session=True)
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                row['timed_out'] = True
    except BaseException as error:
        row['error'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        try:
            if process is not None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=10)
                row['exit'] = process.returncode
                def members():
                    ps = subprocess.run(['ps', '-axo', 'pid=,pgid=,stat=,comm='],
                                        capture_output=True, text=True, timeout=10, check=True)
                    return [line.strip() for line in ps.stdout.splitlines()
                            if len(fields := line.split(None, 3)) == 4
                            and int(fields[1]) == process.pid and not fields[2].startswith('Z')]
                deadline = time.monotonic() + 10
                remaining = members()
                while remaining and time.monotonic() < deadline:
                    time.sleep(0.05)  # bounded owned-child drain, never remote-job polling
                    remaining = members()
                row['remaining_owned_processes'] = remaining
                require(not remaining, 'command process group did not close')
        finally:
            row.update(seconds=round(time.monotonic() - began, 3), stdout=str(stdout), stderr=str(stderr))
            for key, path in [('stdout', stdout), ('stderr', stderr)]:
                if path.exists():
                    row[key + '_sha256'] = sha(path)
            write(directory / (name + '-command.json'), row)
            validate()
    require(not row['timed_out'] and row.get('exit') == 0, 'command failed: ' + name)
    return row


def prepare(args, root, profile, slot, output, report):
    identity = source_identity(root, args)
    library = modules(root)
    env = environment(root, profile)
    report.update(test_identity=identity, library_input_sha256=library,
                  effective_environment={key: env.get(key) for key in
                      (*OVERRIDES, 'OMEGA_LIBRARY_ROOT', 'CARGO_PROFILE_DEV_OPT_LEVEL',
                       'CARGO_PROFILE_TEST_OPT_LEVEL', 'CARGO_TARGET_DIR', 'CARGO_INCREMENTAL')})
    def validate():
        require(source_identity(root, args) == identity, 'source/host/driver changed')
        require(modules(root) == library, 'library changed')
    with slot.hold(root):
        require(shutil.disk_usage(output).free >= 1_300_000_000, 'build launch floor is unavailable')
        command(root, output, 'rustc-version', ['rustc', '-Vv'], env, validate, 30)
        command(root, output, 'cargo-version', ['cargo', '--version'], env, validate, 30)
        report['toolchain'] = (output / 'rustc-version.stdout').read_text().strip()
        require(report['toolchain'] == identity['toolchain'], 'test toolchain changed')
        tool = 'mbx' if shutil.which('mbx') else 'cargo'
        report['build_tool'] = dict(name=tool, path=shutil.which(tool))
        arguments = [tool, 'test', '--locked', '--lib', '-p', 'terminal-codec', TEST,
                     '--no-run', '--message-format=json']
        report['build'] = command(root, output, 'build', arguments, env, validate, 1800)
        messages = [json.loads(line) for line in (output / 'build.stdout').read_text().splitlines()
                    if line.startswith('{')]
        endings = [m for m in messages if m.get('reason') == 'build-finished']
        require(len(endings) == 1 and endings[0].get('success') is True, 'Cargo build did not finish successfully')
        artifacts = [m for m in messages if m.get('reason') == 'compiler-artifact'
                     and m.get('target', {}).get('name') == 'terminal_codec'
                     and m.get('target', {}).get('kind') == ['lib']
                     and m.get('profile', {}).get('test') is True and m.get('executable')]
        require(len(artifacts) == 1, 'Cargo did not publish one exact codec library test image')
        artifact = artifacts[0]
        codegen = profile.artifact_codegen(artifact)
        require(codegen['opt_level'] in ('1', '2', '3', 's', 'z')
                and str(artifact['profile']['opt_level']) == codegen['opt_level'], 'test image is not effectively optimized')
        image = Path(artifact['executable']).resolve()
        before = sha(image)
        profile.require_executable_image(image, image.read_bytes())
        require(os.access(image, os.X_OK), 'test image is not executable')
        retained = output / 'terminal-codec-test-image'
        shutil.copy2(image, retained)
        require(sha(retained) == before == sha(image), 'test image changed during retention')
        report.update(cargo_artifact=artifact, effective_codegen=codegen, image_sha256=before,
                      original_image=str(image), retained_image=str(retained), selector=TEST,
                      ignored_test=True, test_execution=False)
        validate()
    report['passed'] = True


def raw_row(row, captured):
    for field in ('stdout', 'stderr'):
        captured(row[field], row[field + '_sha256'])
    require(row.get('exit') == 0 and row.get('timed_out') is False
            and row.get('process_tree_closed') is True, 'target capture command did not complete cleanly')


def envelope(path):
    data = Path(path).read_bytes()
    require(len(data) >= 37 and data[:8] == b'PSIART\0\0', 'not a canonical artifact envelope')
    require(struct.unpack_from('<H', data, 8)[0] == 3, 'not current envelope3')
    semantic, optimization = struct.unpack_from('<QQ', data, 10)
    flag = data[26]
    require(flag in (0, 1), 'invalid debug-section tag')
    start = 35 if flag else 27
    debug = struct.unpack_from('<Q', data, 27)[0] if flag else 0
    require(start + semantic + optimization + debug == len(data), 'envelope section lengths disagree')
    require(semantic >= 10 and data[start:start + 8] == b'PSITERM\0'
            and struct.unpack_from('<H', data, start + 8)[0] == 249, 'not fresh semantic249')
    return dict(envelope=3, semantic=249, bytes=len(data), debug_bytes=debug)


def qualify(args, root, profile, slot, output, report):
    identity = source_identity(root, args)
    require(args.capture_binding is not None and args.capture_binding_sha256 is not None,
            'supply the reviewed capture binding and pinned SHA256 together')
    require(re.fullmatch('[0-9a-f]{64}', args.capture_binding_sha256)
            and sha(args.capture_binding) == args.capture_binding_sha256,
            'reviewed capture binding is absent or changed')
    binding = read(args.capture_binding)
    require(binding.get('schema') == 'omega-installed-empty-capture-binding-v1',
            'unknown capture binding schema')
    capture_run = binding['capture_run']
    require(set(capture_run) == {'repository', 'run_id', 'job_id', 'attempt'}
            and capture_run['repository'] == 'CathedralOS/Omega'
            and all(type(capture_run[key]) is int and capture_run[key] > 0
                    for key in ('run_id', 'job_id', 'attempt')), 'invalid capture run identity')
    capture_ci = binding['capture_ci']
    require(set(capture_ci) == {'revision', 'tree'}
            and all(isinstance(value, str) and re.fullmatch('[0-9a-f]{40}', value)
                    for value in capture_ci.values()), 'invalid capture CI identity')
    require(binding['compiled_source'] == args.expected_compiler_source
            and binding['all_ten_compiler_input_objects'] == identity['compiler_inputs'],
            'capture and test use different reviewed compiler inputs')
    require(binding['source_sha256'] == MAIN_SHA and binding['build_sha256'] == BUILD_SHA,
            'capture binding selects another source or oracle')
    require(args.capture_root is not None, 'supply the actual restored capture root')
    capture_root = args.capture_root.resolve()
    require(capture_root.is_dir() and capture_root.is_relative_to(root / 'build'),
            'restored capture must stay in an ignored build directory')
    restored = binding['restored_files']
    require(isinstance(restored, dict) and restored, 'capture transport roster is absent')
    used_files = {}
    def captured(locator, expected_sha):
        require(isinstance(locator, str) and Path(locator).is_absolute()
                and re.fullmatch('[0-9a-f]{64}', expected_sha), 'invalid captured locator/hash')
        entry = restored[locator]
        require(set(entry) == {'member', 'sha256'} and entry['sha256'] == expected_sha,
                'restored file does not match its captured hash')
        member = PurePosixPath(entry['member'])
        require(str(member) == entry['member'] and not member.is_absolute()
                and all(part not in ('.', '..') for part in member.parts)
                and chr(92) not in str(member), 'invalid restored capture member')
        path = capture_root.joinpath(*member.parts)
        require(path.resolve().is_relative_to(capture_root) and sha(path) == expected_sha,
                'restored capture file is missing, outside its root or changed')
        record = dict(restored_path=str(path.resolve()), sha256=expected_sha)
        require(locator not in used_files or used_files[locator] == record,
                'captured locator was rebound')
        used_files[locator] = record
        return path.resolve()
    require(args.preparation.resolve() == captured(binding['preparation_locator'], binding['preparation_sha256'])
            and args.results.resolve() == captured(binding['results_locator'], binding['results_sha256']),
            'capture report locators changed')
    preparation = read(args.preparation)
    require(preparation.get('schema') == 'omega-compiler-preparation-v1' and preparation.get('passed') is True,
            'producer preparation is unavailable or failed')
    producer = preparation['producer']
    require(preparation['candidate'] == producer['revision'] == capture_ci['revision']
            and producer['tree'] == capture_ci['tree'] and producer == binding['producer']
            and producer['target'] == preparation['target'] == identity['host']
            and producer['toolchain'] == identity['toolchain'], 'capture producer/source/host/toolchain mismatch')
    def captured_profile(locator, recorded):
        image = captured(locator, recorded['sha256'])
        require(recorded['binary'] == locator, 'profile names another captured image')
        expected = dict(recorded, binary=str(image))
        if 'build_receipt' in recorded:
            receipt = recorded['build_receipt']
            path = captured(receipt['path'], receipt['sha256'])
            require(path == Path(str(image) + '.compiler-profile.json'),
                    'restored profile receipt is not beside its captured image')
            expected['build_receipt'] = dict(receipt, path=str(path))
        actual = profile.require_optimized(image)
        require(actual == expected, 'restored producer bytes/profile/receipt changed')
        return actual
    require(args.producer_image.resolve() == args.receiver_image.resolve()
            == captured(producer['cli'], producer['cli_profile']['sha256']),
            'the public receiver must be the exact restored captured producer image')
    captured_profile(producer['cli'], producer['cli_profile'])
    captured_profile(producer['runner'], producer['runner_profile'])
    results = read(args.results)
    capture = results['identity']
    require(capture['revision'] == capture_ci['revision'] and capture['producer'] == producer
            and capture['compiler_inputs'] == identity['compiler_inputs']
            and capture['preparation_sha256'] == sha(args.preparation), 'full98 capture provenance mismatch')
    require(capture['expected_compiler_source'] == args.expected_compiler_source,
            'full98 capture names another compiler source')
    source = Path(capture['input_root']) / RELATIVE_MAIN
    require(args.source.resolve() == captured(str(source), MAIN_SHA)
            and args.build.resolve() == captured(str(source.with_name('build.omg')), BUILD_SHA),
            'frozen source/Build transport mismatch')
    require(modules(root) == capture['actual_library_input_sha256']
            == binding['library_input_sha256'], 'effective bundled library differs from capture')
    rows = [row for row in results['observations']
            if row.get('group') == 60 and row.get('name') == 'installed_empty_snapshot']
    require([row['mode'] for row in rows] == ['check', 'both', 'inspect'],
            'need exactly the target check/BOTH/SAME-artifact receiving rows')
    by_mode = {r['mode']: r for r in rows}
    for row in rows:
        require(row['source'] == RELATIVE_MAIN and row['negative'] is False, 'wrong captured control')
        raw_row(row, captured)
    cli = producer['cli']
    require(by_mode['check']['command'] == [cli, '--check', str(source)], 'source check command mismatch')
    both = by_mode['both']
    require(both['command'] == [cli, 'run', '--both', '--keep', str(source)], 'BOTH command mismatch')
    stderr = captured(both['stderr'], both['stderr_sha256']).read_text(errors='replace')
    require(re.findall(r'^native exit (-?\d+)$', stderr, re.M) == ['0']
            and re.findall(r'^interp exit: (-?\d+)$', stderr, re.M) == ['0']
            and both.get('native_exit') == both.get('terminal_exit') == 0, 'no actual engine agreement')
    kept = re.findall(r'^build dir kept: (.+)$', stderr, re.M)
    require(len(kept) == 1 and both.get('kept_build_directory') == kept[0], 'kept artifact is ambiguous')
    original_locator = str(Path(kept[0]) / 'program.oterm')
    inspected = by_mode['inspect']
    artifact_sha = binding['artifact_sha256']
    artifact = captured(inspected['artifact'], artifact_sha)
    require(args.artifact.resolve() == artifact
            and inspected['command'] == [cli, 'inspect-terminal', '--artifact', inspected['artifact']],
            'artifact is not the restored exact public receiver input')
    require(artifact_sha == both['artifact_sha256']
            == inspected['sha256_before'] == inspected['sha256_after'] == inspected['kept_sha256_after']
            and inspected.get('same_as_kept_product') is True, 'artifact differs from same-invocation output')
    original_exported = binding['original_kept_product_exported']
    require(type(original_exported) is bool, 'kept-product export status must be explicit')
    if original_exported:
        captured(original_locator, artifact_sha)
    else:
        require(original_locator not in restored,
                'an exported kept product cannot be described as unexported')
    header = envelope(artifact)
    test_preparation = read(args.test_preparation)
    require(test_preparation.get('schema') == 'omega-installed-empty-codec-test-preparation-v2'
            and test_preparation.get('passed') is True and test_preparation['test_identity'] == identity
            and test_preparation['selector'] == TEST and test_preparation['ignored_test'] is True,
            'test preparation/source/selector mismatch')
    require(test_preparation['toolchain'] == producer['toolchain'], 'producer/test toolchains differ')
    require(test_preparation['library_input_sha256'] == capture['actual_library_input_sha256'],
            'test preparation used another library')
    image = args.test_image.resolve()
    require(image == Path(test_preparation['retained_image']).resolve(), 'test image was substituted')
    image_sha = test_preparation['image_sha256']
    require(sha(image) == image_sha == sha(test_preparation['original_image']), 'test image bytes differ')
    build = test_preparation['build']
    require(build['exit'] == 0 and not build['timed_out'] and not build['remaining_owned_processes'], 'test build failed')
    for field in ('stdout', 'stderr'):
        require(sha(build[field]) == build[field + '_sha256'], 'test build raw changed')
    cargo = [json.loads(line) for line in Path(build['stdout']).read_text().splitlines() if line.startswith('{')]
    require(sum(m.get('reason') == 'build-finished' and m.get('success') is True for m in cargo) == 1,
            'test Cargo completion is absent')
    require(test_preparation['cargo_artifact'] in cargo, 'test image has no exact Cargo artifact')
    cargo_artifact = test_preparation['cargo_artifact']
    require(cargo_artifact.get('target', {}).get('name') == 'terminal_codec'
            and cargo_artifact.get('target', {}).get('kind') == ['lib']
            and cargo_artifact.get('profile', {}).get('test') is True
            and Path(cargo_artifact['executable']).resolve() == Path(test_preparation['original_image']).resolve(),
            'test image is not the captured codec library executable')
    require(profile.artifact_codegen(test_preparation['cargo_artifact']) == test_preparation['effective_codegen']
            and test_preparation['effective_codegen']['opt_level'] in ('1', '2', '3', 's', 'z'),
            'test Cargo fingerprint changed or is not optimized')
    profile.require_executable_image(image, image.read_bytes())
    immutable = {p.resolve(): sha(p) for p in
                 [args.capture_binding, args.preparation, args.results, args.test_preparation, args.source, args.build,
                  artifact, image, Path(test_preparation['original_image']),
                  Path(build['stdout']), Path(build['stderr'])]}
    immutable[args.capture_binding.resolve()] = args.capture_binding_sha256
    immutable.update({Path(row['restored_path']): row['sha256'] for row in used_files.values()})
    if args.compiler_input_roster is not None:
        prior_roster = Path(test_preparation['retained_compiler_input_roster'])
        require(sha(prior_roster) == args.compiler_input_roster_sha256,
                'test preparation retained another reviewed input manifest')
        immutable[prior_roster.resolve()] = args.compiler_input_roster_sha256
    env = environment(root, profile)
    require(not env.get(TEST_ENV), 'unexpected inherited artifact selector')
    env[TEST_ENV] = str(artifact)
    report.update(test_identity=identity, capture_identity=dict(capture_ci, ci_run=capture_run,
                      compiled_source=capture['expected_compiler_source'],
                      all_ten_compiler_input_objects=capture['compiler_inputs'],
                      host=producer['target'], toolchain=producer['toolchain'],
                      library_input_sha256=capture['actual_library_input_sha256']),
                  capture_binding_sha256=args.capture_binding_sha256,
                  source_sha256=MAIN_SHA, build_sha256=BUILD_SHA,
                  producer=producer, test_image_sha256=image_sha, artifact_sha256=artifact_sha,
                  artifact=str(artifact), captured_artifact_locator=inspected['artifact'],
                  original_artifact_locator=original_locator,
                  original_kept_product_exported=original_exported,
                  original_product_check=('restored-export-matches-captured-hash' if original_exported
                      else 'retained-export-matches-actual-BOTH-and-inspect-kept-before-after-hashes'),
                  capture_filesystem_transport=dict(root=str(capture_root), files=used_files),
                  artifact_formats=header,
                  capture_results_sha256=sha(args.results), captured_target_rows=rows,
                  test_preparation_sha256=sha(args.test_preparation), test_codegen=test_preparation['effective_codegen'],
                  full98_failures_preserved=results.get('failures', []), selector=TEST, ignored_test=True)
    def validate():
        require(source_identity(root, args) == identity, 'source/host changed')
        require(modules(root) == capture['actual_library_input_sha256'], 'library changed')
        for path, digest in immutable.items():
            require(sha(path) == digest, 'bound input/image/artifact changed: ' + str(path))
        for row in rows:
            raw_row(row, captured)
        captured_profile(producer['cli'], producer['cli_profile'])
        captured_profile(producer['runner'], producer['runner_profile'])
        require(profile.artifact_codegen(test_preparation['cargo_artifact']) == test_preparation['effective_codegen'],
                'test fingerprint changed')
    with slot.hold(root):
        test = command(root, output, 'ignored-test', [str(image), TEST, '--exact', '--ignored',
                       '-Z', 'unstable-options', '--format=json', '--nocapture', '--test-threads=1'],
                       env, validate, 300)
    events = [json.loads(line) for line in (output / 'ignored-test.stdout').read_text().splitlines() if line.strip()]
    starts = [e for e in events if e.get('type') == 'suite' and e.get('event') == 'started']
    ends = [e for e in events if e.get('type') == 'suite' and e.get('event') in ('ok', 'failed')]
    passed = [e.get('name') for e in events if e.get('type') == 'test' and e.get('event') == 'ok']
    require(len(starts) == len(ends) == 1 and starts[0].get('test_count') == 1
            and ends[0].get('event') == 'ok' and ends[0].get('passed') == 1
            and all(ends[0].get(k) == 0 for k in ('failed', 'ignored', 'measured'))
            and passed == [TEST], 'not exactly one actual ignored-test execution/pass')
    lines = re.findall(r'^INSTALLED_EMPTY_FORWARDING_OMISSION (.+)$',
                       (output / 'ignored-test.stderr').read_text(errors='replace'), re.M)
    require(len(lines) == 1, 'missing or ambiguous actual omission diagnostic')
    pattern = (r'MalformedOperationFoundation \{ machine: ("(?:\\.|[^"\\])*"), '
               r'state: ("(?:\\.|[^"\\])*"), block: BlockId\((\d+)\), '
               r'operation: OperationId\((\d+)\), reason: "' + re.escape(REASON) + r'" \}')
    match = re.fullmatch(pattern, lines[0])
    require(match, 'omission did not reach the exact current membership failure')
    block, operation = int(match[3]), int(match[4])
    require(block > 0 and operation > 0, 'invalid actual failure coordinate')
    intact_text = captured(inspected['stdout'], inspected['stdout_sha256']).read_text(errors='replace')
    operation_lines = re.findall(r'^operation machine=machine:(\d+) block=block:' + str(block)
                                + r' id=operation:' + str(operation) + r' kind=(.+)$', intact_text, re.M)
    require(len(operation_lines) == 1 and operation_lines[0][1].startswith('EstablishReference'),
            'failure does not rejoin one actual intact reference formation')
    machine_id, description = operation_lines[0]
    headers = re.findall(r'^machine id=machine:' + machine_id + r' (.+)$', intact_text, re.M)
    require(len(headers) == 1, 'failure machine has no unique intact owner')
    report.update(test=test, actual_test_suite=ends[0], actual_passes=1,
                  test_image_sha256_after=sha(image),
                  original_test_image_sha256_after=sha(test_preparation['original_image']),
                  test_codegen_after=profile.artifact_codegen(test_preparation['cargo_artifact']),
                  omission_error=lines[0], current_coordinates=dict(machine=json.loads(match[1]),
                      state=json.loads(match[2]), block=block, operation=operation,
                      intact_machine_id=int(machine_id), intact_machine_header=headers[0],
                      intact_operation=description), artifact_sha256_after=sha(artifact),
                  component_observed=True, passed=True,
                  source_use_attribution='pending-independent-current-artifact-review',
                  component_acceptance_complete=False,
                  scope='Only this exact fresh249 intact-to-forwarding-roster-omission component. '
                        'No full-artifact certificate for the semantic counterfactual, no full98 acceptance, '
                        'no native custody or whole-candidate acceptance. Portable owner/operation coordinates '
                        'and the unchanged intact artifact/debug bytes are retained for source-use review.')
    validate()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', 'qualify'])
    parser.add_argument('--repository', required=True, type=Path)
    parser.add_argument('--expected-compiler-source', required=True)
    parser.add_argument('--compiler-input-roster', type=Path,
                        help='reviewed manifest with compiled_source and all_ten_compiler_input_objects')
    parser.add_argument('--compiler-input-roster-sha256')
    parser.add_argument('--capture-binding', type=Path,
                        help='reviewed capture CI, producer, reports, artifact and input binding')
    parser.add_argument('--capture-binding-sha256')
    parser.add_argument('--capture-root', type=Path,
                        help='actual restored exported files, without recreating uncaptured originals')
    parser.add_argument('--out', required=True, type=Path)
    for name in ('preparation', 'results', 'producer-image', 'receiver-image', 'source', 'build',
                 'artifact', 'test-preparation', 'test-image'):
        parser.add_argument('--' + name, type=Path)
    args = parser.parse_args()
    root = args.repository.resolve()
    output = args.out.resolve()
    require(output.is_relative_to(root / 'build/supplied-output-loan'), 'output must be an ignored task directory')
    require(not output.exists(), 'output already exists; do not overwrite prior evidence')
    output.mkdir(parents=True, exist_ok=False)
    report = dict(schema=('omega-installed-empty-codec-test-preparation-v2' if args.mode == 'prepare'
                          else 'omega-installed-empty-current249-component-v2'), passed=False,
                  proposal_mode=args.mode,
                  source_use_attribution='pending-independent-current-artifact-review',
                  component_acceptance_complete=False)
    try:
        _, roster = compiler_inputs(root, args)
        report['compiler_input_roster'] = roster
        if args.compiler_input_roster is not None:
            retained_roster = output / 'reviewed-compiler-input-roster.json'
            data = args.compiler_input_roster.read_bytes()
            require(hashlib.sha256(data).hexdigest() == args.compiler_input_roster_sha256,
                    'reviewed input manifest changed before retention')
            with retained_roster.open('xb') as retained:
                retained.write(data)
            require(sha(retained_roster) == args.compiler_input_roster_sha256,
                    'retained input manifest bytes changed')
            report['retained_compiler_input_roster'] = str(retained_roster)
        sys.path.insert(0, str(root / 'tools'))
        import compiler_profile
        import build_slot
        if args.mode == 'prepare':
            prepare(args, root, compiler_profile, build_slot, output, report)
        else:
            for name in ('preparation', 'results', 'producer_image', 'receiver_image', 'source', 'build',
                         'artifact', 'test_preparation', 'test_image'):
                require(getattr(args, name) is not None, 'missing explicit --' + name.replace('_', '-'))
            qualify(args, root, compiler_profile, build_slot, output, report)
        if args.compiler_input_roster is not None:
            require(sha(retained_roster) == args.compiler_input_roster_sha256,
                    'retained reviewed manifest changed during execution')
            report['retained_compiler_input_roster_sha256'] = args.compiler_input_roster_sha256
        return 0 if report['passed'] else 1
    except BaseException as error:
        report['passed'] = False
        report['error'] = type(error).__name__ + ': ' + str(error)
        return 1
    finally:
        write(output / ('preparation.json' if args.mode == 'prepare' else 'result.json'), report)
        print('INSTALLED_EMPTY_OBSERVATIONS_PASSED', report['passed'],
              'COMPONENT_ACCEPTANCE_COMPLETE', report['component_acceptance_complete'], flush=True)


if __name__ == '__main__':
    raise SystemExit(main())
