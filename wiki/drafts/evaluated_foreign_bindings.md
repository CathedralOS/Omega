# EVALUATED-FOREIGN-BINDINGS — resume recipe

Board row: `TASKS.md` `**EVALUATED-FOREIGN-BINDINGS.**` — the evaluated
locator values ride the
[normalized-import settlement contract](../spec/terminal-psi/boundary_calls.md#consumer-owned-settlement).
Recorded on linux x86-64 against `75650d2e94` (origin/main), claims snapshot
~2026-09-21T13:10Z.

The recipe exists because the producing chain for privileged port effects is
the missing slice, and every producing surface is fenced — by sibling board
items or by live claims (see the fence map). Resume by claiming the producing
surface again (or coordinating with its holder): `claim` on a contested path
returns exit 2 while it is live. Delete this recipe once the producer legs
land and a port-bearing artifact retains its exact effects through native
replay.

## Landed state — do not rebuild

Two halves of the item already landed; only the producer is missing.

**Consumed half.** `machine_code::boundary::ports::PortEffectRecord` binds one
privileged effect to its exact semantic service and byte span
(`psi_operation`, `service`, `port: u16`, `value: u8`, `operation_ordinal`,
`code_offset`, `byte_count`), and `MachineCodeFunction::port_effects`
(`machine_code/functions.rs`) retains them per emitted function. Object
construction rebases each row to its absolute `.text` offset
(`object_artifact/construction/text_emission.rs`) while
`object_artifact/construction/function_validation.rs::validate_port_effects`
enforces canonical ordering, provenance membership, and the exact
`x86_encoding::encode_immediate_port_write(port, value)` bytes at the recorded
span. The installation record constructs, codecs (format-36
`installation_record/codec/port_effect_codec.rs`), orders and scopes them
(`record_shape.rs::validate_port_effects` — ordered, unique per
`(machine, psi_operation)`, inside the function's text span, `byte_count`
equal to the encoded width), and requires record-vs-image equality
(`record_validation.rs`: `record.port_effects != image.port_effects()`
rejects). Native physical custody
(`native-artifact/src/physical/derivation/`) replays each
`MetadataOnlyPort` settlement against the object rows
(`provider_custody.rs::metadata_port_effect_custody`): exactly one effect must
rejoin (missing, duplicate, or substituted rows reject with "does not rejoin
one privileged port effect"), the settlement ordinal must be
`effect.operation_ordinal + 1` and `settlement.code_offset` must be
`effect.code_offset + effect.byte_count` (role-swapped children reject), the
joined Terminal operation must still be `PortWrite { service, port, value }`,
and `evidence.rs` reports any object effect no settlement consumed as
`NativePhysicalEvidenceGapSubject::UnownedPortEffect`.

**Admission policy.** A privileged port effect settles only beneath an
installed selected checked adapter (landed `c67ab1d0f2`): a direct-root
`PortWrite` rejects at the terminal-authority closure review with
"root-reachable checked physical operation has no selected provider
requirement custody" — pinned by
`immediate_port_io_rejects_without_provider_custody` on the linux x86-64
fixture `inline_asm/asm_port_out_final_validation` and the `physical-port`
leg of `artifact_identities_and_entries`.

**Locator custody.** PE, versioned ELF, and Mach-O import tables each carry
`(symbol, NormalizedForeignLocator)` pairs: the shared image builder holds
`FinalImageImportPlan::Normalized(NormalizedForeignLocator)`
(`image/src/final_image/symbols.rs`), consumed per format at
`image-pe/src/imports.rs`, `image-elf/src/imports.rs`
(`ForeignLocatorCandidate::ElfVersioned` plus a `LinuxArm64 | LinuxX64`
target-applicability check), and `image-macho/src/dyld_linking/imports.rs`.
The object side produces the pairs at
`object_artifact/construction/relocations.rs` and derives import symbol
spellings from the locator itself
(`object-file/src/names/mod.rs::normalized_foreign_import_symbol_name`) — raw
foreign bytes stay locator data, never Omega symbol names.

## Where it stops today

The chain reaches the checked-assembly boundary and stops:

- `OperationKind::PortWrite` lowers to `AbstractOperation::PortWrite`
  (terminal-psi-to-abstract-operations
  `lowering/machine/operation/effects.rs`) and the authority review
  classifies it `AuthorityEdge::CheckedPortWrite`
  (`terminal_authority_review/operations.rs::authority_edge`).
- `abstract-operations-to-target-operations` has no lowering arm for
  `AbstractOperation::PortWrite`: the `control_flow/operations.rs` dispatch
  falls through to `UnsupportedControlFlow`, so no
  `TargetUnitOperation::PortWrite` is ever pushed. The representation
  variant exists (`target_operations/operations/unit.rs`) and
  `lowering/unit/boundary_call.rs` *requires* a matching trailing `PortWrite`
  for every `MetadataOnlyPort` settlement — but nothing produces it.
- `target-operations-to-selected-instructions` cannot select it either:
  `legalization/scalar_graph_input/target/unit.rs::validate_operation` has no
  `PortWrite` arm and falls to `SourceCustodyMismatch`; there is no
  selected-instruction variant for a port write, and no register-home or
  post-allocation transport for one.
- `backend/machine-emission` never emits `encode_immediate_port_write` bytes
  and nothing constructs a `PortEffectRecord` for program functions; the
  artifact-level writer hardcodes `port_effects: Vec::new()`
  (`function_fragments/production.rs`). The private callback thunk lane
  (`native-realization/.../callback_thunks.rs`) also writes `Vec::new()`, which
  is correct by construction — `object_artifact/private_functions.rs` rejects
  any private function whose `port_effects` is nonempty.
- Corpus: `tests/omega/pass/ports` does not exist; only
  `fail/ports/asm_port_in_unsettled` and the `fail/inline_asm/asm_port_*`
  type-check rejections are registered. The positive fixture
  `pass/inline_asm/asm_port_out_final_validation` is a direct-root `out` that
  deliberately rejects at custody review, and
  `asm_runtime_port_msr_final_validation` (in `MACHINE_CONTROL_PASS_CANARIES`)
  stays fenced behind the missing transport.

## Resume path

The customer is a `PortIo`-declaring boundary requirement bound to a checked
adapter whose machine emits `out`; the direct-root fixture cannot become
positive without faking custody. Ordered legs:

1. Re-author the positive fixture: an adapter-routed `PortIo` machine under
   `tests/omega/pass/ports` (owned by the sibling row, see fences), keeping
   the direct-root rejection pins.
2. a2t: admit `AbstractOperation::PortWrite` in the
   `control_flow/operations.rs` dispatch, pushing
   `TargetUnitOperation::PortWrite` under the adapter's provider custody so
   `boundary_call.rs`'s settlement-follows-port-write join can find it.
3. t2s: select the port write — a `PortWrite` arm in
   `scalar_graph_input/target/unit.rs` plus a selected-instruction variant
   carrying `(psi_operation, service, port, value)`.
4. register-homes / post-allocation transport of the selected port write.
5. `machine-emission`: emit `encode_immediate_port_write(port, value)` at the
   operation's span and construct each `PortEffectRecord`; lift them into the
   artifact row set at `function_fragments/production.rs`.
6. The consumers engage unmodified: object validation rebases and checks the
   exact bytes, the record codecs them, and physical custody replay rejects
   the missing/duplicate/substituted/role-swapped mutations.

## Fence map at snapshot

Live claims covering producer legs:

- `representations/selected-instructions`, `representations/register-homes`,
  `selected-instructions-to-register-homes/{lib.rs,output,assignment`,
  `assignment/post_allocation_manifest,assignment/stack_slot_coloring`,
  `rewrites/rematerialization}`, and
  `selected-instructions-to-selected-instructions/{lib.rs` plus several
  `rewrites/}` — DURABLE-CODEC-RELOCATION (Devin / w10-w10-14, expires
  ~21:02Z). The selection and register-home legs ride that lane.

Board-recorded item fences (no live claim at snapshot, but the owning rows
still assign the surfaces):

- PRIVILEGED-PORT-EFFECT-SETTLEMENTS — `machine-code/boundary`,
  `object_artifact/construction`, `terminal_authority_policy`, and
  `tests/omega/{pass,fail}/ports`. Its remaining legs enumerate this same
  transport (PortWrite selection, register-home transport, machine-emission
  `out` encoding with port-effect records, and the re-authored positive
  fixture): the production writer and admission policy belong to that row.
- PSI-NATIVE-FIELD-STORES — `function_fragments`.
- PHYSICAL-ACCESS-PROFILES — `native-artifact/src/physical` (the row itself
  is marked resolved on the board).
- NORMALIZED-ABI-LOWERING imports leg — the normalized-import evidence tests.

## Acceptance

Port-bearing artifacts retain their exact effects; independent native replay
rejects missing, duplicate, substituted, or role-swapped children; raw
foreign bytes remain locator data, never Omega symbol names or ambient lookup
authority. The consumed-half replay machinery already witnesses the
rejections — only the producer needs to land.
