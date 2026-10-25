#!/usr/bin/env python3
"""Build the observation-only consumer in an ignored private Cargo project.

This sample does not install a Rust application or add a workspace dependency.
The reviewed observe.rs source consumes the repository's existing public APIs.
Compiler packages are selected from this checkout; no worker artifacts are used.
"""
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

sample = Path(__file__).resolve().parents[1]
repository = sample.parents[3]
project = sample / 'build' / 'observer-source'
project.mkdir(parents=True, exist_ok=True)
shutil.copyfile(sample / 'validation' / 'observe.rs', project / 'observe.rs')
shutil.copyfile(repository / 'Cargo.lock', project / 'Cargo.lock')
packages = {
    'omega': 'omega-rust/omega/application/cli',
    'omega-build': 'omega-rust/omega/application/build',
    'compiler': 'omega-rust/omega/compilation/driver',
    'terminal-runtime': 'omega-rust/omega/application/terminal-runtime',
    'package-manager': 'omega-rust/omega/packages/package-manager',
    'target': 'omega-rust/omega/compilation/backend/target',
    'terminal-interpreter': 'omega-rust/psi/interpreter/terminal-interpreter',
}
manifest = '''[package]
name = "ind02-observe"
version = "0.1.0"
edition = "2024"
[workspace]
[[bin]]
name = "ind02-observe"
path = "observe.rs"
[dependencies]
'''
manifest += ''.join(f'{name} = {{ path = {json.dumps(str(repository / path))} }}\n'
                    for name, path in packages.items())
manifest += '''[profile.dev]
debug = 0
[profile.dev.package.proof-admission]
opt-level = 3
[profile.dev.package.sha2]
opt-level = 3
'''
(project / 'Cargo.toml').write_text(manifest)
target = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else sample / 'build' / 'observer-target'
environment = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='4')
tool = 'mbx' if shutil.which('mbx') else 'cargo'
subprocess.run([tool, 'build', '--manifest-path', str(project / 'Cargo.toml')],
               env=environment, check=True)
print(target / 'debug' / ('ind02-observe.exe' if os.name == 'nt' else 'ind02-observe'))
