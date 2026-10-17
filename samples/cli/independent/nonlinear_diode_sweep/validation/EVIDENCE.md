# NEW031 execution evidence

Completed independently in `sample/new031`, isolated at
`/workspace/Omega/.codex/worktrees/new031`. The original checkout was preserved.
No worker, unfinished numerics library, remote worker branch, or shared Omega
export was used. The candidate is for independent review, not remotely landed.

## Source and executor identities

- Refreshed main base: `1987794a0a57db86c6917b8837264e9913e8f35d`.
- Oracle freeze commit: `24ffe84b427c9b677560da2a0330cdbb60153ddc`, authored
  `2026-10-11T00:33:24Z`, before any Omega check or execution.
- Frozen oracle SHA-256:
  `4c0b480ea3f4b73c4993978b407acac212603c381d137d529637eabcda1653d6`.
- Repaired CLI SHA-256:
  `6a226562d461dace6c5a6a12c4fe2f1bb57a4be61579b821b601fe20866de4ed`.
- Fresh unmodified-main CLI SHA-256:
  `63d5b8ddac2b83ab1466facf8eca6fdf8308277180e3b1ec2efcdf259d76a8f2`.
- Private Terminal observer SHA-256:
  `2b5c89bbeb287b14efbbbcf0e5c6132b221be18d8be11ba3a85d58819d930199`.
- Linux native application SHA-256:
  `80e4d6964e629a9bd18299d8b0cfe3c8b82544e4384ce245188875cea148582c`.
- Captured canonical Terminal artifact SHA-256:
  `594bd4acdcebca537368f0050aa9d19760cacd2d0842970d7229e3dede3a7a36`.

Host: Linux x86_64, kernel 6.18.44, glibc 2.41, 5 CPUs, approximately 17 GiB RAM.
Rust: `1.100.0-nightly (a69a63265 2026-09-03)`.
Rust builds used four jobs; Omega compilation/execution was serial. There was
at most one Rust build and one Omega execution concurrently. Target/cache root:
`/workspace/cloud-setup/cache/mbx/targets/v1/3b190584293237d7cc00904a02ee999c8968edb76253fa3ed65523ebbcee1f50`.

The default sandbox compiler route rejected synthetic root ownership (UID
65534) before source checking. Individually approved `require_escalated`
commands retained UID 1000 and observed real root UID 0. No ownership, custody,
proof guard, or permission policy was changed. Future executors must obtain
their own supported context; this record grants no blanket approval.

An executor-disconnect notification was checked against the same workspace:
clean checkpoint `69b7064597`, unchanged oracle, preserved PID 27764/session
33300. The operation remained observable and completed. `recovery.json`
records the read-only readiness evidence; recovery changed no permissions.

## Outcomes

| Route | Observation |
| --- | --- |
| Application check | Exit 0 with repaired CLI. |
| Native | All 21 input records passed the frozen Decimal checker; 54 operating-point rows, including converged, exhausted and unbracketed states. |
| CLI native/Terminal | Mixed sweep: native exit 0 and `interp exit: 0`. CLI exposes native stdout only. |
| Full Terminal streams | All 21 records passed the same frozen Decimal checker. All stdout streams were byte-identical to native; stderr was empty. |
| Intended-invalid source | Exit 1, exactly ``cannot prove index `2` is within length 2``. Package, parser, entry-slot and other failures are not passes. |
| New IEEE arrival fixture | Checked; native exit 70 and Terminal exit 70, agreement. Includes structural-prefix scalar positions, f32/f64 comparisons and NaN rejection. |
| New invalid arrival fixture | Rejected for the unguarded `bounded` state requirement. |
| Existing comparison/contract controls | Five named fixtures retained their expected rejection diagnostics. |
| Rust checks | `mbx check --locked -p typed-trees-to-checked-trees -p omega -j4` passed. |
| Formatting | Edited Rust files and private observer passed. Crate-wide check reports only the unchanged main import layout in `execution/terminal_unit/control/statement_sequence.rs`; that file was preserved. |

Validation is scoped to the actual Linux host. macOS and Windows entry bindings
are declared but their runtime routes were unavailable here. No full-corpus or
cross-platform runtime claim is made. Historical corpus performance numbers
came from a different host and job count; the recorded timing difference is
not a regression conclusion. Existing golden rows were preserved byte for
byte, with only the two new check rows and one new runtime row appended.

## Real compiler gap and bounded repair

The original application reached `Protocol::number`'s IEEE arrival requirement
and failed checked-body production with `state signature: predicate lowering
failed`. The complete managed `samples/cli/facts/repro` source reproduced that exact
failure on freshly built, unmodified main: `inspect-terminal` exit 1. With the
repair, the identical source produced a Terminal inspection artifact, exit 0.
Earlier unmanaged inspection passed the first repaired boundary but lacked an
exact declaration owner; managed build metadata supplies the ordinary owner.

The repair is in the existing scalar contract owner at
`04_typed-trees-to-checked-trees/src/values/scalar/result_contract.rs`, with its
existing crash-contract consumer in `contract_entry/crash_entry.rs`. Arrival,
entry, result and crash predicates now share authenticated IEEE comparison
construction. Exact float format, literal bits, selected builtin/sealed meaning,
subject positions and unordered semantics are retained. Unsupported spellings
and lookalikes acquire no authority. Integer proof and custody rules remain.

Native validation also found endpoint-subtraction cancellation in the reported
bracket width. The sample now carries the nominal width through binary halvings.
The oracle, tolerances, budget and residual threshold were never relaxed.
Pre-fix output is retained in `initial/native-before-stable-width`.

## Package review and execution commands

The CLI proposal requested Console stdin/stdout/exit for the application. The
unchanged repository std package additionally declares its existing platform
imports and FilesystemHost/Console capabilities. Exact rows were inspected and
accepted through ordinary review, without `--dangerously-accept-all`.
Subsequent source-only refreshes retained policy decisions. No package
acceptance was denied and no denied acceptance was retried.

The private observer initially refused the CLI's lock because its compiler
binary identity differed. The lock and gate were preserved. Its explicit
`--review` command then performed ordinary fresh source checking and published
for the exact observer binary with existing capability decisions. Its run mode
never edits decisions or falls back to unaccepted inputs. No identity override,
trusted-fixture wrapper, source interpreter or acceptance bypass is present.

From the repository root, after `source /workspace/cloud-setup/activate.sh`:

```sh
sample=samples/cli/independent/nonlinear_diode_sweep
mbx build --locked -p omega --release -j4
mbx check --locked -p typed-trees-to-checked-trees -p omega -j4
mbx test --locked --no-run -p compiler --test corpus_runner -j4
target/release/omega update --project "$sample" --target linux_x86_64
# Inspect exact decisions; resume only a pending reviewed proposal:
target/release/omega update --resume --project "$sample"
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega "$PWD/target/release/omega" --route check
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega "$PWD/target/release/omega" --route native --case mixed_bias
# Reuse the verified retained omega-program path recorded by --keep:
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega "$PWD/target/release/omega" --route native --native-executable RETAINED_EXECUTABLE
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega "$PWD/target/release/omega" --route terminal --case mixed_bias
PYTHONDONTWRITEBYTECODE=1 python3 "$sample/validation/run.py" --omega "$PWD/target/release/omega" --route invalid
target/release/omega inspect-terminal --machine Main::main --target linux_x86_64 "samples/cli/facts/repro/main.omg"
```

The actual observer was built in an ignored `build/new031-terminal-driver`
directory using identical source and dependency versions, with `--target-dir
"$PWD/target"`. Its source is delivered in `validation/terminal-driver`; the
README gives the portable manifest commands. Rebuilding changes its binary
identity and requires ordinary review again. The precise execution commands,
exits and streams are in the route records and observer logs.

Focused corpus commands used the verified runner
`target/debug/build/compiler/3549256695963475/out/corpus_runner-3549256695963475`
with `OMEGA_CORPUS_JOBS=1`, filters `state_arrival_ieee` for check and
`state_arrival_ieee_predicates` for `--both`. Detailed reports are
`corpus-arrival-check.json` and `corpus-arrival-both.json`. The five existing
controls are listed in `corpus-check.json`. A positive fixture authoring error
was corrected to use identical guard/contract facts before successful recording.

## Review artifacts and remaining limits

The handoff archive contains an incremental Git bundle, complete base-to-candidate
patch, changed source snapshot, per-file SHA-256 manifest, this evidence,
frozen oracle, all native/Terminal streams, compiler-gap logs, package proposal
receipts and the verified Linux application executable. Compiler/observer
binaries and mutable lock/admission state are omitted; consumers review their
own exact sources and compiler identities.

No application or package-review blocker remains. Coordination registration
was unavailable: ticket `a9fcd2933c834cc894d0eec1ba450d9b` timed out and subsequent
status was unavailable. `ownership.txt` records the local scope and bounded
repair; no registration or landing success is claimed. The final archive
manifest records the exact candidate commit and transfer hash. Library transfer
verification is recorded separately after archive creation.
