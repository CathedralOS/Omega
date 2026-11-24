# Packet unit_031_gap_016: selected-instruction effect catalogs (area 12)

Packet complete. One claim was reproduced and repaired: the machine-effect
catalog validator accepted a row declared as "may fault" whose alternative was
encoded as "never traps", and stage 10 skips its installed fault check for
never-trapping alternatives. Three claims remain open and are listed below.

Scope: [record](../../records/unit_031_gap_016.md) for
[`units/gap_016.tsv`](../../units/gap_016.tsv), area 12-native-representations.
The ranges are `effects/catalog/identity.rs` 1-2336, `effects/catalog/validation.rs`
1-3601 and `effects/program.rs` 1-63 under
`source-rust/subsystems/compiler/psi-to-native/representations/selected-instructions/src/`.
Baseline `24ad2700685e`; reviewed at `a0152d26`. `identity.rs` is now 2,122
lines, so record line numbers in that file are historical.

## Claims

| # | Record section | Code path | Outcome | Evidence or proposed fix |
| --- | --- | --- | --- | --- |
| 1 | §1 Fail-open risks, encoded-memory tag 19 (`identity.rs:278-285`, `:440`); §5 item 4 | `effects/memory.rs`, `catalog/identity.rs` `encode_machine_encoded_effects_identity` | Fixed in [#5185](https://github.com/CathedralOS/Omega/pull/5185); duplicate of the area-12 row "effect tag 19" in [07-13](../07-13-psi-and-native.md) | The encoder now calls `effects.memory.encode(bytes)`. `memory.rs` gives `ReadTableFieldV1` tag 19 and `CatalogFenceV1` tag 28 in one wire table shared by identity and plan decode. `encoded_memory_effect_identity_bytes_are_pinned` pins `[28]` and `[19, ...]`. |
| 2 | §1 Fail-open risks, `encode_len` panics (`identity.rs:744`) | `catalog/identity.rs` `encode_len` | Intended | `u64::try_from(usize)` cannot fail on the 64-bit hosts Omega targets. The `expect` message states the invariant. |
| 3 | §2 Fail-open risks, encoded trap not tied to declared trap on the generic arm (`validation.rs:3473-3478`); §5 item 4 | `catalog/validation.rs` `validate_encoded_effects`, last tuple arm; consumer `pipeline/10_framed-machine-to-bound-framed-machine/src/installed_handlers.rs` | Reproduced and fixed in this PR | Stage 10 accepts `Trap::NeverV1` with no further check and requires a discharged fault origin only for `MayArchitecturalFaultV1`. Before: `may_fault_declarations_cannot_encode_a_never_trapping_alternative` failed with `left: Ok(())`, `right: Err(InvalidEncodedEffects(PortWrite))`. After: the arm requires an encoded may-fault trap when the row declares one; 131 selected-instructions tests pass. The x86-64 and AArch64 catalogs already encode these rows as may-fault (`form_effects/port_write.rs`, `register_access.rs`, AArch64 `machine_effects.rs`), and their 41 and 50 tests still pass. A wider encoded trap stays valid, as `control_flow_rows_pin_their_encoded_control_shape` requires for branches. |
| 4 | §2 Fail-open risks, `AtLeastOneOperandDoesNotAliasView` ignores `excluded_view` (`validation.rs:3582-3594`) | `catalog/validation.rs` `validate_applicability`; consumer `pipeline/08_register-homes-to-post-allocation-machine/src/machine/plan.rs` `is_applicable` | Still open | The catalog validator has no physical register model, so it cannot check the view. Stage 08 evaluates `!aliases(left, excluded) \|\| !aliases(right, excluded)`, and `PhysicalRegisterModel::aliases` (`targets/register-model/src/lib.rs:131`) returns `false` for an unknown view, so an unknown `excluded_view` makes the alternative always applicable. No shipped catalog uses this applicability (only a `program.rs` test does). Proposed fix: stage 08 refuses an `excluded_view` that `physical.model().view()` does not find. |
| 5 | §2 Fail-open risks, `OptimizationSelected` checks only the phase (`validation.rs:3595-3599`) | `catalog/validation.rs`; `pipeline/08_.../machine/optimization.rs` `declared_form`, `extra_units` | Intended | The validator still runs every encoded-effect check on the alternative (implicit clobbers may only be a superset, `validation.rs:2714`). Stage 08 picks the form by the exact `Optimization` value and rewrites onto it only when the extra defined or clobbered units are proved dead (`optimization.rs` header, `:DeclaredFormLiveness`). Stage 08's `plan.rs` never chooses it as an ordinary alternative. |
| 6 | §2 Fail-open risks, `callback_writes` and `table_call` derived from the alternatives (`validation.rs:2260-2274`) | `catalog/validation.rs` | Intended (traced by the record) | The record notes that it is not an accept hole: the expected memory and trap follow the alternatives, and the foreign-call tuple arms then bind each alternative's memory, stack and trap shape. |
| 7 | §2 Fail-open risks, `Store64` frame-store arm leaves `stack_pointer` unconstrained (`validation.rs:3379-3383`) | `catalog/validation.rs` arm for `WriteFrameStorageV1 { byte_count: 8, .. }` | Still open | The validator has only the constraint row and architecture, which name no stack-pointer view. Consumers of `WriteFrameStorageV1.stack_pointer` in stages 08-13 were not traced. Next check: find the reader of that field and confirm that it compares the view with the target's stack pointer. |
| 8 | §2 Same job elsewhere, and §5 item 5: `ResultAliasesOperands` and `ResultDistinctFromOperands` validate identically (`validation.rs:3540-3581`) | `catalog/validation.rs` `validate_applicability`; `pipeline/08_.../machine/plan.rs` `is_applicable` | Intended | The catalog check is structural (operands exist, are distinct, have the right access and share a class). Stage 08 evaluates the aliasing against allocated views: `aliases(result, left) && aliases(result, right)` against `!aliases(..) && !aliases(..)`, and refuses an ambiguous or empty choice. |
| 9 | §2 Unused or one-caller code: the header names `validate_isa_machine_effect_catalog` as living in validation.rs | `catalog/validation.rs:1-5` | Fixed in this PR (comment) | The header now says that function is in `catalog.rs` and reaches this file through `validate_machine_effect_catalog`. |
| 10 | §2 Same job elsewhere, call rosters checked three ways (`validation.rs:2154`, `:2640-2680`, per-arm `positional_roster`) | `catalog/validation.rs` | Intended | The checks overlap but agree; the per-arm checks add fixed positions the generic list checks do not. No wrong result. |
| 11 | §3 Same job elsewhere: plan fields `memory`/`trap`/`barrier`/`call`/`cleanup` must equal the catalog's | `pipeline/06_.../analyses/src/machine_effects.rs:319-339` | Traced (producer) | Stage 06 looks up the exact declaration by key and copies all five fields and the alternatives into `InstructionMachineEffects`. The plan identity binds the catalog identity. A consumer-side re-check was not traced and is not claimed. |
| 12 | §1 Same job elsewhere; "What I am unsure about" item 4: do `machine-code` and `post-allocation-machine` share the encoder | `machine-code/src/layout.rs`, `layout/text_section.rs`, `encoding_identity.rs`; `post-allocation-machine/src/identity.rs:101` | Intended | Each calls `selected_instructions::encode_machine_*_identity`; none copies it. |
| 13 | "What I am unsure about" item 3: pinned family tag coverage beyond `MachineSemanticKind::ALL` | `catalog/identity.rs` tests `alternative_family_tag_is_pinned_for_every_family` | Still open | The test covers families reachable from semantics. Whether `MachineAlternativeFamily` has other variants was not checked. Next check: compare the enum's variants with the families derived from `MachineSemanticKind::ALL`. |
| 14 | "What I am unsure about" item 1: do ISA builders produce never-trapping rows for may-fault semantics | `targets/target-x86_64/src/form_effects/`, `targets/target-aarch64/src/machine_effects.rs` | Resolved with claim 3 | The shipped rows encode may-fault. The new validator rule passes both target suites. |

## Unresolved claims

- Claim 4: an unknown `excluded_view` makes an `AtLeastOneOperandDoesNotAliasView` alternative always applicable in stage 08.
- Claim 7: the `Store64` frame-store stack-pointer view is not checked in the catalog validator; its consumer is untraced.
- Claim 13: pinned tag coverage for `MachineAlternativeFamily` variants not reachable from semantics is unverified.

## Validation

- Before fix (fixture commit only): `mbx test -p selected-instructions --lib may_fault_declarations < /dev/null` failed: `left: Ok(())`, `right: Err(InvalidEncodedEffects(PortWrite))`.
- After fix: `mbx test -p selected-instructions < /dev/null`: 131 passed, plus 1 doc test.
- `mbx test -p target-x86_64 -p target-aarch64 < /dev/null`: 41 and 50 passed.
- `mbx clippy -p selected-instructions --all-targets -- -D warnings`: clean.
- `rustfmt --edition 2024 --check` on `validation.rs`: clean. `mbx xtask fmt --check` reports diffs in unrelated `source-rust/application/tests/` files that also fail on the parent.
