# Area 03: accepts when it does not know

Read-only trace of every entry classed `open` in `.agents/atlas/compiler-2026-11-10/digests/area_03.md`. Citations in that digest are at `457adf4a11f`. This checkout is `8febfc42316`, which contains that commit. The sites opened below still sit at the cited lines. Paths are under `omega-rust/` unless noted. "Read" means the lines were opened. "Inferred" means a conclusion from those lines, not a second site.

`A-SFE` is `omega/compilation/backend/instruction-set-architectures/isa-aarch64/src/selected_form_encoding.rs`. `X-SFE` is the x86-64 file of the same name.

## Summary

| # | Site | Verdict | Reason |
| --- | --- | --- | --- |
| 1 | A-SFE:2445-2457, 15409-15412 | CLOSED | The reference travels in the fixup, copied from the same instruction, and later equality checks require it. |
| 2 | A-SFE:12902 | CLOSED | The accessor contract is checked when the realization is built, then only copied. |
| 3 | A-SFE:15802 | CLOSED | Callers bind boundary and ordinal to the foreign-call record before encode or validate. |
| 4 | A-SFE:15974-15985 | CLOSED | The add's base register is operand 0 of the same placement list the view check uses. |
| 5 | A-SFE:10798-10799, 12228-12236 | CLOSED | Address decode accepts one sequence per offset, and the zero-page store compares bytes exactly. |
| 6 | A-SFE:9770 | CLOSED | The `(0, 0)` arm is only for carriers of 32 bits or more, and the only caller requires fewer than 32. |
| 7 | A-SFE:8043-8047 | CLOSED | Register resolution rejects any index above 30 before the request returns. |
| 8 | A-SFE:2287-2295 | CLOSED | A non-listed condition falls through into masks that cannot match that word, so decode fails. |
| 9 | A-SFE:7138, 7432, 7967, 7407, 8004 | CLOSED | Hosted catalog rows exist only for exact Linux arm64 and macOS arm64. |
| 10 | A-SFE:4928-4932 | CLOSED | The extra clobbers are damage the one system-register word does not do; nothing is accepted by them. |
| 11 | `isa-x86_64` `machine_effects.rs:1395-1403` | CLOSED | Kinds that reach the wildcard are the ones the encoder also records as flag-free and memory-free. |
| 12 | `isa-x86_64` `frame_protocol.rs:390-392` | CLOSED | When alignment is 1 the member offset is already inside the displacement that is checked. |
| 13 | `isa-x86_64` `generated_entry.rs:539-541` | CLOSED | The computed local target must land on an instruction start before decode returns success. |
| 14 | `isa-x86_64` `generated_entry.rs:528-542` | CLOSED | A non-body call is a local call only if that landing check passes, and the body field must still match. |
| 15 | X-SFE:2843-2927 | CLOSED | Operand count is required to equal the form's count before any register index is used. |
| 16 | `isa-x86_64` `post_handoff_writer.rs:118-138` | CLOSED | The fragment check allows 3, 5, 6, and 7 byte containers, and the byte loop is their encoder. |
| 17 | `layout/src/lib.rs:138-143` | CLOSED | `for_target` returns `Ok(None)` only for a natural layout with no authored realizations. |
| 18 | `layout/src/boundary_values.rs:215-217` | UNDECIDED | The size overwrite does not recheck field fit; the codec check is not on every in-memory path. |
| 19 | `ME/exit_contract.rs:2584-2597` | UNDECIDED | The `NoneV1` arm was read; the earlier arms of the same match were not, so a current kind may still fall through. |
| 20 | `ME/exit_contract.rs:2332, 2376` | REACHABLE | Copy and scan succeed here with no trap or operand-list check, and the function then returns success. |
| 21 | `object-file` `names.rs:79-85` | UNDECIDED | An invalid handle becomes an empty name; one caller rejects a non-unique match, two callers were not read. |
| 22 | `direct_boundary_leaves.rs:89-91` | UNDECIDED | An intrinsic returns success without looking at its identity; the two production callers were not read. |
| 23 | `direct_boundary_leaves.rs:96, 116` | REACHABLE | An empty selected calling roster skips the comparison with the binding's applications. |
| 24 | `synchronous_invocations.rs:200-205, 108` | REACHABLE | An invocation that matches no selected plan adds no edge and no diagnostic, and a missing package matches every package. |
| 25 | `target_providers.rs:643-653, 698-722` | REACHABLE | An obligations row with no selecting declaration is applied to every owner of that package and template. |
| 26 | `provider_plan.rs:204-209, 284-289` | UNDECIDED | The memo can go stale if a public field changes after the first digest; no writer-after-read was searched. |
| 27 | `provider_plan.rs:867-882` | CLOSED | Schema validation ignores `Err` storage; the one realization arm rejects it. |
| 28 | `provider_plan.rs:841` | CLOSED | Schema validation only checks the number range; admission refuses a non-Linux target. |
| 29 | `provider_grants.rs:80-82` | REACHABLE | A root grant that names neither a plan nor a slot is dropped, and binding still succeeds. |
| 30 | `entry_signature.rs:265-267, 286-288` | UNDECIDED | A non-absent index that does not resolve hashes as empty; one caller interns, the codec caller was not read. |
| 31 | `executable-installation` `post_handoff_writer.rs:137` | CLOSED | Nothing in the tree calls the constructor, so no program builds a receipt for the claim check to trust. |
| 32 | `runtime-abi` `windows_provider.rs:14` | REACHABLE | Any image whose bytes contain `omega_host.dll` is treated as importing that library and gets the DLL staged. |
| 33 | `stack_composition.rs:193-235` | CLOSED | The function does not recompute the validation id, but the only production caller computes it from the same fields. |
| 34 | `task-plans` `lib.rs:319-334` | CLOSED | Unresolved calls stay in the demand on purpose, and the projection fingerprint that is checked includes them. |
| 35 | `register-model` `identities.rs:184-308` | REACHABLE | The environment identity hashes the directed-binary float key and not the directed-unary key, and nothing else hashes that field. |

## 1. Image address bytes ignore the reference

Verdict: CLOSED.

Read `validate_decoded` (`A-SFE:2410-2457`) and `encode_unchecked` (`A-SFE:15339-15412`). Both `ImageAddress { .. }` arms look only at the destination register. `byte_literal_address_words` (`A-SFE:15735-15741`, read) emits an `adrp` plus an `add` of zero. The reference is not an input to the bytes.

Search `encode_aarch64_selected_form(` and `validate_aarch64_selected_form_encoding(`: the production callers are `row_encoding.rs:571` and `validation.rs:471`. `exit_contract.rs:2855` is under `#[cfg(test)]` (the helper at `2797` is). `native_foreign_bridge.rs:400` passes only sign-extend kinds (read `383-393`).

Read `route_of` (`row_encoding.rs:220-255`): every `ImageAddress` goes to `ByteLiteralAddress`. Read `image_address` (`350-374`): it requires the kind, encodes, then `retagged` (`133-138`, read) replaces the state with `UnresolvedImageAddress` whose `reference` is the kind's reference. Read stage 07 `validation.rs:329-361`: the row is kept only when `fixup.reference == reference` and the fields match, and only then are the bytes validated. Read `ordinary.rs:532-556` and stage 08 `validation.rs:392-405`: both require the same equality.

The digest's guessed lines `row_encoding.rs:383-385` and `validation.rs:336-339` set patch offsets (read). They do not compare the reference. The comparison is `validation.rs:343-345`.

## 2. Accessor scalar drops the contract

Verdict: CLOSED.

Read `accessor_scalar_ordinary` (`A-SFE:12896-12925`). It keeps `form` and `byte_offset` and drops `contract` with `..`, then encodes the ordinary load or store. Search `accessor_scalar_ordinary(`: two calls, `encode_aarch64_selected_memory_form` (`12734`) and `validate_aarch64_selected_memory_form` (`12803`), both read. Search `encode_aarch64_selected_memory_form(`: one production caller, `row_encoding.rs:486`.

Search `AccessorScalarAccessRealization {`: one constructor, `compiler_builtins.rs:228`. Read `187-228`: the contract must equal `compiler_accessor_scalar_contract_identity` of the schema, placement, slot, and operation, or admission returns an error. Later stages copy that value (`control_flow_operations.rs:49-103`, `legalization_source.rs:1737-1742`, `selection_scalar_graph.rs:166-170`, all read). The encoder does not choose or replace the contract.

The digest is right that this function treats two contracts alike. A contract that does not match the sealed accessor never becomes a realization.

## 3. System call ignores boundary and ordinal

Verdict: CLOSED.

Read `validate_aarch64_selected_system_call_form` (`A-SFE:15789-15842`) and `call_alternative_key` (`selected-instructions/src/instructions.rs:395-407`). The key is family `NormalizedForeignCall`, variant 0. Boundary and ordinal are not in it. The digest is right about the key.

Search `encode_aarch64_selected_system_call_form(` and `validate_aarch64_selected_system_call_form(`: production callers are `row_encoding.rs:870`, `validation.rs:796`, and `fragment_custody.rs:219`. `exit_contract.rs:3216` is inside `#[test]` at `3176`.

Read `normalized_foreign_call_record` (`row_encoding.rs:773-793`): the record at `ordinal` must be this instruction, this `boundary`, this target, and a valid entry. Both stage 07 encode (`848-878`) and validate (`742-807`) call that function and pass `binding.number` from that record. Read `fragment_custody.rs:56-78`: the record is the unique one for the invocation site and boundary, and the instruction kind must carry that boundary and ordinal, before `219`.

## 4. Table-call add base versus the views

Verdict: CLOSED.

Read `validate_aarch64_selected_table_call_form` (`A-SFE:15945-16004`) and `table_address` (`16006-16045`). The add word is checked against `base` from `table_address`, which is `operand_placements().get(0)` and must be one `Aarch64X` register. Callbacks make `table_address` return `None` (`16009-16011`).

Read `foreign_argument_registers` (`selected-instructions/src/calls.rs:55-82`) and `append_operand` (`263-272`). With no callbacks, placements are not filtered out. Float placements sort after integers. `get(0)` is either the synthetic integer `x16` when `dispatch_only` (`realizations.rs:347-352`, read) or the first parameter. That register is therefore constraint operand 0, stored as `fixed_view`. The validator requires every supplied view to equal that row's `fixed_view` (`15976-15982`). So the add base is `views[0]`.

`normalized_foreign_constraint` (`machine_effects.rs:193-218`, read) builds that row from the same record. The digest is wrong that nothing in the module checks the base: the check is the view zip, not a separate comparison with `views[0]`.

## 5. Frame-store and address-offset do not re-encode

Verdict: CLOSED.

Read `decode_address_words` (`isa-aarch64/src/immediate_offset.rs:148-177`) and `address_words` (`60-67`). A sequence is one chain of shifted adds whose page chunks are 4095 except the last, then at most one unshifted remainder. Immediate 0 is allowed only as the single first word. That is one sequence per offset. Inferred from those rules; the file's own test name at `241` says the same, and that test body was not read past the first assertion setup.

Read the frame validator (`A-SFE:10790-10818`). When `displacement >> 12 == 0`, `10798-10799` compares the prefix to `store_address_words` (`10610-10617`, read), which is a shifted add of immediate 0. That is an exact compare. The digest is wrong that those two lines call the decoder. The decoder rejects immediate 0 when shifted (`163-164`), which is why this path does not use it. When there are pages, the prefix length must equal that word list and the decode must equal the page part.

Read the address-offset validator (`12228-12236`): word count must equal `address_word_count`, and the decode must equal the displacement.

## 6. Narrow bounds return zero for an unlisted carrier

Verdict: CLOSED.

Read `narrow_bounds` (`A-SFE:9764-9771`) and its only caller `narrow_truncation` (`9795`, search `narrow_bounds(`: definition plus one call). The only caller of `narrow_truncation` is `program` at `9695-9698` (search count 2), and that arm requires `target.bits() < 32`. Read `IeeeFloatIntegerCarrier::bits` (`target-operations/src/boundary/execution.rs:146-152`): the four unlisted carriers are 32 or 64 bits. They cannot reach `(0, 0)`.

## 7. Hosted write does not refuse register 31 in the request

Verdict: CLOSED.

Read `request` (`A-SFE:8018-8047`) and `decode` (`8153-8170`). Decode returns `None` when the extracted register is 31. Request gets the register only from `resolve_registers` (`15293-15317`, read), which parses an `x` prefix and rejects an index above 30. Search `resolve_registers(` inside the write-byte request: the one call at `8043`. Register 31 is not returned.

## 8. SetBoolean falls through for other conditions

Verdict: CLOSED.

Read `decode_word` from `2093` through the error at `2407`, including the arm at `2287-2295` and `CONVERSIONS` (`2084-2091`). A word that matches `0xffff_0fe0 == 0x9a9f_07e0` has bits 16-31 fixed to `0x9a9f`. Every pattern after the arm requires a different top half (`0x9280`, `0xd280`, `0xf280`, `0xaa`, `0xf1`, `0xeb`, `0x8a`, `0xca`, `0x8b`, `0x91`, `0xcb`, `0xd1`, or the conversion constants, all read). None can match. The word becomes `MalformedEncoding`. The digest's overlap was checked by reading those masks.

## 9. Hosted effects use the Linux register unless the target is macOS

Verdict: CLOSED.

Read the three `effects` functions (`A-SFE:7128-7142`, `7420-7436`, `7955-7971`) and the size and declaration arms (`7405-7416`, `7992-8013`). Anything that is not `macos_arm64()` selects `x8` and the Linux size.

Read `aarch64_selected_constraint_keys` (`machine_effects.rs:424-500`). Hosted keys are `Some` only when the target is exactly `linux_arm64()` or `macos_arm64()`. Otherwise they are `None`. Read `declaration_keys` (`selected-instructions/src/effects/catalog.rs:403-448`): a `None` key from `for_semantic` contributes no declaration. The catalog builder (`machine_effects.rs:341-372`, read) calls the hosted `declaration` functions only for semantics that have a key. For those two targets the `else` branch is Linux, and the `if` branch is macOS. `aarch64_selected_abi` (`409-421`, read) does admit any AArch64 ELF or Mach-O, but the hosted keys are a second exact-target test. A third target does not get a hosted row.

## 10. System-register effects declare extra clobbers

Verdict: CLOSED.

Read `effects` (`A-SFE:4926-4953`) and `encode` (`4980-5010`). Encode returns one `mrs` or `msr` word. The declared clobbers name `x9` and `x15` on a read, plus `x10` and `x11` on a write. Those registers are not written by that word. Declaring them killed does not let a program through; it records more damage than the word causes. The digest's description matches what was read.

## 11. x86 generic effects wildcard

Verdict: CLOSED.

Read `encoded_effects` (`isa-x86_64/src/machine_effects.rs:1123-1403`) and `alternative` (`1105-1120`). Search `encoded_effects(`: the call at `1119` and the trapping helper's own method. `declaration` (`962-1057`, read) is the only caller of `alternative`, and the catalog calls `declaration` only from the final `else` at `751` (read `619-756`). Memory, calls, hosted forms, atomics, and register operations return earlier and do not use this function.

The wildcard at `1395-1403` is no reads beyond the first match, no flags, no memory, never-trap, fallthrough. Kinds that reach it from `declaration` include materialize (except variant 1, which is named), image address, copy and extends, bitwise not, byte-view address, wrapping and exact add. Read the encoder footprint (`X-SFE:3957-3974` and `4080-4114`) and the byte arms (`13945-13947`, `14059-14068`): those are `lea`, `mov`, or `not`, and the footprint's flag bit is false. The wildcard agrees with that footprint. Accessor scalar is in `declared_semantics!` (`32-34`, read) and is built by `memory::declaration`, not this function.

The digest's "newly added semantic" is not a current program.

## 12. Alignment 1 ignores the member offset

Verdict: CLOSED.

Read `validate_frame_address` (`frame_protocol.rs:376-392`) and `encode_x86_64_aligned_frame_address` (`349-373`). Search `validate_frame_address(`: the definition and `X-SFE:14601`.

Read the frame encoder (`X-SFE:14508-14549`): alignment 1 calls `encode_frame_address_signed` and does not pass `member`. Read `resolve` (`frame_address.rs:388-422`): when `frame_alignment_bytes == 1`, `byte_offset` is added into the displacement. Read `validate_address` (`538-561`): that same displacement must be `start + byte_offset` when alignment is 1, and `start + 0` otherwise. The member the byte checker ignores is already the displacement it checks.

## 13. Generated local call has no range check at the add

Verdict: CLOSED.

Read `decode_x86_64_generated_entry` (`generated_entry.rs:519-552`). The target is computed at `539-541` with no range test there. The function always ends in `GeneratedEntryEvidence::from_target_instructions` (`551`). Read that function (`calling-conventions/src/plans.rs:256-311`): a `LocalCall` or `Branch` whose target is not some instruction's `byte_offset` returns an error (`295-305`). Search `from_target_instructions(`: the x86 decode, the AArch64 twin, and the definition.

## 14. A call that is not the body field becomes a local call

Verdict: CLOSED.

Same function, read. A `Call` whose field is not `body_call_field` becomes `LocalCall` (`528-541`). `from_target_instructions` then requires the remaining `Call` fields to equal `body_call_field` (`292-294`) and the local target to land on an instruction (`295-305`). If `body_call_field` is `None`, any `LocalCall` is an error (`270-274`). The test `local_prefix_call_and_body_relocation_have_distinct_exact_custody` (`635-657`, read) accepts a zero-displacement prefix call and rejects displacements 1 and 127. That classification is the defined behavior of this decoder, bounded by the landing check.

## 15. Atomic access indexes registers with no length test in the arm

Verdict: CLOSED.

Read the `AtomicAccess` arm (`X-SFE:2842-2928`) and the compare-exchange continuation (`2906-2928`). It indexes `registers[0]`, `[1]`, `[2]`, and for compare-exchange `[3]` and the outcome operand. Search `validate_decoded(`: the definition, the accessor recursion at `2822`, and the call at `23991`.

Read `validate_x86_64_selected_form_encoding` (`23943-23991`): `validate_request` runs first. Read `validate_request` (`17748-17762`) and the atomic count (`17793-17796`), which is `form.operand_count()`. Read `operand_count` and `input_count` (`selected-instructions/src/atomic.rs:89-99, 232-236`). Load and store have 2 registers and use indexes 0 and 1. The other two-input operations have 3 and use up to 2. Compare-exchange has 4 and uses up to 3. Compare-exchange-once has 5 and its outcome index is 4. A short list is `OperandCountMismatch` before the index.

## 16. Post-handoff byte loop for other widths

Verdict: CLOSED.

Read `load` and `store` (`post_handoff_writer.rs:117-165`) and `append_fragment` (`12-84`). The loop runs when `container_bytes` is not 1, 2, 4, or 8. Before that, `validate_fragment` is called (`49-58`). Read `validate_fragment` (`psi/foundation/layout-plans/src/materialization.rs:1351-1393`): container width must be a positive multiple of 8 and at most 64. That allows 24, 40, 48, and 56 bits, which are 3, 5, 6, and 7 bytes. The loop encodes those widths. It does not accept a width the fragment check rejects.

## 17. No authored layout returns None

Verdict: CLOSED.

Read `terminal_layout_identity` (`layout/src/lib.rs:129-142`). `Ok(None)` is only `for_target` returning `None`. Read `for_target` (`psi/compilation/representations/terminal-psi/src/lib.rs:636-656`). `Ok(None)` is only an absent policy identity together with an empty realization list. A present policy with no row for the target is an error (`656`). A natural record that still has realizations is an error (`644`). Read the callback caller (`terminal_callback_layouts.rs:124-169`): `None` takes the natural field-offset path and errors if a field has no single offset. Search `terminal_layout_identity(`: this function, the callback caller, and a test helper.

The digest is wrong that an authored record can take the `None` path. That path is the absent-policy case.

## 18. Supplied storage overwrites packed size

Verdict: UNDECIDED.

Read `record` (`boundary_values.rs:175-245`). After fields are summed, `215-217` replaces `size` and `alignment` with `storage` when it is present. Nothing after that compares each field's end with `storage.byte_size`. Read `foreign_record_storage` (`terminal-psi/semantics/src/storage.rs:2214-2248`): it forwards `Authored` storage and does not check field fit.

Read `validate_authored_field_extent` (`terminal-psi/codec/src/foundation_validation.rs:8356-8438`): an authored field whose end is past `record_size` is rejected. Search `validate_structural_foundation(`: one caller, `decode_module` (`semantic_module.rs:4109`). That is the byte-decode path. In-memory constructors of `StructuralRecordStorage::Authored` were found by search (`attached_unit.rs:3792` and others) and not read. Whether a record built in memory can reach `boundary_values.rs:215` with a field past the authored size is what is open.

## 19. NoneV1 accepts kinds outside a hand list

Verdict: UNDECIDED.

Read `validate_non_return` (`ME/exit_contract.rs:2094-2606`) through `expected_control` (`2106-2142`) and the `NoneV1` arm (`2584-2597`). The arm returns true for any kind not in that exclusion list when the memory effect is `NoneV1`. The match's final arm is `_ => false` (`2598`). `expected_control`'s default is `FallThroughV1` (`2142`). The arms from `2184` to `2583` were not read, so it is not known whether every current memory-touching kind is either matched earlier or named in the exclusion list. Reading those arms would decide it.

## 20. Copy and scan return true

Verdict: REACHABLE.

Read the arms at `2332` and `2376`, and the following `ZeroBytes` arm (`2333-2343`) which does check trap and operand lists. Copy and scan do not. After `memory_matches`, the function checks control (`2603-2605`) and returns `Ok(())` (`2606`). Copy and scan are not named in `expected_control`, so the expected control is `FallThroughV1` (`2142`). A byte-copy or byte-scan whose memory-effect shape matches, whose control is fallthrough, and whose trap behavior or operand lists do not match that operation, is accepted by this function.

Search `validate_non_return(`: calls at `638`, `1130`, and three test sites. Those two production callers were not read. The accept is already the `Ok` of this function.

## 21. Invalid symbol handle becomes an empty name

Verdict: UNDECIDED.

Read `object_symbol_name` (`object-file/src/names.rs:79-85`). An invalid handle returns `""`. Search `object_symbol_name(`: `image.rs:945`, `image.rs:996`, `entries.rs:3936`, and the definition.

Read `image.rs:944-958`: the name is matched to compiler regions, and any count other than one is an error. An invalid handle therefore fails there unless exactly one region has an empty symbol. Whether an empty symbol can be created was not read. `image.rs:996` and `entries.rs:3936` were not read. Those two callers would decide it.

## 22. Intrinsic skips the calling-roster checks

Verdict: UNDECIDED.

Read `validate_calling_custody` (`direct_boundary_leaves.rs:25-130`) and `DirectBoundaryExecution` (`19-22`). Line `89-91` returns `Ok(())` unless the execution is `Foreign(Ok(_))`. That includes every `Intrinsic`. Read `validate_bound_calling_custody` (`132-156`): it calls the same function and then counts boundary machines. It does not read the intrinsic identity.

Search `validate_calling_custody(`: `provider_rows.rs:208`, `retained_terminal_codec.rs:249` and `277`, a test, and the definition. The two production callers were not read. Whether one of them rejects a wrong intrinsic identity before or after this `Ok` is what is open. The roster checks below line `91` are only for a foreign `Ok` binding; skipping them does not by itself accept a mismatched foreign roster.

## 23. Empty calling roster compares against nothing

Verdict: REACHABLE.

Read `96-128`. `calling_for_requirement` is compared with the binding's applications only when `expected` is non-empty (`96` and `116`). When it is empty, each application is still checked for a repeated ABI, the requirement identity, and a recomputed commitment (`101-115`), and then the roster comparison is skipped. A direct foreign binding whose source records no calling roster is accepted even when its applications are not a selected roster, because there is no roster to fail against.

## 24. Unmatched synchronous target, and a missing package

Verdict: REACHABLE.

Read `matches` (`synchronous_invocations.rs:102-118`) and the cycle loop (`194-217`). Search was the read of this file's two functions; callers of `validate_selected_synchronous_invocation_cycles` were not searched.

At `108`, `package.is_none()` makes the owner comparison true for every package. An invocation recorded with no package matches a trait of that name in any package. At `200-201`, `[]` adds no edge and no diagnostic. The function returns `Err` only when `diagnostics` is non-empty (`217`). A synchronous invocation whose target matches no selected plan is dropped, and a cycle through that target is not reported.

## 25. Target filter and obligations with no selecting declaration

Verdict: REACHABLE.

Read the candidate filter (`target_providers.rs:620-653`) and the obligations loop (`687-756`).

The family test at `643-653` is a restriction: a callable that appears in a family must appear there for this target, and a callable that appears in no family passes because every family's `!any` is true. That is not an accept of the wrong target. A callable listed only for another target fails the same predicate.

The obligations filter at `704-707` uses `is_none_or`. A row whose `selecting_declaration` is `None` passes for every owner. Its `required_instances` are then demanded (`724-756`), and the plan is accepted when each requirement has exactly one candidate. An obligations row that names no selecting declaration is applied to every provider owner in the same package and template, including an owner that row does not select.

## 26. Digest memo stays after a public field changes

Verdict: UNDECIDED.

Read `ProviderPlan` (`provider_plan.rs:82-101`), `Clone` for the memo (`104-107`), `identity_digest` (`204-208`), and `report_fingerprint` (`284-288`). The fields of the plan are public. The digest and fingerprint are `OnceLock`s filled on first read. `Clone` drops them. A write to `name`, `rows`, or another public field after the first `identity_digest()` keeps the old digest. No search was made for a caller that writes those fields after calling `identity_digest` or `report_fingerprint`. That search would decide whether a program can observe the stale value.

The file is `plans/effects/src/provider_evidence/providers/provider_plan.rs`, not `plans/effects/src/provider_plan.rs`. The digest's path is one directory short.

## 27. Table binding storage may be an error string

Verdict: CLOSED.

Read the binding enum (`provider_plan.rs:54-59`) and `validate_candidate_against_schema` (`867-882`). The arm checks that `table` and `field` are non-empty. It does not look at `storage`. The digest is right about this function.

Read `field_offset` (`provider-planning/src/provider_planning/external_binding_rows.rs:211-217`): `storage.as_ref().map_err(...)?` rejects `Err`. Search `field_offset(`: one call, at `141`, from the `VtableField` and `TableFunction` arm (`124-148`, read). Realizing either binding with `Err` storage fails there.

## 28. Syscall schema check is only the u32 range

Verdict: CLOSED.

Read `841-848`. A `Syscall` binding on any target string passes when `number` fits in `u32`. The digest is right about this function.

Read `conservative_syscall_terminal_mechanism` (`provider-admission/.../external_mechanisms.rs:110-124`): the target must be `LinuxX64` or `LinuxArm64`. Read `rejoin_system_call` (`settlements.rs:501-519`) and `native_syscall_for_target` (`calling-conventions/src/plans/vocabulary.rs:498-504`): Mach-O and Windows COFF have no syscall policy, so rejoin returns the "no supported kernel entry policy" error. A non-Linux program is not admitted through these paths. The AArch64 COFF arm panics; that pair is not one of the hosted targets.

## 29. A root grant that names nothing is skipped

Verdict: REACHABLE.

Read `resolve_selected_provider_grants` (`provider_grants.rs:64-119`). At `80-82`, a grant whose text is neither a candidate plan name nor a trait name hits `continue`. It is not an error. Search `resolve_selected_provider_grants(`: `binding.rs:68`, `accepted_machine_grants.rs:29`, `admission_settlement.rs:41`, a test, and the definition.

Read `bind_selected_provider_plan_facts` (`binding.rs:58-84`) and `validate_qualification_grants` (`87-113`). The qualification check walks the resolved grants, not the original strings. A dropped grant is absent from that list, and binding still returns the selected plans. A root grant that names neither a provider plan nor a trait slot is ignored, and provider binding succeeds.

The digest says this was already traced for `accepted_machine_grants.rs`. That caller was not re-read here. The `binding.rs` path was.

## 30. Missing identity text hashes as empty

Verdict: UNDECIDED.

Read `identity` (`entry_signature.rs:262-288`) and `validate_capture` (`313-323`). `is_absent` is required for the callable index and each parameter type index. Read `is_absent` (`terminal-psi/src/identity.rs:251-253`) and `text` (`354-356`): `text` is `None` when the index is past `entries`. `unwrap_or_default` then hashes `""`. `validate_capture` does not require `text` to be `Some`.

Search `from_checked_declaration(`: `program_entry.rs:236` and `retained_terminal_codec.rs:821` and `919`. Read `program_entry.rs:204-247`: that caller `intern`s into the same table it passes, and `intern` (`423-432`, read) always appends, so those indexes resolve. `retained_terminal_codec.rs:821` and `919` were not read. Whether a decoded signature can carry a non-absent index that does not resolve is what is open.

## 31. Preparation receipt takes unchecked pin flags

Verdict: CLOSED.

Read `from_admitted_provider` (`executable-installation/src/post_handoff_writer.rs:137-150`) and `validate_post_handoff_destination_binding` (`237-264`). The constructor stores `pinned` and `unpublished` as given. The claim check requires both to be true and does not compare them to the mapping. The digest is right about that trust.

Search `DestinationPreparationReceipt`: only this file. Search `from_admitted_provider` inside the file: only the definition. Nothing constructs a receipt, so no program reaches the claim check with these flags.

## 32. Windows provider test is a byte substring

Verdict: REACHABLE.

Read `image_imports_windows_internet_provider` (`runtime-abi/src/windows_provider.rs:12-18`). It is true when the image contains the bytes `omega_host.dll` at any alignment. Search `image_imports_windows_internet_provider(`: `publication.rs:221` and `223`, plus a test in the same file.

Read `publication.rs:219-238`. On a COFF target, a true result stages `omega_host.dll` next to the image unless a component already uses that filename. An image that does not import that library, but whose bytes happen to contain that name, is published with the provider DLL.

## 33. Stack-frame validation id is trusted

Verdict: CLOSED.

Read `validate_task_stack_frame_summary` (`stack_composition.rs:193-234`) and `TaskStackFrameSummary` (`175-181`). The function checks local size, alignment, call alignment, and that each unresolved site names this frame. It does not compare `summary.validation` with `task_stack_frame_validation_identity` (`237-243`, read the start). The test at `720-727` passes a forged id `29` and the function returns `Ok`.

Search `validate_task_stack_frame_summary(`: `activation.rs:405` and that test. Read `activation.rs:398-412`: the production caller computes the id from the same frame, sizes, calls, and unresolved sites it then stores. No other production caller passes a different id.

## 34. Activation validation does not mention unresolved calls

Verdict: CLOSED.

Read `validate_wcsu_activation_plan` (`task-plans/src/lib.rs:319-338`). It checks the candidate shape, `projection.has_valid_identity()`, and that the stack plan equals the projection. It does not call `unresolved_calls()`.

Read `has_valid_identity` (`stack_composition.rs:393-398`) and the fingerprint (`695`, the `hash_unresolved_calls` call). The identity that must match is computed from the demand, including its unresolved calls. Read the file header (`9-12`): an unresolved call is kept so the demand stays partial on purpose. Search `validate_wcsu_activation_plan(`: `activation.rs:93` and the definition. A partial plan is not a program this function was supposed to reject.

## 35. Directed unary float key is not in the environment identity

Verdict: REACHABLE.

Read `target_register_environment_identity` (`register-model/src/identities.rs:172-317`). The optional-key list includes `ieee_float_directed_binary` (`249`) and `ieee_float_unary` (`245`). It does not include `ieee_float_directed_unary`. The function ends at `from_canonical_bytes` (`317`). Search `ieee_float_directed_unary` under `register-model`: the field on `constraint_catalog.rs:150` and a test that sets it to `None`. No identity encoder hashes it.

Two register environments that differ only in that directed-unary constraint key get the same `TargetRegisterEnvironmentIdentity`. A program compiled against one of them is accepted as the other.

## Counts

- CLOSED: 22 (entries 1-17, 27, 28, 31, 33, 34)
- REACHABLE: 7 (entries 20, 23, 24, 25, 29, 32, 35)
- UNDECIDED: 6 (entries 18, 19, 21, 22, 26, 30)
- Entries not reached: none

## What I did not read

- The arms of `validate_non_return` from `exit_contract.rs:2184` to `2583`, and the production callers at `638` and `1130`.
- `image.rs:996`, `entries.rs:3936`, and the symbol-name constructor that would show whether an empty name can exist.
- `provider_rows.rs` around `208`, and `retained_terminal_codec.rs` around `249`, `277`, `821`, and `919`.
- In-memory constructors of `StructuralRecordStorage::Authored`, including `attached_unit.rs:3792`.
- Any caller that assigns `ProviderPlan`'s public fields after `identity_digest` or `report_fingerprint`.
- Callers of `validate_selected_synchronous_invocation_cycles` outside the function that was read.
- `accepted_machine_grants.rs` and `admission_settlement.rs`, beyond the search hit that names them.
- Test bodies, except the generated-entry test at `generated_entry.rs:635-657`, the stack-summary test at `stack_composition.rs:720-727`, and the signatures of a few `#[cfg(test)]` helpers used to exclude test callers.
