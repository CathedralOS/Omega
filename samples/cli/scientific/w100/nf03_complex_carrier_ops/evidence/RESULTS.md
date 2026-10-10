# NF03 verification and remaining frontier

Compiler baseline: `9d4451128dd05885e7b9c591796b2760189851db`.
Source candidate: `c989b3a932` on remote branch
`codex/w100-nf03-complex-20261010`. Evidence commit follows this candidate.
No main landing or qualification acceptance is asserted.

`source /workspace/cloud-setup/activate.sh; mbx build --release -p omega`
completed with exit 0 in 6m 29s. Full output is `rust-build.log`.
Independent exact rational fixtures passed; see `mathematical-fixtures.txt`.
No numerical oracle was evaluated by an Omega executable.

| Project | Check | Native | Terminal comparison | Intended rejection |
| --- | --- | --- | --- | --- |
| phasor_power (F64) | 1, blocked | 200, compile failure | 200, compile failure | not applicable |
| quadrature_rotation (F32) | 1, blocked | 200, compile failure | 200, compile failure | not applicable |
| invalid_alias | 1, wrong diagnostic | not requested | not requested | unqualified |

All final staged project requests report 240 unresolved core floating-conversion
provider satisfactions, starting with `I8::from_f32` not resolving to an exact
trait requirement or top-level boundary requirement. Direct carrier-file check
reports all 240 individually in `carrier-check.log`. Native and Terminal
requests fail during compilation; neither route ran a numerical program.
The alias control's rejection is masked by this earlier failure.

The failure is reduced on unchanged baseline compiler/library sources:

```omega
module scientific::baseline;
use omega::language::core::float_operations;
pub machine finite(value: f64) -> bool { F64::is_finite(value) }
```

`omega --check --target linux_x86_64` on this three-line source exits 1 with
240 errors. Removing only the first line yields exit 0 (25 source files).
Complete source, commands, status and all diagnostics are retained under
`runs/named-module-baseline` and `runs/unnamed-module-baseline`.
A separate plain-source float control passes; a hosted std-only control and a
control with a minimal unnamed dependency reach the expected missing entry
acceptance refusal without these 240 errors. The named-module trigger is
therefore independent of Complex arithmetic. No shared compiler repair or
change to the authored module boundary was attempted.

Environment: sandbox calls reject cache ancestry because `/` is owned by
`nobody`. Approved unsandboxed commands preserve all custody checks and pass
that boundary. Ownership, permissions and guards were never changed.
The first source attempt had constructor syntax errors; corrected sources
use contextual generic constructors. Original sources and logs remain intact.

Next acceptance: restore core floating requirements/provider resolution when
imported from a named module; supply NF01 numerics package metadata; distribute
NF02's actual policy signatures/module/error mapping. Then rerun
`python samples/cli/scientific/w100/nf03_complex_carrier_ops/evidence/qualify.py
--cli target/release/omega --output /tmp/nf03-reviewed` outside the affected
filesystem remapping, and run integrated sample projects with the real NF01
metadata. Require each valid check/native route exit 0, explicit Terminal
agreement, and a loan-conflict rejection for the control. No broad baseline or
other-host acceptance is claimed.

The implementation commit passes ordinary `git diff --check`. Historical
`.omg.txt` snapshots preserve their original final blank lines for exact
reproduction; evidence whitespace checking excludes only blank-at-EOF warnings.
