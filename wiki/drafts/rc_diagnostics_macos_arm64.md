# RC diagnostics — macos_arm64 row

Witnessed row of the `RC-DIAGNOSTICS` release gate on macOS AArch64. First
run of this gate on any host other than Linux. Recorded at revision
`c3e3bfec3542` (2026-09-21), host `Darwin` / `arm64` (Apple M4, 16 GB),
`rustc 1.100.0-nightly (a69a63265 2026-09-03)`, `cargo-nextest 0.9.143`
(mbx unavailable).

The gate command in `wiki/drafts/rust_compiler_completion.md` names
`proof_and_float_suites::fail_canaries_reject_with_expected_diagnostic_fragment`;
the test lives one module deeper, as the linux row also records:

```
cargo nextest run -p compiler --test canary_suite \
  proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment
```

## Verdict

**Green after one fixture-portability repair.** The first macOS run was
**red** — 1 drifted canary in 90.0 s, against a linux row that is green at
`72fc66d6c326` with zero drift. The single difference was host-dependent,
not a compiler regression, and the repair is in this commit.

## The drift was host target selection, not diagnostics

`tests/omega/fail/ports/asm_port_in_unsettled` pins the fragment
`local construction stopped at statement sequence: call: call operation`
— a lowering wall reached only after root binding succeeds. Its
`build.omg` binds one root:

```
builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
```

On a Linux x86-64 host the selected target is already `linux_x86_64`, so
the compile reaches the pinned wall and the canary passes. On this host the
selected target is `macos_arm64`, which has no bound root, so compilation
stops much earlier with

```
error: selected target `macos_arm64` has no bound required root slot `macos_arm64::ProgramEntry`
```

and the pinned fragment never appears. The fixture is correct; what was
missing was the annotation that tells the harness which target to compile
it for. This is invisible on Linux by construction — a gate that can only
be red off-host.

## Repair

Added `("ports/asm_port_in_unsettled", "linux_x86_64")` to
`CROSS_TARGET_PRODUCTION_FAIL_CANARIES` in
`omega-rust/omega/compiler/compiler/tests/fixture_rosters/proof_and_float_suites.rs`.
That roster is the existing, documented mechanism for exactly this shape —
its doc comment already says these canaries "bind a non-native
`ProgramEntry`, so they carry an explicit target", and both existing
entries are fixtures that bind only `linux_x86_64::ProgramEntry`. The
*production* roster rather than `CROSS_TARGET_FAIL_CANARIES` because this
refusal lives behind checked semantics at the lowering wall, which the
Terminal-artifact route reaches and a `Check` stop does not.

The entry is a no-op on a Linux x86-64 host beyond pinning the target
explicitly, so the linux row's green result is unaffected.

Verified by control, not by assumption:

| run | roster entry | result |
|-----|--------------|--------|
| `OMEGA_FAIL_CANARY_FILTER=asm_port_in_unsettled` | present | **1 passed** in 7.9 s |
| same filter | removed | **1 failed**, naming this fixture and the `macos_arm64` root-slot error |

Removing the entry reproduces the exact original diagnostic, so the entry
is what closes the drift.

## Row result

With the entry in place the full gate is **green on macos_arm64**:
`fail_canaries_reject_with_expected_diagnostic_fragment` PASSes in 91.2 s
across the whole fail corpus, zero drift. That matches the linux row's
109.3 s green at `72fc66d6c326`.

## Two unrelated roster reds on this host

`roster::registered_fail_canaries_have_source_and_their_owned_expectations`
and `roster::registered_pass_canaries_have_source_on_every_host` both fail,
naming fixtures that exist on disk with no roster owner:
`domains/predicate_domain_local_initializer_unproved`,
`proofs/quotient_lift_invalid_law_rejected`,
`proofs/quotient_lift_nonhermetic_identity_rejected` and
`domains/predicate_domain_local_initializer_established`.

These are **not** this row's and not this change's: both tests fail
identically with the change stashed at the same revision. They belong to
whichever lane authored those four fixtures. They are recorded here only so
the next worker on this host does not re-diagnose them.

## Note for RC-DIAGNOSTICS-CLOSURE

That item is owned (`Zergling-187 / rc-diagnostics-closure`) and its board
row records the linux measurement only. This row does not touch it. The
harness file `canary_suite.rs` — which holds `ACTIVE_FAIL_CANARIES` and the
sibling `CROSS_TARGET_FAIL_CANARIES` — is fenced to
`PROOF-SUBJECT-CHECKED-CALL-ATTRIBUTION` and was not edited; the repair
needed only the unfenced roster file, and the roster self-test that
requires every cross-target annotation to have an executing roster entry
passes because the fixture was already in `ACTIVE_FAIL_CANARIES`.
