# Area 02 — accepts when it does not know

Read-only trace of every entry classed `open` in `.agents/atlas/compiler-2026-11-10/digests/area_02.md`. Citations are `file:line` at `457adf4a11f`. `git diff --stat 457adf4a11f HEAD` on the cited files was empty, so the line numbers still match. Each claim below is from lines I read unless it says inferred.

Paths are under `omega-rust/omega/compilation/backend/`. `NA/` is `artifacts/native-artifact/src/`, `CC/` is `calling-conventions/src/`, `ELF/` is `images/image-elf/src/`, `EM/` is `images/image-emission/src/`, `ff` is `EM/function_fragments.rs`, `MO/` is `images/image-macho/src/`, `PE/` is `images/image-pe/src/`, `IM/` is `images/image/src/`, `A64/` is `instruction-set-architectures/isa-aarch64/src/`.

| # | file:line | verdict | reason |
| --- | --- | --- | --- |
| 1 | NA/native_artifact/provider_execution.rs:140-169 | CLOSED | Import, Component, and Table always carry a same-stack contribution; only SystemCall takes the kernel arm. The final `Ok` is not reached. |
| 2 | provider_execution.rs:37-40, :99-102 | REACHABLE | A boundary whose identity text is missing is replayed as the empty requirement and accepted when the plan and the execution roster agree on that empty name. |
| 3 | native_artifact.rs:992-1023 | CLOSED | An unmapped name is returned unchanged, then the roster equality rejects it unless a plan identity selects it. |
| 4 | NA/native_artifact/identity.rs:322-325 | REACHABLE | An identity index outside the table is hashed as the empty string, and emission seals that identity. |
| 5 | NA/physical/derivation/evidence.rs:154 | CLOSED | Retained foreign-call custody comes from the same publication option that gates the compare, so a missing publication has an empty list. |
| 6 | evidence.rs:201-204, :264-266 | REACHABLE | Missing boundary text is looked up as `""`, and exactly one plan that lists `""` is accepted for that boundary. |
| 7 | evidence.rs:60-65; dynamic_elf.rs:63-65 | REACHABLE | An unsupported shape becomes a recorded gap, and emission returns success with that gap in the identity. |
| 8 | provider_custody.rs:29-37 | REACHABLE | A requirement with no family-realization row skips the family checks and derivation continues. |
| 9 | internal_calls.rs:474-481, :488-497 | REACHABLE | A provider or candidate identity index with no text is stored as the empty name and the child is still produced. |
| 10 | fragment_custody.rs:401-404 | CLOSED | A register argument one past the operand list fails the rejoin; a stack argument does not use that index. |
| 11 | runtime_provisions.rs:172-179 | REACHABLE | Two empty strings match as a selected row for any provider-plan identity when no row is keyed by the empty name. |
| 12 | runtime_provisions.rs:215 | REACHABLE | A system call, table, or component host provision with no admitted calling-application row is kept. |
| 13 | CC/terminal_layout.rs:191-203 | UNDECIDED | The declared size is returned without reading fields. The four layout callers were not read. |
| 14 | CC/plans/call_plan_evaluation.rs:167-310 | REACHABLE | A call plan is accepted from coverage and architecture alone, including an import whose registers are not the ones the platform convention would assign. |
| 15 | call_plan_evaluation.rs:290-292 | REACHABLE | A result register equal to a parameter register is not recorded in the occupied set. |
| 16 | CC/host_operations.rs:380-394 | REACHABLE | A Darwin symbol that matches no prefix is bound to libSystem and the image is still emitted. |
| 17 | ELF/imports.rs:178-185 | CLOSED | The three emitters later reject an invalid symbol handle in `apply_relocations`. |
| 18 | ELF/file_assembly.rs:2217-2337 | UNDECIDED | `.rela.dyn` bytes outside fixup fields are not compared. The producer of those bytes was not read. |
| 19 | ff:3988-4112 | REACHABLE | An object with no fragment replay passes this validator when it avoids six listed shapes, and image validation continues. |
| 20 | ff:18735-18746, :18864-18865, :18995-19007 | UNDECIDED | Those operation arms admit with no instruction compare. The rest of `source::admit` was not read. |
| 21 | ff:18986-18994 | UNDECIDED | An empty cleanup list admits a return without looking at a selected return. Later passes in the same function were not read. |
| 22 | ff:18728-18733 | REACHABLE | A port write is admitted when any one selected instruction matches, including a duplicated write. |
| 23 | ff:16982-16984 | UNDECIDED | Empty runtime indices return true. The other conjunct at the only caller was not read. |
| 24 | ff:4717-4729 | UNDECIDED | A descriptor instruction owned by an installation is skipped. `installed_interrupts::validate` was not read. |
| 25 | ff:20219 | REACHABLE | A unit return that discards only one of two hosted read-byte results is admitted. |
| 26 | EM/object_artifact.rs:3628 | REACHABLE | An admitted-provider execution paired with the hosted write-byte realization passes; the following custody compare does not look at the realization. |
| 27 | object_artifact.rs:1170 | UNDECIDED | Non-call operations in the installed RAM closure are skipped. No other checker of those operations was read. |
| 28 | object_artifact.rs:1673-1675 | REACHABLE | A normalized foreign call with no descriptor-input record passes this validator. |
| 29 | object_artifact.rs:2594-2609 | REACHABLE | A parameter, block parameter, boolean immediate, or float immediate is recorded as the whole call instruction, with no placement transport check. |
| 30 | EM/entries.rs:328 | UNDECIDED | Instruction families outside the privileged match add nothing. The full family list was not read. |
| 31 | entries.rs:3891-3896 | REACHABLE | A free-unit entry on a target with no unit-entry form is emitted with no shim. |
| 32 | entries.rs:5575-5594 | REACHABLE | Two origin rows with one identity are merged when a field is empty on one row and filled on the other. |
| 33 | MO/dyld_linking.rs:193-210 | CLOSED | `emit_macho_container_contents` runs `apply_relocations` after thunk installation, and that rejects an invalid symbol handle. |
| 34 | PE/imports.rs:95 | REACHABLE | A PE image with subsystem 10, 11, or 12 and a string-backed or component import is emitted. |
| 35 | PE/headers.rs:42, :52; PE/sections.rs:51, :57, :60, :61, :86, :183 | REACHABLE | Section lengths are cast to `u32` with no range check, and the PE file is still returned. |
| 36 | PE/lib.rs:343 | UNDECIDED | The written file is returned without being parsed back. No concrete mismatched field was shown from the planner. |
| 37 | IM/lib.rs:221 | REACHABLE | A symbol whose offset is past the end of its section still supplies a relocation address. |
| 38 | A64/machine_effects.rs:993-998, :1187 | UNDECIDED | Unknown semantics get no implicit effects, never-traps, and 4 bytes. The reads/writes table was not compared arm by arm. |
| 39 | A64/post_handoff_writer.rs:104-126 | UNDECIDED | The byte loop is used for widths other than 1, 2, 4, and 8. Whether a width above 8 can arrive was not read. |

## 1. CLOSED

`validate_foreign_provider_binding` is the function at `provider_execution.rs:121-171` (read in full). Its only non-test caller is `replay_provider_executions` at `:103` (grep `validate_foreign_provider_binding(`: 3 matches, the definition, `:103`, and `tests.rs:80`). `replay_provider_executions(` has 3 matches: the definition, `native_artifact.rs:556`, and `dynamic_elf.rs:147`, both inside `validate`.

`NormalizedForeignCallMechanism` (`target-operations/.../boundary/realizations.rs:92-111`, read) puts `same_stack_contribution` on Import, Component, and Table, and not on SystemCall. `NormalizedForeignCallBinding::same_stack_contribution` (`:317-318`, read) therefore returns `Some` for those three. `system_call` (`:264-270`, read) returns `Some` only for SystemCall. The `Ok(())` at `provider_execution.rs:169` has no value that reaches it.

The digest is wrong that Import, Table, and Component reach the unchecked return. They all enter `validate_foreign_stack_contribution` at `:140-150`.

## 2. REACHABLE

Read `replay_provider_executions` (`provider_execution.rs:20-119`) and both `validate` callers (`native_artifact.rs:521-563`, `dynamic_elf.rs:109-154`). `TerminalIdentityTable::text` (`psi/.../terminal-psi/src/identity.rs:354-356`, read) returns `None` only when the index is outside `entries`. `unwrap_or_default` turns that into `""`. Index 0 is the empty string (`identity.rs:302`, `:249-252`, read), so an absent index and the empty entry both become `""`.

`validate_boundary_machine` (`foundation_validation.rs:1769-1866`, read) checks the owner identity is nonempty (`:1777-1782`) and does not check `boundary.identity`. Grep of `text(boundary.identity)` across `omega-rust` found one rejection of an empty boundary identity: `validate_retained_external_provider_owners` (`compiler_builtins/retained.rs:301-315`, read), called only from `admit_terminal_intrinsics` (`:434`, grep of that name: 2 matches, the definition and `build-settings/.../provider_execution.rs:115`). `NativeArtifact::validate` does not call it.

`selected_requirement_identity("")` (`native_artifact.rs:992-1023`, read) returns `""` when no row matches. `validate_provider_execution_reports` (`:264-314`, read) allows `""` in `requirement_identities` and accepts when the reported set equals the required set. A boundary whose identity index is outside the table, admitted as a provider execution, is therefore replayed as the empty requirement and accepted when the selected plan lists that empty name and the execution roster records it.

`realize_native_artifact` (`native-compiler/src/realization.rs:128-141`, read) calls `emit`, and `from_emitted_parts` (`native_artifact.rs:471-477`, read) does not call `validate`. This hole is on the replay gate (`from_replayed_parts` at `:504`, and `saved_project.rs:278`).

## 3. CLOSED

Read `NativeSelectedProviderPlan::selected_requirement_identity` (`native_artifact.rs:992-1023`) and the test at `tests.rs:253-318`. Grep `fn selected_requirement_identity`: 2 definitions (this one and `provider_evidence.rs:725`). Callers of this method inside the replay are `required_report_key` (`provider_execution.rs:238-241`), `validate_provider_execution_reports` (`:298-301`), and `validate_foreign_stack_contribution` (`:337-338`).

An unmapped name is returned as itself (`:1020-1023`). `validate_provider_execution_reports` then requires some plan requirement identity to select to the execution's name (`:298-301`, read) and the reported set to equal the required set (`:312-314`, read). The test's unknown name `other` is returned unchanged (`tests.rs:287`) and a roster that uses it is rejected (`:316-318`). Every path I read goes through that equality.

## 4. REACHABLE

Read `hash_identity_text` (`identity.rs:317-326`) and its uses in `derive_native_artifact_identity` (`:182-194`, read) and `hash_physical_entry_requirements` (`:399-411`, read). Grep `hash_identity_text(`: 8 matches, all in this file.

`text` returns `None` out of range, and `unwrap_or_default` hashes no bytes. Two different out-of-range indexes therefore contribute the same digest bytes. `validate_family_realization_rows` (`provider_evidence.rs:921-939`, read) rejects index 0 (`is_absent`) but not an out-of-range index: `identity_text` (`:1199-1204`, read) turns that into `""`, and one such insert succeeds. `from_emitted_parts` seals the recomputed identity (`native_artifact.rs:471-477`, `:508-518`, read) without rejecting the index.

A family-realization row whose head index is outside the identity table is sealed with the same identity contribution as a different missing index.

## 5. CLOSED

Read `derive_physical_evidence` (`evidence.rs:67-156`) and `ValidatedOptimizedNativePhysicalEvidenceScope` (`physical/mod.rs:248-258`). `fragment_publication` and `foreign_call_custody` both read `self.publication`. When the publication is `None`, custody is `&[]` (`:254-258`). The compare at `evidence.rs:154-156` runs only when `fragment_publication.is_some()`. A nonempty custody list cannot exist on the path that skips the compare.

Grep `derive_physical_evidence(`: 4 matches (the definition, `dynamic_elf.rs:73`, `native_artifact.rs:456` and `:483`).

## 6. REACHABLE

Read the map at `evidence.rs:195-207` and the lookup at `:261-271`, in `derive_physical_evidence`. Missing text becomes `""` the same way as entry 2. `binary_search` then requires exactly one selected plan whose `requirement_identities` contain `""`; zero or several return the error at `:269-270`. One such plan is accepted and `validate_family_provider_custody` is called with `""`.

`project_selection` (`native_artifact.rs:771-793`, read) fills `requirement_identities` from `text(...).unwrap_or` inside `selected_row_requirement_identities` (that helper was not read) and `NativeSelectedProviderPlan::new` (`:852-870`, read) does not drop `""`. A boundary with no identity text is therefore joined to the one plan that lists the empty requirement, and physical derivation continues.

## 7. REACHABLE

Read `blocked` (`evidence.rs:60-65`), `from_emitted_parts` for both artifact types (`native_artifact.rs:471-477`, `dynamic_elf.rs:60-66`), and `into_parts` (`native_artifact.rs:290-301`). `Ok(Blocked(gap))` becomes `physical_evidence: None` and `physical_evidence_gap: Some(gap)`, then `seal` returns `Ok`. `RequestedNativeArtifact::emit` (`:194-218`, read) maps that `Ok` to a successful artifact.

Grep `.physical_evidence_gap`: only the field compares inside `validate` and the identity hash. Grep `physical_evidence(` as a call found only the two accessors. No caller refuses a present gap. An occurrence whose settlement is neither a hosted builtin nor an admitted-provider child (`evidence.rs:308-313`, read), or whose foreign call or operator span the deriver cannot place (`:332-335`, `:419-423`, read), is emitted as a successful artifact whose physical evidence is a named gap.

## 8. REACHABLE

Read `validate_family_provider_custody` (`provider_custody.rs:20-80`). Grep of that name: 2 matches, the definition and `evidence.rs:272`. After `Ok(())` at `:36`, `derive_physical_evidence` continues into the settlement, foreign, or internal-call arm (`:280-360`, read). Grep of `family_realization` and `target_machine_families` under `physical/` found no later family-row requirement on that path (`internal_calls.rs:122` is the separate family-origin arm, read at `:109-150`).

A boundary that belongs to a target-machine family, whose selected plan lists the requirement name and has no family-realization row for it, skips the terminal, target, contract, and member checks in this function and is derived as an ordinary settlement or call.

## 9. REACHABLE

Read `derive_boundary_internal_call_child` from `:194` through `:509`. The provider declaration text is rejected when missing (`:463-466`). The provider identity and the candidate identity use `unwrap_or_default` (`:474-481` and again `:488-497`) and are copied into `BoundaryTraitSettlementRole::InstalledProviderInternalCall`. `validate_provider_candidate` (`foundation_validation.rs:1893-1909`, read) rejects `is_absent` (index 0) for those two indexes and does not reject an out-of-range index.

The callers are the two `derive_boundary_internal_call_child` sites in `evidence.rs` (`:240` and `:341`, read). A `Some(child)` is pushed and derivation can complete. An installed-provider internal call whose provider or candidate index is outside the table is recorded under the empty name.

## 10. CLOSED

Read `fragment_scalar_argument_custody_rejoin` (`fragment_custody.rs:317-493`), `fragment_immediate_argument_span_rejoin` (`:496-520`), and `scalar_immediate_span` (`object_artifact.rs:3221-3364`). The past-the-end index is `register_arguments` when `register_positions` has no entry (`fragment_custody.rs:401-404`).

For a register placement, `instruction.operands.get(register_operand)` (`object_artifact.rs:3242`) is `None` one past the end, so `scalar_immediate_span` returns `None`, the rejoin is false, and `fragment_normalized_foreign_rejoin` returns the error at `fragment_custody.rs:250` (read `:237-251`). Its caller is `foreign_calls.rs:223` (grep: 2 matches). For a stack placement the index is not read (`object_artifact.rs:3263-3360`); the outgoing slot and the following store decide. Any other placement is `_ => None` (`:3362`).

The digest's description of the index is right. It does not accept a register argument that is not in the register list.

## 11. REACHABLE

Read `capture_native_admission` (`runtime_provisions.rs:148-249`) and `selected_row_matches_requirement` (`provider_evidence.rs:738-760`). Callers of `capture_native_admission` on this type: `native-compiler/src/realization.rs:63` and `runtime_provisions.rs:606` (grep of the name also hits two other types' methods). `realize_native_artifact` builds provisions through `admit_realization` (`realization.rs:44-70`, read) before emission.

When no family, specialization, or external row is keyed by the requirement, the function returns `requirement == selected_requirement` and does not look at the plan identity (`provider_evidence.rs:759`). `("", "", id)` is therefore true for every `id`. `text(...).unwrap_or_default()` at `runtime_provisions.rs:173-176` feeds that pair when both the boundary identity and the binding requirement text are missing or empty. The check at `:172-180` then passes. A host provision whose boundary and binding are both unnamed is admitted for whatever provider-plan report identity the binding carries.

## 12. REACHABLE

Same function, read through `:249`. `binding.import()` (`realizations.rs:273-287`, read) is `Some` only for `Import`. The error at `runtime_provisions.rs:215-218` fires only in that case. SystemCall, Table, and Component with no external-binding row leave `calling_application` as `None` and are pushed (`:227-240`). `admit_realization` (`realization.rs:60-70`, read) passes each settlement realization into this capture on the compile path.

A system call, dispatch table, or component host provision that has no admitted calling-application row is kept.

## 13. UNDECIDED

Read `resolve_structural_shape` (`terminal_layout.rs:146-203`). Authored, AccessView, and Accessor storage return `ValueShape::integer` of the declared size and do not walk fields. Grep `resolve_structural_shape(`: 4 matches, all in this file. Grep `StructuralLayouts::new(`: 4 callers (`function_signature.rs:785`, `structural_layout.rs:1220` and `:1294`, `native_execution_scope.rs:151`, `task-plans/.../activation.rs:278`). Those caller bodies were not read, so I cannot say the declared size is what a later pass accepts.

`foundation_validation.rs:8116-8201` (read) checks authored field offsets against the declared size and uses `_ => {}` for AccessView and Accessor, so that validator does not close the AccessView or Accessor case either.

## 14. REACHABLE

Read `validate_call_plan_structure` (`call_plan_evaluation.rs:167-309`) and `validate_value_placement` (`:312-390`). The structure check covers count, shape, alignment, disjoint parameter registers and stack, and register architecture. It never calls `evaluate_call_plan`. Grep `validate_call_plan_structure(`: 3 matches (the definition, `validate_call_plan` at `:157`, and `plans.rs:420`). `validate_boundary_entry_plan` (`plans.rs:405-411`, read) is what the foreign-call deriver uses (`foreign_calls.rs:472-483`, read).

`has_valid_entry_mechanism` (`realizations.rs:238-242`, read) checks policy and `CallReturn` for Import and Table, and compares `evaluate_call_plan` only for SystemCall (`:243-260`). An imported foreign call whose entry plan uses a non-overlapping register of the right architecture, with shapes that match the signature, but not the register the platform convention would assign, passes both checks.

Grep of `validate_boundary_entry_plan(` shows many production callers. I read the foreign-call one and the wrapper. I did not read each caller.

## 15. REACHABLE

In the same function, parameter registers are inserted into `occupied_registers` (`call_plan_evaluation.rs:205-217`, read). The result is checked only by `validate_value_placement(result, architecture, ...)` at `:290-291`. That helper's signature is `(placement, architecture, value_index)` and does not receive the occupied set (read `:312-316`). An indirect result pointer uses the same helper (`:377-382`) and is not compared with parameter registers.

A call plan whose result register is the same register as one of its parameters passes `validate_call_plan_structure` and therefore `validate_boundary_entry_plan`.

## 16. REACHABLE

Read `darwin_import_library` (`host_operations.rs:380-395`). Grep of that name: 3 matches, the definition plus `dyld_linking.rs:117` and `:1608`. The digest's "three call sites" is wrong; there are two.

At `:117` the symbol is a string-backed bootstrap import inside `install_import_thunks` (`:75`, read `:90-126`). An empty name is rejected at `:118`, after the library is chosen. A nonempty name that does not start with `_objc_`, `_sel_`, `_class_`, `_object_`, `_method_`, `_ivar_`, `_protocol_`, or `_CG` is bound to `/usr/lib/libSystem.B.dylib`. The second site (`:1605-1616`, read) is the fallback when no normalized import locator matches, and the fixup validator then checks pointer geometry (`:1628-1635`), not the symbol spelling.

A misspelled Darwin bootstrap symbol that loses those prefixes is bound to libSystem and Mach-O emission continues.

## 17. CLOSED

Read `canonical_referenced_imports` through the skip (`imports.rs:178-199`) and its three callers (grep: `shared_component.rs:95`, `static_executable.rs:184`, `import_sections.rs:88`).

`apply_relocations` (`IM/relocations.rs:17-37`, read) returns an error when `final_image_symbol_address` is `None`. That function (`IM/lib.rs:208-222`, read) returns `None` when `symbol()` fails, which an invalid handle does. `final_image_imports_symbol` (`:224-230`) is also false for a handle that is not an import row, so the error is "unknown symbol" rather than the import message. Either result is `Err`.

Order I read: `emit_elf_executable` calls the import scan at `static_executable.rs:184` and `apply_relocations` at `:216` before writing bytes. `shared_component` `emit` calls the scan at `:95` and `apply_relocations` at `:307`; the only `Ok((output, ...))` in that function is at `:449`, after the apply. `emit_elf_dynamic_executable` (`dynamic_executable.rs:614-662`, read) always reaches procedure-linkage validation, whose `relocate_image_local_memory` (`file_assembly.rs:2476-2504`, read) calls `apply_relocations`. An invalid handle is not an import symbol (`final_image_imports_symbol` is false), so it is kept in the local relocation table and then rejected.

## 18. UNDECIDED

Read `validate_contents` (`file_assembly.rs:2217-2336`) and the fingerprint (`:2401-2414`). The unchanged-byte loop lists SourceText, ProcedureLinkage, ProcedureGot, and ProcedureRelocation. `GeneralRelocation` is a storage arm (`:3345-3346`, `:3377`, read) and is not in that loop or in the fingerprint hash. Fixup applications that do name GeneralRelocation are compared field by field (`:2300-2320`). Bytes of `.rela.dyn` outside those fields are not compared with the upstream section, and the length of that section is not checked in this function.

The producer that fills `contents` for GeneralRelocation was not read, so I cannot say whether a program input can change one of those unchecked bytes. The seal-then-validate macro was not opened.

## 19. REACHABLE

Read `replay::validate` (`ff:3988-4112`). When `fragment_replay` is `None` it errors only for graph storage, mismatched parameter homes, borrowed structural pointers, a selected Unit-call argument, a selected boundary output, or a hosted read-byte settlement. Otherwise it returns `Ok(())` at `:4111` and does not call `validate_function_fragment_object_artifact`.

Grep `function_fragments::replay::validate(`: `image.rs:224` and `:1894`, both with `?`. An object that has functions and none of those six shapes, and no fragment replay, passes image validation without a replay of the abstract, target, or selected evidence.

What constructs an object with `fragment_replay: None` was not read. The validator itself accepts that object.

## 20. UNDECIDED

Read the match arms at `ff:18735-18746` (atomic load, store, read-modify-write, swap, compare-exchange, fence), `:18864-18865` (integer and boolean constants), and `:18995-19007` (comparisons, boolean and bitwise operations, integer arithmetic, shifts, casts, trapping integer, float-result policy). Each arm is `true`. The loop rejects only when `admitted` is false (`:19026-19029`, read). `source::admit` starts at `:18534` (read the opening through `:18548` and the per-fragment setup `:18587-18609`). I did not read the rest of `admit` after this loop, so I cannot say a later pass rejects a constant or an atomic that has no matching instruction.

## 21. UNDECIDED

Read the `Return` and `ReturnUnit` arms (`ff:18986-18994`). Empty `cleanup_actions` is `true` without calling `control_flow::retained` or `cleanup_actions_match`. The same limit as entry 20: this loop admits that return, and I did not read the rest of `source::admit`.

## 22. REACHABLE

Read the `PortWrite` arm (`ff:18728-18733`). It is `selected.instructions().any(...)` with the port, the value, and the operation provenance. A second selected port-write with the same port, value, and provenance still yields true. The failure path is the same `if !admitted` at `:19026`. A port write that was duplicated in the selected function is admitted.

## 23. UNDECIDED

Read `runtime_footprint_retained` (`ff:16937-16984`). When `operation.runtime_indices()` is empty it returns `true` at `:16982-16983` and does not look at memory-access rows. Grep of the name: 2 matches, the definition and the `StructuralByteSequenceFieldLength` arm at `:18787-18789`, which also requires `structural_fields::retained`. That `retained` function was not read, so I cannot say a length operation with no runtime indices and no memory-access rows is accepted.

## 24. UNDECIDED

Read `dedicated_descriptor_owner` (`ff:4717-4729`) and its only caller (`:4456`, grep: 2 matches). A hardware descriptor instruction whose provenance operation is in the installation list and not in the descriptor-operation list returns `Ok(false)`, and the caller `continue`s (`:4456-4458`), leaving it out of `hardware`. `source::admit` calls `installed_interrupts::validate` at `:18597` before `DescriptorReplay::new` at `:18606` (read). I did not read `installed_interrupts::validate`, so I cannot say the skipped instruction is checked there.

## 25. REACHABLE

Read `cleanup_actions_match` (`ff:20219-20277`). The digest is wrong that the body was unread and that an empty list is unchecked: `:20247` requires `!actions.is_empty()`. It is also not an arbitrary subset of discards. Every action must be a `DiscardRoot` of a distinct affine hosted read-byte result with one matching settlement (`:20248-20276`). It does not require every such result to appear.

Grep `cleanup_actions_match(`: the definition, the `ReturnUnit` arm at `:18994`, and tests. The tests assert that `actions[..1]` and `actions[1..]` both match (`:20644-20646`, read those lines). `ReturnUnit` admits when this function returns true (`:18990-18994`). A unit-returning function that discards only one of two hosted read-byte results is admitted.

## 26. REACHABLE

Read `validate_completion_custody` (`object_artifact.rs:3573-3642`) and `derive_completion_provider_custody` (`machine-code/src/boundary.rs:51-71`). The match returns `true` for the three hosted builtin pairs, `false` for any other `CompilerBuiltin` execution and for a `HostedExitProcessI32` or `HostedReadByte` realization, and `true` on `_` (`object_artifact.rs:3628`). `HostedWriteByteI32` is not in the `false` arm. The custody compare (`:3633-3641`) uses `derive_completion_provider_custody`, which looks only at whether the execution is `AdmittedProvider` and whether each receipt has a source. It does not look at the realization.

An admitted-provider execution whose realization is the hosted write-byte builtin passes this function when the claim sources and receipts agree.

## 27. UNDECIDED

Read `ram::validate` (`object_artifact.rs:1089-1189`). The operation match handles `EstablishAccessReference`, three static-interrupt boundary calls, other boundary calls, and external completions. `_ => continue` at `:1170` skips every other kind, and the function then returns `Ok(())`. I did not search for another checker of those skipped operations on this closure, so I cannot say a skipped operation is a wrong program that would be accepted.

## 28. REACHABLE

Read `descriptor_input::validate` (`object_artifact.rs:1668-1675` and the guard walk `:1676-1705`). `record.call.descriptor_input` of `None` returns `Some(())` immediately. The only caller (grep `descriptor_input::validate(`: `object_artifact.rs:1969`) uses `.ok_or_else`, so `Some(())` is success. A normalized foreign call whose selected record has no descriptor input passes this check. I did not read a separate pass that requires the guard from the instruction bytes.

## 29. REACHABLE

Read `foreign_scalar_argument_custody` (`object_artifact.rs:2486-2609`). Parameter, block-parameter, boolean-immediate, and IEEE-float-immediate sources set the recorded span to `fragment_instruction_span` of the call instruction (`:2607-2608`). The checks above that (`:2580-2587`) compare parameter index, plan placement, and scalar shape. They do not check that the value was moved into the argument register or stack slot. Integer-immediate and home sources are separate arms and were not the cited ones.

A scalar argument of one of those four sources is recorded as covering the whole call instruction.

## 30. UNDECIDED

Read the `_ => {}` at `entries.rs:328` and the surrounding match from `:250`. Families in the match update a footprint; the default adds nothing. I did not read the function from its start, and I did not enumerate `MachineAlternativeFamily`, so I cannot say a privileged family is missing from the match.

## 31. REACHABLE

Read `hosted_unit_entry::prepare` (`entries.rs:3880-3897`). A physical entry or a hosted receiver returns `Some`. Otherwise a free-unit entry with no `unit_entry_form` returns `Ok(None)` at `:3894-3895`. A non-free-unit artifact also returns `Ok(None)` at `:3891-3892`.

Grep `hosted_unit_entry::prepare(`: `image.rs:157`, `:238`, `:1830`, `:1911`. `emit_direct_executable_image_signed` (`image.rs:1819-1832`, read) and `derive_output` (`:234-240`, read) pass that `Option` through `prepared_parts` and keep emitting. `unit_entry_form` was not read. A free-unit entry whose target has no form is emitted with no entry shim.

## 32. REACHABLE

Read `EntryOrigins::new` (`entries.rs:5556-5595`). Rows are sorted by identity. A later row with the same identity conflicts only when a field is `Some` on both sides and the values differ (`:5573-5578`). A `None` next to a `Some` is not a conflict, and the `Some` is copied onto the prior row (`:5584-5594`).

Two origin rows that share an identity, one of them missing its plan or its assumption or its scope while the other has that field, are merged, and the empty field is filled from the row that has it.

## 33. CLOSED

Read the skip in `install_import_thunks` (`dyld_linking.rs:193-209`) and `emit_macho_container_contents` (`MO/lib.rs:714-783`). The emit function calls `install_import_thunks` at `:738` and `image::apply_relocations` at `:783` before it builds `bytes`. There is no success return between them in the lines I read (`:740-783`). `apply_relocations` rejects an invalid symbol handle, as in entry 17 (`IM/relocations.rs:25-37`).

## 34. REACHABLE

Read the import match in `install_import_thunks` (`PE/imports.rs:78-148`). Grep `subsystem` in that file: 2 matches, the parameter and the `10..=12` test at `:95`, which is only in the normalized-foreign arm. The string-backed arm (`:81-90`) and the component arm (`:123-137`) do not test the subsystem. `emit_pe_x86_64` (`PE/lib.rs:178-183`, read) calls `install_import_thunks` and, on `Ok`, writes the file (`:343-360`, read).

A PE image whose subsystem is 10, 11, or 12 and which has a string-backed or component import is emitted. A normalized foreign import on those subsystems is rejected.

## 35. REACHABLE

Read `write_pe_headers` (`PE/headers.rs:39-69`), `plan_pe_sections` (`PE/sections.rs:50-86`), and `build_base_relocations` (`:170-183`). `write_pe_headers` is called only from `emit_pe_x86_64` (`PE/lib.rs:248`, grep: 2 matches). `section_count as u16` (`headers.rs:42`) cannot truncate for that caller: `section_count` is `1 + usize::from` of four booleans (`sections.rs:65-69`, read), so it is at most 5. `(rva & 0xFFF) as u16` (`sections.rs:202`) is masked to 12 bits.

`image.memory.text.len() as u32`, and the same cast for rdata, data, reloc bytes, and bss (`sections.rs:51`, `:57`, `:60`, `:61`, `:86`), and `relocation.offset as u32` (`:183`), truncate when the `usize` does not fit in `u32`. `plan_pe_sections` does not test that. `emit_pe_x86_64` returns those bytes (`lib.rs:343-360`). A PE image whose text is longer than `u32::MAX` bytes is planned with the truncated length and still returned.

`headers.rs:52` is `size_of_code as u32` on a value `plan_pe_sections` already stored as `u32` (`sections.rs` text raw size; the field type was inferred from the call at `lib.rs:253`). That particular cast does not add a second truncation beyond entry into `plan_pe_sections`.

## 36. UNDECIDED

Read `emit_pe_x86_64` (`PE/lib.rs:178-360`). It writes the headers and sections into `bytes` and returns `Ok(ExecutableImageOutput { bytes, .. })`. Nothing in that function parses `bytes` back. I did not read the planner far enough to show a header field that disagrees with the bytes it just wrote, so I cannot name a wrong program this function accepts beyond the missing re-read itself.

## 37. REACHABLE

Read `final_image_symbol_address` (`IM/lib.rs:208-222`) and `apply_relocations` (`IM/relocations.rs:24-38`). The address is `section_address + symbol.offset as u64` with no comparison to the section length. `apply_relocations` uses that address as the relocation target whenever it is `Some`. A symbol in the text, data, or bss section whose offset is past the end of that section still produces an address, and a relocation that names it is patched with that address when the patch write itself is in range. `patch_bytes::write_u64` was not read.

## 38. UNDECIDED

Read the default arm of `encoded_effects` (`A64/machine_effects.rs:993-998`) and the size default (`:1187`). A semantic that misses the effect match gets no reads, no writes, `NeverV1`, and fall-through, and the size match gives `ExactBytes(4)`. I read the effect match's tail (`:970-998`) and the size function (`:1164-1188`). I did not compare every arm of the reads/writes match that starts at `:729` with these defaults, so I cannot say a semantic is listed in one table and not the other.

## 39. UNDECIDED

Read `load` and `store` (`A64/post_handoff_writer.rs:93-129`). Counts 1, 2, 4, and 8 use a single load or store. Every other count uses a per-byte loop whose shift is `index * 8` (`:111-113`). A count above 8 shifts by 64 or more in that word. The callers that choose `count`, and `layout_plans::validate_fragment`, were not read, so I cannot say a container wider than 8 bytes reaches this loop.

## Counts

- CLOSED: 6 (entries 1, 3, 5, 10, 17, 33)
- REACHABLE: 22 (entries 2, 4, 6, 7, 8, 9, 11, 12, 14, 15, 16, 19, 22, 25, 26, 28, 29, 31, 32, 34, 35, 37)
- UNDECIDED: 11 (entries 13, 18, 20, 21, 23, 24, 27, 30, 36, 38, 39)
- Entries not reached: none

## What I did not read

- The bodies of the four `StructuralLayouts::new` callers (entry 13).
- The GeneralRelocation byte producer and the `seal_stage` macro (entry 18).
- `source::admit` after the per-operation loop, and any earlier pass in that function besides the setup at `ff:18534-18609` (entries 20 and 21).
- `structural_fields::retained` (entry 23) and `installed_interrupts::validate` (entry 24).
- Any checker of non-call operations in the installed RAM closure besides `ram::validate` (entry 27).
- The start of the footprint function that contains `entries.rs:328`, and the `MachineAlternativeFamily` declaration (entry 30).
- `unit_entry_form` (entry 31) and `patch_bytes::write_u64` (entry 37).
- The PE planner beyond `plan_pe_sections`, `write_pe_headers`, and `emit_pe_x86_64` (entry 36).
- The reads/writes match at `machine_effects.rs:729` arm by arm (entry 38).
- Callers of `post_handoff_writer` `load`/`store` and `layout_plans::validate_fragment` (entry 39).
- `selected_row_requirement_identities` (mentioned under entry 6).
- Each production caller of `validate_boundary_entry_plan` other than `foreign_calls.rs:472-483` and the wrapper in `plans.rs` (entry 14). The grep list was not opened one by one.
