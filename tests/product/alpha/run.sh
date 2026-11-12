#!/bin/sh
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../.." && pwd)
mkdir -p "$root/build"
# A stable ignored project identity lets unchanged dependency/review records
# be reused on the next development run. Parallel runs use separate worktrees.
package="$root/build/product-alpha"
mkdir -p "$package"
# Omega is an application and cannot be imported as a package dependency.
# This gate compiles an exact copy of its emitter with a gate-owned entry;
# it does not retain a second emitter implementation or edit the product entry.
cp "$here/main.omg" "$package/main.omg"
cp "$root/source/omega/alpha/emitter.omg" "$package/emitter.omg"
python3 - "$root" "$package" <<'PY'
import json
from pathlib import Path
import sys
root, package = map(Path, sys.argv[1:])
lines = ['machine build(builder: &mut Build) {',
         '    Build::application(builder, "omega_alpha_gate");']
for target in ('linux_x86_64', 'linux_arm64', 'macos_arm64', 'windows_x86_64'):
    lines += [f'    Build::select_provider<omega_language_std::Console, omega_language_std::NativeConsole>(builder, {target});',
              f'    Build::bind(builder, {target}::ProgramEntry, main);']
for alias, path in [('psi', root / 'source/psi'), ('std', root / 'source/library/std')]:
    lines += [f'    let mut {alias}_dependency: DependencyDescription = dependency::from_source(Source::Path {{ location: {json.dumps(str(path))} }});']
    if alias == 'psi':
        lines += ['    dependency::with_alias(&mut psi_dependency, "psi");']
    lines += [f'    let {alias}_registration: DependencyRegistration = Build::dependency(builder, {alias}_dependency);']
lines += ['}']
(package / 'build.omg').write_text('\n'.join(lines) + '\n')
PY
status=0
sh "$here/../gate.sh" alpha "$package" "$here/test.py" "$@" </dev/null || status=$?
printf 'Alpha product gate retained at %s\n' "$package" >&2
exit "$status"
