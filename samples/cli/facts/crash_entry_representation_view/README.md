# Crash-entry integer representation views

Owning requirement: **CRASH-CONTRACT**, entry-operand normalization.

A checked equal-width integer view preserves the authenticated source operand's
bits through a guarded scalar helper. The source's entry equality must discharge
the helper's zero-crash route without adding a caller crash ceiling or replacing
the helper's published guard.

`main.omg` retains the frozen failing witness's semantics (with the current
`public` spelling): an entry `u64` equal to 7 is viewed as
`i64`, copied, and passed to `maybe_zero`. On main `53cd2938437a29356a48d6eaf29f14de07a4bb99`
it rejected with an uncovered `Trap` route at statement 2. The same-carrier
control executed native/Terminal exit 0. Initial cache-ownership and malformed
reference-argument probes were setup failures, not this semantic baseline.

The acceptance command is `omega run --both main.omg`; every positive declares
exit 0 and binds all four hosted targets. Matching Linux x86-64 execution is
recorded separately from unexecuted Windows, macOS and Linux ARM64 products.

The witnesses select distinct obligations, not a count matrix:

- Root: parameter → primitive view → immutable copy → guarded helper.
- `high_bit`: `u64::MAX` reads as `i64(-1)`, preserving bit interpretation.
- `reverse_signed`: `i32(-1)` reads as `u32::MAX`.
- `field`: a shared record field retains its exact entry projection.
- `call_result`: a normal-result equality forwards the viewed snapshot.
- `other_state`: an authenticated arrival retains the original entry operand.
- `snapshot_after_write`: the copied view stays 7 after its original storage is overwritten.
- `surviving_guard`: a nonclosed route remains permitted and executes normally for input 7.
- `view_then_widen`: signed interpretation happens before value-preserving widening.

`controls/changed_guard` changes the helper's consistent contract/body guard to
7; `controls/stale_storage` overwrites the source before observing it. Both must
reject the uncovered call route. `controls/incompatible_view` must reject a
`u8` → `bool` view whose source representations do not establish Boolean validity.
These are pinned with the positives under `tests/omega/{pass,fail}/facts/crash_entry_integer_view_*`.

The governing contracts are [recasts](../../../../wiki/spec/layouts/recasts.md),
[guarded crashes](../../../../wiki/spec/language/effects.md#guarded-crashes), and
[entry identity/call composition](../../../../wiki/spec/terminal-psi/calls_and_outcomes.md),
and [snapshot identity](../../../../wiki/spec/language/state_contracts.md).
Checked recast admission still establishes geometry, representation compatibility,
authority and custody. Crash identity retains both integer carriers and recursively
authenticates the source; exact entry equations interpret only closed source bits.
Writes invalidate current storage provenance; completed scalar snapshots retain
their earlier meaning. No proof-only value authorizes an executable read.

Source checking, checked-to-Terminal lowering, ordinary calls/ABI placement,
bit-image instruction selection and ELF execution are exercised by the same
programs. Existing representation-view realization is reused; no new provider,
layout policy, cleanup rule, external dependency or target-specific semantic rule
is introduced. Boolean/IEEE cross-family crash views, other entry normalization,
generic structural crash calls, outcomes and broader CRASH-CONTRACT acceptance
remain open.

The focused 97-case comparison against `ec3c24b634dd7933829f534b5fb541e63df452d7`
had zero losses and twelve new expected outcomes. Its existing baseline refusals
are retained rather than rewritten. The checked and Linux both-engine golden rows
retain their actual producer revision; integration replay uses a fresh producer.
