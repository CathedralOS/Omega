#!/usr/bin/env python3
"""Capture Terminal bytes through the public runtime, with normal package admission.

Build the private host once, then replay all five frozen streams serially against
one retained native report's genuine Terminal artifact. No package acceptance,
compiler changes, expected-output provider, or checked-tree fallback is supplied.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parent
HOST = r'''
use std::{fs, io::Cursor, path::PathBuf};
use terminal_runtime::{execute, TerminalDomainExitDelegation, TerminalRuntimeRequest};
use package_manager::operations::{prepare_local_project, LocalProjectPreparationOptions,
    compile_prepared_local_project_for_native, PreparedLocalProjectNativeRequest};
use terminal_interpreter::TerminalExecutionStatus;

fn validate() -> Result<(), String> {
    let root = PathBuf::from(std::env::args().nth(1).ok_or("missing sample root")?);
    let output = root.join("build/terminal-validation");
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let target = target::TargetProfile::LinuxX64;
    let prepared = prepare_local_project(&root.join("main.omg"),
        LocalProjectPreparationOptions { target, offline: true })
        .map_err(|e| e.to_string())?.ok_or("expected an ordinary package project")?;
    let report = compile_prepared_local_project_for_native(
        PreparedLocalProjectNativeRequest::new(prepared, output.join("production"), target))
        .map_err(|e| e.to_string())?;
    let artifact = report.retained_native_artifact().map(|a| a.psi_artifact())
        .or_else(|| report.retained_dynamic_elf_artifact().map(|a| a.psi_artifact()))
        .ok_or("native report lost its genuine Terminal artifact")?;
    let original = artifact.semantic_bytes().to_vec();
    fs::write(output.join("artifact.psi"), &original).map_err(|e| e.to_string())?;
    for (name, expected_exit) in [("exhaustive", 0), ("empty", 0),
        ("truncated", 0), ("oversized", 3), ("limit", 3)] {
        let input = fs::read(root.join(format!("data/{name}.stdin"))).map_err(|e| e.to_string())?;
        let observed = execute(TerminalRuntimeRequest {
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64").expect("host flags capture"),
            artifact, target: Some("linux_x86_64"), provisions: report.runtime_provisions(),
            provider_proposal: None,
            // Finite exhaustive replay: fixed 2000-record cap, 25 candidates per frame.
            fuel_allowance: 100_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(input))), clock: None,
        }).map_err(|e| format!("{name}: {e}"))?;
        fs::write(output.join(format!("{name}.stdout")), &observed.stdout).map_err(|e| e.to_string())?;
        fs::write(output.join(format!("{name}.stderr")), &observed.stderr).map_err(|e| e.to_string())?;
        fs::write(output.join(format!("{name}.status")), format!("{:?}\n", observed.status))
            .map_err(|e| e.to_string())?;
        let exit = match observed.status {
            TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
            status => return Err(format!("{name}: incomplete/unsupported outcome {status:?}")),
        };
        let expected = fs::read(root.join(format!("data/{name}.stdout"))).map_err(|e| e.to_string())?;
        if exit != expected_exit || observed.stdout != expected || !observed.stderr.is_empty() {
            return Err(format!("{name}: frozen stream/exit oracle mismatch (exit {exit})"));
        }
        if artifact.semantic_bytes() != original {
            return Err(format!("{name}: retained artifact changed"));
        }
        println!("{name}: Terminal stdout/empty stderr/exit match; artifact unchanged");
    }
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new().stack_size(256 * 1024 * 1024)
        .spawn(validate).expect("compiler worker").join().expect("compiler worker panic");
    if let Err(error) = result { eprintln!("{error}"); std::process::exit(1); }
}
'''

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true', help='write private host sources only')
    parser.add_argument('--build', action='store_true', help='serially build host and retain receipt')
    parser.add_argument('--target-dir', help='Cargo target directory for --build')
    parser.add_argument('--host', help='run this already-built host, without another Rust build')
    args = parser.parse_args()
    oracle = json.loads((ROOT/'data/oracle.json').read_text())
    for name, digest in oracle['sha256'].items():
        assert hashlib.sha256((ROOT/'data'/name).read_bytes()).hexdigest() == digest, name
    repository = ROOT.parents[3]
    project = ROOT/'build/terminal-host'
    (project/'src').mkdir(parents=True, exist_ok=True)
    dependencies = {'compiler': 'omega-rust/omega/compilation/driver',
                    'terminal-runtime': 'omega-rust/omega/application/terminal-runtime',
                    'package-manager': 'omega-rust/omega/packages/package-manager',
                    'target': 'omega-rust/omega/compilation/backend/target',
                    'terminal-interpreter': 'omega-rust/psi/interpreter/terminal-interpreter'}
    manifest = '[package]\nname="new084-terminal-host"\nversion="0.1.0"\nedition="2024"\n[workspace]\nresolver="3"\n[dependencies]\n'
    manifest += ''.join(name+'={path='+json.dumps(str(repository/path))+'}\n'
                        for name, path in dependencies.items())
    # Match the documented repository dev profile and pin the existing lock.
    manifest += '\n[profile.dev]\ndebug=0\n[profile.dev.package.proof-admission]\nopt-level=3\n[profile.dev.package.sha2]\nopt-level=3\n'
    (project/'Cargo.toml').write_text(manifest)
    if not (project/'Cargo.lock').exists():
        shutil.copyfile(repository/'Cargo.lock', project/'Cargo.lock')
    (project/'src/main.rs').write_text(HOST)
    if args.prepare:
        print(project/'Cargo.toml')
        return
    if args.build:
        if not args.target_dir:
            parser.error('--build requires --target-dir')
        env = os.environ.copy()
        env.update(CARGO_TARGET_DIR=str(Path(args.target_dir).resolve()),
                   CARGO_BUILD_JOBS='2', CARGO_PROFILE_DEV_OPT_LEVEL='1')
        command = [shutil.which('mbx') or 'cargo', 'build', '--offline',
                   '--manifest-path', str(project/'Cargo.toml')]
        started = time.monotonic()
        with (project/'build.log').open('wb') as log:
            compiled = subprocess.run(command, env=env, stdout=log, stderr=subprocess.STDOUT)
        if compiled.returncode:
            raise SystemExit(compiled.returncode)
        binary = Path(args.target_dir).resolve()/'debug/new084-terminal-host'
        receipt = {'command': command, 'exit': compiled.returncode,
                   'duration_seconds': time.monotonic()-started,
                   'source_sha256': hashlib.sha256((project/'src/main.rs').read_bytes()).hexdigest(),
                   'manifest_sha256': hashlib.sha256((project/'Cargo.toml').read_bytes()).hexdigest(),
                   'lock_sha256': hashlib.sha256((project/'Cargo.lock').read_bytes()).hexdigest(),
                   'toolchain_sha256': hashlib.sha256((repository/'rust-toolchain.toml').read_bytes()).hexdigest(),
                   'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                   'binary': str(binary),
                   'repository_head': subprocess.check_output(['git','rev-parse','HEAD'], cwd=repository).decode().strip()}
        (project/'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
        print(json.dumps(receipt), flush=True)
        return
    if not args.host:
        parser.error('use --prepare, build the private host serially, then supply --host')
    receipt = json.loads((project/'receipt.json').read_text())
    assert receipt['source_sha256'] == hashlib.sha256((project/'src/main.rs').read_bytes()).hexdigest(), 'stale host source'
    assert receipt['manifest_sha256'] == hashlib.sha256((project/'Cargo.toml').read_bytes()).hexdigest(), 'stale host manifest'
    assert receipt['lock_sha256'] == hashlib.sha256((project/'Cargo.lock').read_bytes()).hexdigest(), 'stale host lock'
    assert receipt['binary_sha256'] == hashlib.sha256(Path(args.host).read_bytes()).hexdigest(), 'unverified host binary'
    assert receipt['toolchain_sha256'] == hashlib.sha256((repository/'rust-toolchain.toml').read_bytes()).hexdigest(), 'changed toolchain'
    unchanged = subprocess.run(['git', 'diff', '--quiet', receipt['repository_head'], '--',
                                'omega-rust', 'Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml'],
                               cwd=repository)
    assert unchanged.returncode == 0, 'compiler path dependencies changed after host build'
    output = ROOT/'build/terminal-validation'
    output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env['OMEGA_LIBRARY_ROOT'] = str(repository/'source/library')
    observed = subprocess.run([str(Path(args.host).resolve()), str(ROOT)],
                              env=env, capture_output=True)
    (output/'host.stdout').write_bytes(observed.stdout)
    (output/'host.stderr').write_bytes(observed.stderr)
    results = {'host_exit': observed.returncode, 'oracle_tolerance': oracle['tolerance'],
               'host_sha256': hashlib.sha256(Path(args.host).read_bytes()).hexdigest(),
               'command': [str(Path(args.host).resolve()), str(ROOT)], 'build_receipt': receipt, 'streams': {}}
    if observed.returncode == 0:
        for name in ('exhaustive', 'empty', 'truncated', 'oversized', 'limit'):
            actual = (output/(name+'.stdout')).read_bytes()
            expected = (ROOT/'data'/(name+'.stdout')).read_bytes()
            assert actual == expected, name
            results['streams'][name] = {'stdout_sha256': hashlib.sha256(actual).hexdigest(),
                                        'status': (output/(name+'.status')).read_text().strip()}
        results['artifact_sha256'] = hashlib.sha256((output/'artifact.psi').read_bytes()).hexdigest()
    (output/'results.json').write_text(json.dumps(results, indent=2)+'\n')
    print(observed.stdout.decode(errors='replace'), end='')
    print(observed.stderr.decode(errors='replace'), end='')
    raise SystemExit(observed.returncode)

if __name__ == '__main__':
    main()
