# Validation checkpoint

NEW073 is authored and independently reviewed, but runtime acceptance and main
publication are blocked. No native or Terminal result is claimed.

The starting main revision was `b4008bb3c7894aebad47eaccf8af12bb4a8006b1`.
The preserved unchanged release compiler has SHA-256
`ec44090c516c5d371dc74028234ff2fe20b8ea279e33fe461dcd51388e327a07`.
The repaired release compiler has SHA-256
`1ddd6d26ff65a1886d9f75969afc30de0c4cee0354816b49ef103658f0b3cf6a`.
Both used the pinned Rust toolchain, x86_64-unknown-linux-gnu host, target
linux_x86_64, three build jobs, five executor CPUs, and 17 GiB RAM. Only one
Rust build and one Omega operation ran at a time. The private Cargo target was
`/workspace/scratch/new073-target`.

The default sandbox exposes synthetic unrelated ownership of `/` and fails
cache custody. Approved per-command compiler execution with `require_escalated`
retained UID 1000 and real root ownership. No custody checks or ownership were
modified. The commands below used that supported context where necessary.

## Witnessed compiler repair

The original complete application and the complete control under
`controls/float_conversion_resolution/` exposed 240 unresolved named float
conversion satisfaction diagnostics with the unchanged main compiler. A direct
root float import checked successfully; the domain-module import exposed the
family resolution error. The bounded Psi change retains a visible overloaded
operator family until existing typed signature settlement selects the exact
parameter and result policy. It does not select an arbitrary overload or change
library exports. The compiler repair and general regression fixture accompany
this sample checkpoint.

Release build and scoped release check exited zero. A final scoped dev check
also exited zero:

```
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=/workspace/scratch/new073-target mbx build --release -p omega
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=/workspace/scratch/new073-target mbx check --release -p syntax-trees-to-symbol-resolved-trees -p omega
CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=/workspace/scratch/new073-target mbx check -p syntax-trees-to-symbol-resolved-trees -p omega
```

After authoring fixes, package preparation exited 3 with two source packages
checked and pending review, rather than the original compiler diagnostics:

```
omega update --project samples/cli/independent/heat_exchanger_balances --target linux_x86_64
```

The final source-check command after the reporting correction exited 1 solely
at the documented program-entry acceptance guard:

```
omega --check --target linux_x86_64 samples/cli/independent/heat_exchanger_balances/main.omg
```

Its diagnostic says the dependency's `LinuxX86_64Application` boundary schema
needs an accepted program-entry binding. This is a blocker, not a passing check.

## Actual package-acceptance denial

Automatic approval review rejected the first proposed acceptance/resume command
before it ran. Its stated reason was blanket acceptance of pending decisions,
including broad std filesystem/process capabilities, without explicit
authorization for each capability or confirmation that all are required.
No package decisions were changed, no acceptance succeeded, and no denied
acceptance was retried or bypassed.

The pending review comparison is
`e6c59fcd795749e532fc6fd70dfe7fa146e1f446d7d8f3659abcb829e9387c12`.
It contains 286 decisions: the application has three terminal permission rows
(Console read_byte/input, write_byte/output, exit_process/termination) and a
Console process declaration. The unchanged std package has 140 callable and
140 external-supply rows plus FilesystemHost/filesystem and Console/process
declarations. The rejected command mistakenly asserted a count of 281; it was
never executed. The actual review count is 286. The broader declarations are
std package surface; this application requests no filesystem operation.

Exact package identities in that review:

- application: `f2f7bef454299022d8fca18165281a1682db5a6a6adee0a5ac675cd33768e392`
- std: `e2f1efffe3cf03efc7448e989b6138389fc654f8f858160a6b407ad9af5bb52d`

The generated source rendering hit its resource limit for both packages.
Independent review directly inspected the application and std Console Linux
closure; std sources were unchanged from the base. The report correction is a
later application source edit, so the pending review must also be refreshed and
examined once explicit authorization permits resuming this action.

Native compilation, 28 input executions, Terminal comparison, the native
regression fixture, and main-only landing are pending that authorization. No
alternate provider, deleted entry binding, package-guard change, or fallback
execution was used to evade the denial.

## Oracles and independent review

The original oracle was frozen before any Omega invocation and remains SHA-256
`fba8b0b7e3bde8a7795425d6691f1cdeeb64d29174fcd32fe22ea9d4c241a32c`.
The supplemental oracle was frozen before any Omega execution of its inputs:
`ecf49ffc955a53786b61c69a27ffcf3a2199f28c49f026f9cc6628f95fff248a`.
Both are retained without post-execution expectation changes.

Independent review verified all 28 input hashes, all 13 exact valid rational
solutions and stdout oracles, and supplemental byte/token boundaries. It found
the cold-outlet exact tie 225/16 for `1 1 1 9 1 17 100 0 3`; the final report
policy treats computed values within the existing 1e-7 scientific tolerance of
half-milli thresholds as ties. This resolves the static binary64 adversary
without changing the exchanger equations, balance tolerance, or original
expected bytes. Static review approved the correction, but does not establish
native execution acceptance.

The complete intended-invalid bounds control rejected on the unchanged base
with exit 1 and the exact intended reason:
`cannot prove index '9' is within length 9`. It was separately checked again
with the repaired compiler; retained logs identify the result.

Claims coordination was unavailable. Ticket
`7145ec93523f48cdbb32e01f54c4b476` is locally recorded, registration unconfirmed.
No publication claim was acquired and no remote worker branch was pushed.
Review bundle evidence preserves exact command outputs and source hashes.
