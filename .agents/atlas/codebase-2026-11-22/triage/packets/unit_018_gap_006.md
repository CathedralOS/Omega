# Packet unit_018_gap_006: x86-64 selected-form encoder, lines 19001-21917

The second-pass record for the tail of the x86-64 selected-form encoder found no wrong result, and this review confirms that: every claim is intended behavior, a duplicate of an earlier disposition, or a test-only duplication with no wrong output. No encoder code changed. Running the owning tests exposed a separate build break on `main` (B1 below), which this packet repairs. The record is [unit_018_gap_006](../../records/unit_018_gap_006.md); it covers lines 19001-21917 of the stage 11 encoder (area 11). About 2,650 of those lines are `#[cfg(test)]` code, and the production code is the `trapping_forms` shim, `canonical_effects_encoding`, the public types and the two public entry points.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. The file is unchanged between that baseline and `a0152d26`. It has 21,918 lines; the record counted 21,917, so production-tail line numbers below are the record's plus one where they differ.

`X64sfe` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`.

Merged evidence checked first: [area 07-13 ledger](../07-13-psi-and-native.md) row "11, x86 memory immediate and width diagnostics" (repaired in #5392, the last change to `X64sfe`), and [digest 11](../../digests/11-native-stages-11-15.md) §2, §9 and §10 item 13. No open PR or claim overlaps `X64sfe` lines 19001-21918.

## Claims

### C1. `trapping_form` returns `None` for other kinds
- Source: record §"21655-21671 production module `trapping_forms`", "Fail-open" and "What I am unsure about" item 3; digest 11 line 160.
- Code path: `trapping_forms::trapping_form` (`X64sfe:21665-21670`), called only by `decoding::footprint` (`X64sfe:2989`).
- Contract: `footprint` must give every kind its declared effects.
- Trace: after the early return for trapping forms, `footprint` matches `kind` exhaustively with no `_` arm, and the `Kind::TrappingInteger { .. }` arm is `unreachable!("trapping forms handled above")` (`X64sfe:3084-3086`). A `None` only means "not a trapping kind"; control reaches the IEEE, atomic and per-kind arms.
- Outcome: **intended**. A kind filter, not a fail-open. No fix.

### C2. The validator accepts a different scratch register when a trapping form does not use scratch
- Source: record §"19776-20677 mod trapping", substitution test `X64sfe:20064`/`:20106`, and "Fail-open summary".
- Code path: generic validate arm (`X64sfe:21885-21898`) recomputes `encode_unchecked` with the supplied operands and requires byte equality. For shapes `OverflowFlag`, `CarryFlag`, `ShiftRight` and `Convert(None)` (`X64sfe:19964-19972`) the encoder never names the scratch operand, so the bytes do not depend on it.
- Contract: the footprint must cover every register the bytes write. `trapping_footprint` (`X64sfe:2937-2949`) takes reads and writes from `TrappingShape::operand_reads_and_writes` (`targets/target-x86_64/src/form_effects/trapping_forms.rs`), which always lists the scratch slot as written (`vec![2, 3]` or `vec![1, 2]`).
- Trace: the accepted program writes nothing to the substituted register, and the footprint still declares it written. The declaration over-approximates; no consumer reads the scratch slot as a result.
- Fixture: `trapping_forms_reject_every_mutation_substitution_and_wrong_request` (`X64sfe:20064`).
- Outcome: **intended**. Sound over-approximation. No fix.

### C3. Different trapping forms accept each other's bytes when their shape is equal
- Source: record §"19776-20677 mod trapping", test `trapping_bytes_replay_only_under_a_form_of_the_same_shape` (`X64sfe:20239`) and "Five things" item 4.
- Code path: the same generic byte-equality validator; `TrappingShape::of(form)` decides the bytes and the footprint.
- Contract: equal bytes are acceptable only when the forms have the same meaning on the 64-bit registers they receive.
- Trace: the test's explicit groups (`X64sfe:20276-20306`) are: unsigned divide and remainder for u8, u16, u32 and u64 (zero-extended operands give the same quotient, remainder and divide-by-zero trap); signed and unsigned sources converted to the same narrow unsigned carrier (one unsigned compare against the carrier maximum decides both, because a negative signed source is a large unsigned value); i64-to-u64 with u64-to-i64 (both trap when bit 63 is set); and the two identity conversions. The test asserts equal `TrappingShape` for every accepted pair (`X64sfe:20320`), so their footprints are equal too. `trapping_decoded_forms_trap_exactly_when_the_policy_predicate_holds` (`X64sfe:20524`) runs every form's bytes against that form's own reference (`trapping_reference`, `X64sfe:20325`).
- Outcome: **intended**. No fix.

### C4. `encode` and `validate` dispatch different kinds
- Source: record §"21841-21898", "Asymmetry", "Five things" item 3 and "What I am unsure about" item 2.
- Code path: `validate` returns early for `RegisterAccess` and `PortWrite` (`X64sfe:21852`, `:21855`); `encode` sends both to `encode_unchecked`, which calls `register_access::encode` (`X64sfe:12142`) and `x86_64_encoding::encode_immediate_port_write` (`X64sfe:12153`). `port_write` has only `validate` (`X64sfe:7444-7466`).
- Outcome: **duplicate** of digest 11 §10 item 13 and §9 ("x86 encode vs validate dispatch"). Rechecked at the baseline; the lines moved by one. No fix.

### C5. `canonical_effects_encoding` looks up the view named `"condition_flags"`
- Source: record §"21719-21749 canonical_effects_encoding", "Decides on"; digest 11 §2 "Spelling of register view names".
- Code path: `X64sfe:21720-21750`. Its four callers (`X64sfe:1407`, `:1543`, `:10459`, `:10606`) first call `byte_loop_registers` (`X64sfe:1372`, `:1600`, `:10478`, `:10636`), which calls `guard_canonical` (`X64sfe:16066-16073`); that refuses any model whose identity differs from the canonical x86-64 model.
- Trace: the lookup runs only on the canonical model, which declares the view; a missing view still returns `NonCanonicalPhysicalModel` instead of a default.
- Outcome: **intended**. The spelling-keyed lookup is the code-quality pattern already counted in digest 11 §2; no wrong result. No fix in this packet.

### C6. `canonical_effects_encoding` slices `operands[..read_count]`
- Source: record §"21719-21749", "Fail-open" ("callers pass fixed counts (not traced)").
- Trace: `byte_loop_registers::<N>` converts the resolved operands with `try_into` and returns `OperandCountMismatch` unless there are exactly `N` (`X64sfe:16086-16088`). The (N, read_count) pairs are compare (8, 4), copy (5, 3), scan (5, 2) and zero (3, 2). `read_count <= N` in each, so the slice cannot panic.
- Outcome: **intended**. No fix.

### C7. `canonical_effects_encoding` hard-codes `MayArchitecturalFaultV1`
- Source: record §"21719-21749", "Stated limits".
- Contract: the encoded trap must match the target's machine-effect catalog. `targets/target-x86_64/src/machine_effects.rs:96-155` declares `MayArchitecturalFaultV1` for `CopyBytes`, `ZeroBytes`, `ScanByteSequence` and `CompareByteSequences`, the four forms that use the helper. These loops dereference caller pointers, so a page fault is the honest trap.
- Outcome: **intended**. No fix.

### C8. `CallScalar` is refused with `LayoutDependentForm`
- Source: record "Stated limits" item 1; test `scalar_call_is_explicitly_refused_before_encoding` (`X64sfe:19127`).
- Code path: `CallScalar` is listed in `selected_instructions::dedicated_selected_forms!` (`representations/selected-instructions/src/instructions.rs:454-488`). `family_and_operand_count` (`X64sfe:16244-16247`) and `encode_unchecked` (`X64sfe:12434-12437`) refuse that whole group. Scalar calls are encoded by `encode_x86_64_selected_scalar_call_template` (re-exported at `X64sfe:21696-21700`).
- Outcome: **intended**. No fix.

### C9. Compare and LEA immediates are limited to 0..=4095
- Source: record "Stated limits" item 2; tests at `X64sfe:19284`, `:19331-19334`, `:19576`.
- Outcome: **duplicate** of the area 07-13 row "11, x86 memory immediate and width diagnostics" (repaired in #5392): unsigned 12-bit immediates keep `ImmediateOutsideU12`, and memory forms now report their signed 32-bit limits. No fix.

### C10. `encode` re-validates its own output, and the generic validator requires canonical bytes
- Source: record "Five things" item 2; `X64sfe:21839`, `:21891-21894`.
- Outcome: **intended**. Confirmed at the baseline. No fix.

### C11. The test byte interpreters panic on an unmodeled instruction or an undefined flag
- Source: record §"21299-21653 the byte interpreter" and "Five things" item 5.
- Trace: `execute_decoded` panics on an unhandled `Decoded` variant and on reading a flag that is `None`. The panic fails the test instead of letting a new instruction or an undefined-flag branch pass silently.
- Outcome: **intended** (strict test oracle). No fix.

### C12. Four byte interpreters and four bit-flip helpers in the tests do the same job
- Source: record "Same job elsewhere" under §"19120-19775 mod scalar" and §"21299-21653 the byte interpreter".
- Trace: interpreters are `execute_decoded` (`X64sfe:21330`), the loops inside `wrapping_remainder_guard_skips_overflowing_quotient...` (`:21034`) and `wrapping_divide_then_sign_extension_wraps_narrow_min_by_minus_one` (`:21145`), and `execute_saturating` (about `:18951`, unit 018 gap 005's range). Bit-flip helpers are `assert_every_byte_change_rejected` (`:20758`), `assert_every_bit_change_rejected` (`:20777`), `assert_every_bit_flip_rejected` (`:21912`) and, for truncation, `assert_byte_fragility` (`:21902`).
- Outcome: **still open** (test-only duplication; no wrong compiler output). Proposed fix: route the two inline loops through `execute_decoded`, and make `assert_every_bit_change_rejected` call `assert_every_bit_flip_rejected` with a validator closure. This is test maintenance, not an ordinary compiler bug, so this packet does not change it.

## Unresolved claims
- C12: test-only duplicate interpreters and bit-flip helpers (proposed fix above).

## Build break found while running the owning tests

### B1. `optimization-unit` does not compile on `main`
- Source: not an atlas claim. The first run of this packet's tests at `a0152d26` (equal to `origin/main` on 2026-11-24) stopped in a dependency: `error[E0004]: non-exhaustive patterns: &semantic_vocabulary::Proposition::DeclarationIdentity(_) not covered` at `optimization-unit/src/identity.rs:267`.
- Cause: #5393 (`4045f5ea`) added `Proposition::DeclarationIdentity(ClauseIdentity)` and taught the Psi declaration encoder to write it (tag 23), but not the optimizer's canonical identity encoder `proposition_encoding::encode_proposition` in `source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs`. Every native crate that depends on `optimization-unit`, including `isa-x86_64`, failed to build. No open PR repaired it.
- Contract: optimizer identity bytes must keep every input that distinguishes two propositions.
- Fix: a `DeclarationIdentity` arm writes tag 23 and every carrier, operand, application declaration (supplier slot and generation, member) and argument, then the two side indexes. Fixture: `identity::proposition_encoding::tests::declaration_identity_bytes_keep_every_clause_input`, which changes one input at a time and requires different bytes.
- Outcome: **fixed** (code and fixture in separate commits).

## Validation
- Before the fix, at `a0152d26`: `CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx test -p isa-x86_64 --lib -- selected_form_encoding::tests::trapping selected_form_encoding::tests::scalar selected_form_encoding::tests::saturating selected_form_encoding::copy_bytes selected_form_encoding::zero_bytes selected_form_encoding::scan_byte_sequence selected_form_encoding::compare_byte_sequences < /dev/null` failed to build with the E0004 error above.
- After the fix, the same command: `test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 136 filtered out`. These tests pin C2, C3, C6, C8, C10 and C11.
- `mbx test -p optimization-unit < /dev/null`: 15 passed, 0 failed (lib), plus the new fixture.
- `mbx clippy -p optimization-unit --all-targets -- -D warnings`: clean.
- `mbx xtask fmt --check` fails on `main` before and after this change. The changed lines pass `rustfmt --check`; the remaining diff in `identity.rs` (line 5171) predates this change.

Packet complete.
