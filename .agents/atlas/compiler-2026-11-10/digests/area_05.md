# Area 5: Omega representations and foundation

Digest of 12 reader records: `unit_025.md`, `unit_025_gap_009.md`, `unit_025_gap_010.md`, `unit_026.md`, `unit_026_gap_011.md`, `unit_026_gap_012.md`, `unit_026_gap_013.md`, `unit_027.md`, `unit_028.md`, `unit_029.md`, `unit_030.md`, `unit_031.md`. Code at commit `457adf4a11f`. Nothing was built or run.

Path prefixes used below (all under `omega-rust/`):

| Prefix | Path |
|---|---|
| `CAND` | `omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/candidates.rs` |
| `CO` | `.../optimization-unit/semantics/src/current_ownership.rs` |
| `CVR` | `.../optimization-unit/semantics/src/current_value_ranges.rs` |
| `DR` | `.../optimization-unit/semantics/src/descriptor_results.rs` |
| `UV` | `.../optimization-unit/semantics/src/unit_validation.rs` |
| `SEM/` | `.../optimization-unit/semantics/src/` |
| `OU/` | `omega/compilation/representations/abstract-operations/optimization-unit/src/` |
| `AO/` | `omega/compilation/representations/abstract-operations/src/` |
| `MC/` | `omega/compilation/representations/machine-code/src/` |
| `PAM/` | `omega/compilation/representations/post-allocation-machine/src/` |
| `RH/` | `omega/compilation/representations/register-homes/src/` |
| `SI/` | `omega/compilation/representations/selected-instructions/src/` |
| `SIA/` | `omega/compilation/representations/selected-instructions/analyses/src/` |
| `TO/` | `omega/compilation/representations/target-operations/src/` |
| `LO/` | `omega/compilation/representations/target-operations/legalized-operations/src/` |
| `F/` | `omega/foundation/` |
| `I/` | `omega/interpreter/` |

Words such as "open", "traced", "stated", "suspected", "inferred" and "not traced" are the readers' own classifications and are kept as written.

---

## What this area does

This area holds the data shapes the Omega back end passes from stage to stage, plus the checkers that sit beside the first of those shapes, plus a handful of small support crates. The front of the area is the "abstract operation" form of a program and the "optimization unit" built from it (a control-flow graph per function with values, places, ownership events and proof facts). A very large validator crate re-derives everything it can about a unit (ownership, value ranges, rewrite legality) without trusting what the producer wrote. After that come the later shapes: target operations, legalized operations, selected instructions (with liveness, live-range and register-legality analyses), register homes, the post-allocation machine plan and machine code. Those later crates are mostly plain records, a byte-exact identity hash for each record, and a wire codec. The foundation crates supply shared hashing and optimization vocabulary, bounded child processes, safe file publication and timing counters. Two interpreter-side crates run native adapter code in a worker process and bind host functions for reference execution.

**abstract-operations** (5,079 lines). In: nothing computed; it defines `AbstractOperation` (about 75 variants, `AO/operations.rs:70-682`) and `AbstractOperationPlan` (`AO/lib.rs:37-46`). Out: metadata queries per operation (paths, storage mutations, psi operation id), atomic-event coherence replay (`AO/atomic.rs:414`), address-join and owner-loan classification, quotient-copy licensing (`AO/quotients.rs:9`).

**optimization-unit** (14,820 lines). In: an `AbstractOperationPlan`. Out: a `PsiOptimizationUnit` seed (`OU/construction.rs:43`), its content hash (`OU/identity.rs:2349`), the rewrite-candidate taxonomy and constructor-time custody checks (`OU/rewrite.rs:2014`), the transformation ledger and manifest codecs, value-range fact types, and a query layer over structural places (`OU/structural_places.rs`).

**optimization-unit-semantics** (43,116 lines; 42,996 in this area, see Not covered). Three file groups:
- `candidates.rs` (10,699): in, a unit plus a `PsiRewriteCandidate`; out, `ValidatedPsiRewrite` after replaying the rewrite on a clone and revalidating (`CAND:10362`, `CAND:10662`).
- `current_ownership.rs` (9,965) and `current_value_ranges.rs` (500): in, a function's current operations; out, the live ownership frontier at every block, node and edge (`CO:7560`), descriptor-loan and address-join conflict checks, and re-derived value-range facts (`CVR:139`).
- `unit_validation.rs` (18,270), `descriptor_results.rs` (2,907), `error.rs`, `lib.rs`: in, a whole unit; out, `Ok` or one `OptimizationUnitValidationError`. Phase order is fixed at `UV:18264-18269`; per-function order at `UV:9546-9673`. `descriptor_results.rs` adapts operations into a descriptor flow graph for a kernel in another crate (`DR:1215-2631`).

**target-operations** (3,751). In/out: type vocabulary joining Terminal Psi operations to ABI placements and boundary realizations. Judgment code is limited to `TO/descriptor_input.rs` (scalar-prefix guard), `TO/boundary.rs` (two layout checks) and `TO/boundary/realizations.rs` (`has_valid_entry_mechanism`, `supports_target`).

**legalized-operations** (9,464). In: nothing computed; defines `LegalizedOperationPlan` and its scalar graph (90-variant instruction enum, `LO/control_flow/scalar_graph.rs:416-959`). Out: plan identity bytes (`LO/identity.rs:46-64`) and call shape/source validation (`LO/validation.rs`).

**selected-instructions** (20,949). In: nothing computed; defines `SelectedInstructionPlan`, about 100 instruction kinds (`SI/instructions.rs:130-364`), paged copy-on-write storage (`SI/functions.rs`, `SI/blocks.rs`), the machine-effect catalog model and its validation (`SI/effects/catalog/validation.rs:2173`). Out: incremental plan identity (`SI/plan_identity.rs:40`), machine-effect plan wire format `OMGMFX` v49 (`SI/effects/program/encoding.rs:1894`).

**selected-instruction-analyses** (12,674). In: a selected program. Out: liveness, live ranges and interference, allocator availability, per-point register-view legality, fixed-precolored intervals/splits/segment homes, the first spill victim, recovery classification, and per-instruction machine effects. Each is a "compute, then seal" pair; the seal only hashes and counts (`unit_029` five-things item 1; `SIA/lib.rs:234`, `SIA/live_ranges.rs:1555`, `SIA/liveness.rs:2087`, `SIA/machine_effects.rs:877`).

**register-homes** (14,172). In/out: records and wire codecs for allocator availability, legality, register homes, spill choices, recovery classification, the fixed-view-copy plan (which embeds a whole re-encoded selected plan, wire v98, `RH/recovery/fixed_view_copy_codec.rs:54-55`) and the post-allocation manifest.

**physical-instructions** (`post-allocation-machine`, 2,284). In/out: `PostAllocationMachinePlan`, its identity and wire codec (`OMGPMX` v27, `PAM/codec.rs:35-36`), a descriptive cost model.

**machine-code** (6,124). In/out: records for function fragments, resolved layout, text-section placement, frames, exit contracts and boundary settlements, each with a SHA-256 identity; two manifest codecs.

**artifacts** (371). Timing collector and 11 process-wide counters (`F/artifacts/src/compile_timings/counters.rs:9-33`).

**bounded-process** (3,413). In: a prepared command and limits. Out: captured output under a deadline with the child's process group or Job Object reaped (`F/bounded-process/src/bounded_process.rs:59`).

**function-identity** (151). A 42-byte machine-function identity and its private symbol name (`F/function-identity/src/lib.rs:26-83`).

**optimization-core** (5,821). The registry of 55 optimizations and 9 phases (`F/optimization-core/src/optimization_core.rs:167-389`), the selection codec, about 40 domain-separated identity types, decision and pass records, rollback overlay. No passes.

**platform-custody** (1,153). Exact-byte file staging and install, lock files, capability-relative record files with identity checks, temporary directories.

**native-provider-runtime** (3,146). In: adapter image bytes and shared memory "homes". Out: calls executed in a bounded worker process over a framed channel (`I/native-provider-runtime/src/worker.rs:85`).

**terminal-runtime** (14,328; only 2,548 lines are in this area). In: a Terminal program and host provisions. Out: an observation. The part read here is the entry (`execution.rs:158`) and the host-import binding chain (`execution/host_imports.rs:371-661`).

---

## How it decides

### Keyed on identity (ids and content hashes)

- Unit validation, ownership replay and range checks key on `MachineId`, `PlaceId`, `BlockId`, `OperationId`, `ClaimId`, `StructuralTypeId`, `ValueId` (`unit_026.md` opening statement; `unit_026_gap_011.md` opening statement; `unit_025_gap_009.md` five-things item 1).
- Rewrite candidates must equal the validator's replay exactly: `candidate.affected_blocks() == expected_blocks` and `candidate.provenance() == provenance` (`CAND:10671`); patch rows equal replayed rows (for example `CAND:3049-3053`, `CAND:10160`).
- Joins in the ownership replay are whole-struct equality with the first arrival (`CO:8801-8818`).
- Every wire decoder recomputes a content hash and compares (`PAM/codec.rs:90-92`, `RH/lib.rs:410-455`, `RH/constraints.rs:102`, `SI/effects/program/encoding.rs:213-216`, `OU/manifest.rs:199`, `F/optimization-core/.../baseline.rs:174-178`). The hash is not a signature: a re-hashed forged record decodes `Ok` (`PAM/codec/tests.rs:846-855`, `SI/effects/program.rs:183-197`).
- Incremental reuse in the analyses keys on `Arc` pointer equality or `==` of a whole `SelectedFunction` (`SIA/lib.rs:921-929`, `SIA/lib.rs:1015-1048`); plan identity reuse keys on pointer equality and a `prefix_verified` flag (`SI/plan_identity.rs:232-235`, `SI/functions.rs:218-221`).
- Rule and validator identities in `candidates.rs` are byte-string constants compared as bytes or as their hashes: 7 site groups (`CAND:453-455`, `2736-2756`, `4806-4859`, `5523-5536`, `6493-6529`, `7943-7978`, `8413-8996`). The record says "Identity keyed on canonical bytes, not display names" (`unit_025.md`, section 1). These are compiler-internal names and are not counted as source spelling below.

### Keyed on place (place id plus structural path)

- Ownership: owned places, partial custody paths, claims by `(place, path)` with prefix and overlap tests (`CO:7674`, `CO:8064`, `CO:8326`); descriptor loans by `(BlockId, node)` position (`CO:1083-1139`); reference custody by carrier `(place, path)` and `ReferenceOrigin` (`CO:5137-5162`).
- Structural call arguments: about twenty "presentation" booleans derived from the source contract of the argument place and combined into an expected multiplicity (`UV:13294-13773`).
- Field value proofs: establishing operation of a place followed through leaf copies and single-source block parameters (`OU/structural_places.rs:1329-1372`).
- Atomic coherence: reaching writes of a location computed backward over predecessors (`AO/atomic.rs:517-603`).

### Keyed on value

- Constant folding and range folding on integer values and intervals (`CAND:8022-8027`, `CAND:8200-8205`, `CVR:234-252`).
- Law literals: operand must be a materialized integer literal node equal to the law constant (`CAND:5460-5499`).
- Work budgets and limits by numeric comparison (`SIA/work_accounting.rs:84`, `F/bounded-process/src/preparation.rs:208-281`).

### Keyed on position (order, index, tag byte)

- Strictly ascending rosters everywhere: functions by machine (`UV:1114-1122`), blocks by id (`UV:5628`), types (`UV:16208-16214`), facts (`UV:9697-9728`), services (`UV:15777-15865`).
- Operand position and count for machine-effect declarations (`SI/effects/catalog/validation.rs:3045-3125`, `SIA/machine_effects.rs:390-425`).
- Wire tags: every decoder is a closed tag table that rejects unknown tags (`PAM/codec.rs:259-376`, `RH/recovery/selected_instructions.rs:207-525`, `SI/effects/program/encoding.rs:339-657`).
- Cleanup order: action groups must appear in `cleanup_root_order` (`CO:3714-3718`).
- Host import binding: first adapter in a fixed ordered chain that returns `Some` wins (`I/terminal-runtime/src/execution/host_imports.rs:455-606`).

### Keyed on spelling: 16 distinct sites

Counted: distinct code sites the records cite where a decision or an identity depends on a text string. Test-only sites are marked.

| # | Site | What is compared |
|---|---|---|
| 1 | `UV:16527` | Content-projection path segments are strings resolved with `identities.index_of(segment)`. |
| 2 | `UV:16901-16947` | Content-claim path segments compared as text: `identities.text(..).unwrap_or_default()` against `ContentPlaceSegment::Field(String)`. |
| 3 | `SIA/legality.rs:167-175` | Rematerialization availability picks views by register name `"rax","rcx"` / `"x0","x1"` through `view_named`. |
| 4 | `SI/calls.rs:1407-1411` | Dynamic-call selector register named `"x16"` (AArch64) or `"r11"` (x86-64). |
| 5 | `SI/fixture.rs:55-61, 76-81` | `view_named(&str)` for program counter and foreign operand; test support only. |
| 6 | `RH/recovery/structural_calls.rs:875-941` | `decode_call_binding` branches on a wire string: empty = system call, `"@table"`, `"@component"`, anything else = target-profile name. |
| 7 | `LO/identity/normalized_foreign.rs:151, 167, 174` | Encoder side of the same discriminant strings; `target_name()` values. |
| 8 | `LO/identity/structural.rs:104, 232, 240, 267` | `ContentPlaceSegment` strings, `ContentAlgebra.parameter` and the `TargetFamily` target string are written into identity bytes. |
| 9 | `LO/validation.rs:226` | `TargetFamily` origin accepted when the `target` string is non-empty. |
| 10 | `F/artifacts/src/compile_timings/mod.rs:120` | Timing rows merged by phase label equality (metrics only). |
| 11 | `F/optimization-core/src/optimization_core.rs:139-143` | `from_build_case_name`: linear string compare over the 55 build.omg case names. |
| 12 | `F/optimization-core/src/optimization_core/rollback/request.rs:25-27` | Rollback names resolved by that exact string match. |
| 13 | `I/terminal-runtime/src/execution/host_imports.rs:192` | `matches_platform_c_symbol`: normalizes `libc.so.6`, `GLIBC_2.2.5`/`GLIBC_2.17`, Darwin install names, Windows library names to a locator and compares. |
| 14 | `I/terminal-runtime/src/execution/host_imports/absolute_value.rs:221-267` | Own copy of the same locator derivation for C `abs`. |
| 15 | `I/terminal-runtime/src/execution/host_imports/allocation/admission.rs:16-37` | Recognizes `calloc`/`free`/`memset` with libSystem and `msvcrt.dll`. |
| 16 | `I/terminal-runtime/src/execution/host_imports/affine_transform.rs:29-48` | CoreGraphics `_CGAffineTransformMake`, macOS aarch64 host only. |

Sites 13 to 16 compare foreign symbol and library names, not Omega source names. `unit_026.md` and `unit_026_gap_011.md` each state they saw no judgment keyed on `display_name`, labels or `format!` of a debug value in their ranges; the only two in `unit_validation.rs` are sites 1 and 2 (`unit_026_gap_012.md`).

---

## The same job in more than one place

"Checked" means I opened both sites. "Suspected" is the reader's word and the other site was not read.

### Graph and dataflow routines

1. **Dominator sets.** `CAND:3224` (`independent_reachable_dominators`: reachable blocks only, `BTreeMap<BlockId, BTreeSet<BlockId>>`; users `CAND:657`, `CVR:16`, `CVR:121`, `CO:1975`, `CO:2799`) and `OU/structural_places.rs:571` calling `OU/dominance.rs:53` (packed bitsets, all declared blocks, re-exported for the validator at `UV:2678`). Checked: same job. They differ in representation and in unreachable blocks (the bitset version keeps greatest-fixed-point rows, `OU/dominance.rs:66-72`). `unit_027.md` also lists 22 grep hits for `dominators(` in 14 files including psi and pipeline 01 (suspected, not read). `CVR` recomputes the map on every call (`CVR:101`, `CVR:112`, `CVR:295`).
2. **Does this block graph have a cycle.** `CAND:9584`/`9606` (DFS with marks), `UV:5770-5792` (Kahn's algorithm inside `validate_total_cfg`), and pipeline 01 `control_flow_analyses.rs:133`. Checked the first and third: same question, different algorithm (the pipeline one reads strongly-connected components).
3. **Block reachability.** `UV:5751-5757` inline, `UV:5806-5822` (`block_reaches`), `CAND:3241-3249`, `CAND:9311-9395` (inside SCCP), `CAND:1834-1879` (call graph).
4. **Hand-written fixpoints in `candidates.rs`.** Cycle `9606`, dominators `3224`, post-dominators `1649`, liveness `5774`, SCCP `9223`: five (`unit_025.md`).
5. **Transitive closure of a place map.** `CO:1959-1974` and `CO:2069-2083`, same shape.
6. **Type-graph walk with an active/complete cycle guard.** `UV:9905` (`finite_owned_shape`), `UV:11322` (`record_type`), `UV:11409` (`constructible`), `UV:17845` (`validate_structural_type_graph`), `CO:9183` (`storage_cannot_retain_loan`), `AO/atomic.rs:112` (`contains_only_scalars`), `AO/control_flow/owner_loans.rs:103` (`view_carrier_shape`), and `psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:11741` (near copy of `UV:17845`, read by the gap_013 reader). They differ in which shapes and field kinds are allowed.
7. **`attachment_storage_types`.** `UV:17908-17947` and `foundation_validation.rs:8566-8605`. Checked with a diff: identical apart from `pub(super)` versus `pub(crate)` and one `use` line.

### Per-operation tables

8. **"What does each `AbstractOperation` touch."** `OU/construction.rs` extractors (`:345, :406, :492, :527, :612, :745, :914, :998`), `OU/observation.rs:335-440`, `OU/identity.rs:3063-5108`, `AO/operations.rs:904, :955, :963, :1089`, `AO/storage_mutations.rs:18-86`, `CO:1252-1466` (`operation_observations`), `CO:5995-6461` (`apply_operation`), `CO:9630-9730` (`observations`), `CO:7831-7910`, `UV:6748` (`operation_place_accesses`), `UV:5848-5944` (fact index), `UV:1482-2117` (range authority), `CAND:10211-10339` (`rewrite_value_operands`). Exhaustive with no wildcard: `AO/operations.rs:1089`, `OU/identity.rs:3063`, `OU/observation.rs:349`, `UV:6748`, `UV:11848`, `CAND:10211`. The rest end in a wildcard. `unit_025_gap_009.md` read `CO:1252` and `CO:5995` and says the lists are not identical (`ElementViewRead` at `CO:1368` only). Checked: `ElementViewRead` appears in `CO` at `1368` and `9707` and nowhere in `5995-6461`.
9. **Value-use rewriters.** `CAND:2113`, `CAND:2223`, `CAND:7293`; they differ on `WriteOnlyPrimitiveStore { range_obligation: Some(_) }` (`CAND:2274-2277` only) and `StructuralCase` (`CAND:7315`).
10. **Operations that carry obligations.** `UV:5848-5944` and `UV:1482-2117`; "Whether they agree: not compared" (`unit_026_gap_011.md`).
11. **`MachineSemanticKind` parallel tables.** `SIA/machine_effects.rs:473`, `SI/effects/catalog.rs:163`, `SI/effects/catalog/identity.rs:575-710`.
12. **Instruction-kind tag table.** `RH/recovery/selected_instructions.rs:91-200` (encode) and `:207-525` (decode), `SI/effects/program/identity.rs:131-244`, `SI/effects/program/encoding.rs:339-657` and `:788-946`.
13. **Successor list per terminator.** `SIA/liveness.rs:27-45`, `SI/control_flow.rs:341-378`, `SIA/liveness.rs:1654-1665`.
14. **Early-clobber, tied and UseDef rules.** `SIA/liveness.rs:839-1047` and `SIA/live_ranges.rs:204-448` (`tied_component` at `:1049` and `:371`); tied registers rejected again at `SIA/fixed_precolored_split_requirements.rs:325` and `SIA/spill_choice.rs:109`. "The two copies are not textually identical" (`unit_029.md`).

### Byte encoders

15. **Structural path segments.** `OU/identity.rs:189-230`, `OU/ownership.rs:148-194` (same tags as the previous), `LO/identity/structural_types.rs:187-225` (RuntimeIndex 5), `SI/effects/program/identity.rs:455-492` (RuntimeIndex 6), `SI/plan_identity.rs:587-622` (Referent 2, FixedRange 3, Field 0, FixedIndex 1, RuntimeIndex 4), and terminal-codec `structural_wire.rs` (not read). The decoder at `SI/effects/program/encoding.rs:1646-1665` accepts tags 1, 2, 3, 4, 6, 7 only.
16. **`ValueDefinitionSite`.** `OU/graph.rs:50` (tags 0/1/2), `OU/identity.rs:2877` (1/2/3), `OU/rewrite.rs:159` (1/2/3).
17. **`IntegerType` and `IntegerValue`.** `OU/identity.rs:125-144` and `OU/range.rs:390-413`. Checked: `identity.rs` writes sign (Unsigned 1, Signed 2) and bits and no carrier; `range.rs` writes carrier, sign (Signed 1, Unsigned 2), bits. Value tags are also reversed.
18. **Scalar type.** `LO/identity.rs:157-172`, `SI/plan_identity.rs:716-739` (same tags as LO), `RH/recovery.rs:391-411` and `:636-658` (tags 0..4, sign folded in; decoder accepts 0..3 only), `RH/recovery/fixed_view_copy_codec.rs:464` (tags 0/1/2).
19. **`IeeeFloatFormat`.** Three encodings in register-homes: `RH/recovery/structural_declarations.rs:882-885, 941-945` (1|2), `RH/recovery/fixed_view_copy_codec.rs:456-459` (0|1), `RH/recovery.rs:652-655` (0|1).
20. **Work settlements.** `LO/identity.rs:66-81`, `SI/plan_identity.rs:699-714`, `SI/effects/program/identity.rs:507-520`; decoders `RH/recovery/fixed_view_copy_codec.rs:803`, `RH/recovery.rs:510-524`.
21. **Cleanup actions and ownership events.** `LO/identity/structural.rs:310-352` and `:275-308`; `SI/effects/program/identity.rs:385-433` and `:345-383`; decoders `RH/recovery/structural_contracts.rs:855` and `:835`, `SI/effects/program/encoding.rs:1600` and `:1554`.
22. **Crash site-guard predicates.** `LO/identity/scalar_graph.rs:1589-1596` and `SI/terminator_encoding.rs:56-63`.
23. **Terminator tags.** Encoder `SI/terminator_encoding.rs:17-123`; decoder `RH/recovery/selected_blocks.rs:95-162`. `unit_028.md` suspected a mismatch risk without reading the encoder; `unit_030.md` read the encoder. Comparing the two records' tag lists, the ten numbers agree.
24. **Frame-slot sub-tags.** `SI/kind_encoding.rs:52-74`, `PAM/codec.rs:336-362`, `RH/recovery/selected_instructions.rs:280-299`, `SI/effects/program/encoding.rs:506-510`. `Store` byte size is checked at `PAM/codec.rs:276` and `SI/effects/program/encoding.rs:388-390` and not at `RH/recovery/selected_instructions.rs:246-249`.
25. **`VirtualRegisterOrigin` tags.** Encoder `SI/values.rs:66-147`; two decoders `RH/recovery.rs:423-482` and `RH/recovery/fixed_view_copy_codec.rs:724-765`.
26. **`CallSiteOwner` tags.** `MC/lib.rs:123-154`, `SI/plan_identity.rs:799-867`, `LO/identity/scalar_graph.rs:1437-1477`, `RH/recovery/structural_calls.rs:189-217`: same numbers in all four. `InvocationSite` uses different numbers (`MC/lib.rs:156-174`).
27. **Call target.** `AO/calls.rs:96` and `OU/identity.rs:3595-3612`, different layouts.
28. **`NodeLocation`.** `OU/rewrite.rs:148`, `OU/rewrite.rs:1919`, `OU/ledger.rs:289-296`.
29. **Native target.** `RH/lib.rs:105-125` (`encode_target`), `NativeTarget::identity_bytes` (used at `MC/fragments/identity.rs:39`), `wire_identity_bytes` (used at `MC/fragments/publication.rs:223`); decoders `RH/recovery/fixed_view_copy_codec.rs:417-437` and `SI/effects/program/encoding.rs:1789-1813`.
30. **Branch arm and image-address hashing.** `MC/fragments/identity.rs:345-367`, `MC/layout.rs:308-325`, `MC/layout.rs:529-545`; image-address block at `MC/fragments/identity.rs:81-86` and `MC/layout.rs:327-332`.

### Checks written more than once

31. **Provenance row validation.** `OU/ledger.rs:170-214` and `OU/rewrite.rs:2338-2376`, "almost line for line" with different error enums (`unit_027.md`).
32. **`provenance_proven_unreachable_at`.** `CAND:10488` and pipeline 01 `control_flow_cleanup.rs:1491`. Checked: identical bodies.
33. **Access-supply table.** `UV:5126` (`covers`), `UV:14204` (`structural_access_can_supply`), `UV:14942` (`access_covers`). Checked: the last two give the same answers; `covers` differs in one cell (`MutableBorrow` does not cover `WriteOnlyBorrow`).
34. **Structural path walkers over types.** `UV:14846` (`leaf_type`), `UV:14646` (`access_path_type`), `UV:17178` (`resolve_path`), `AO/control_flow/address_joins.rs:149-257` (`referent_type`), `OU/structural_places.rs:239` and `:339`. Each accepts a different set of segments.
35. **Source contract of a place.** `UV:13928` (`structural_source_contract`, 115 call occurrences in 4 files), `UV:9478` (3-tuple `source_contract`), `UV:8375-8468` (re-derived for `ReturnStructural`), `UV:13984`, `UV:14008`.
36. **Arrival parameter.** `OU/graph.rs:278-310` and `AO/control_flow.rs:69-94` (different conditions; read both by `unit_027.md`).
37. **Unique declaration lookup.** `AO/structural_type_catalog.rs:244`, `AO/atomic.rs:124-128`, `AO/control_flow/address_joins.rs:154-160`, `AO/control_flow/owner_loans.rs:115-121`; seven unique-row filters in `DR` (`:1286, :1400, :1487, :2165, :2312, :2354, :2866`).
38. **Plain-call recognisers.** `UV:9841`, `UV:10514`, `UV:10700`, `UV:10906`, `UV:11582`, all through `UV:10838`; each repeats the same signature checks.
39. **Membership subset checks.** `UV:8754`, `UV:8822`, `UV:8896`, each building its own `DescriptorAnalysis`.
40. **Length producer of a byte field.** `UV:2866-2868` (exact path equality), `UV:2949-2956` (location equivalence), `UV:5327-5340`. Length-before-write: `UV:4852-4858` (a length node exists anywhere in the function) and `UV:6320-6333` (positional within the block).
41. **Width tests.** Unsigned 8-bit non-address: `UV:5120-5122`, `DR:2648-2650`, `UV:17317-17323`. Unsigned 64-bit: 11 times in `UV:11693-11743` plus `UV:11765`, `11854`, `11929`, `11934`, `11961`.
42. **Call argument scalar-type zip.** `UV:12262`, `12282`, `12298`, `12319`, `12333`; dynamic slot lookup at `UV:11950-11957` and `UV:12493-12500`.
43. **Field and claim lookups.** Field exists and is not erased: `UV:8545`, `UV:7432-7445`, `UV:2828-2857`. "Result with claims by place" lists with different members: `UV:8604-8618` and `UV:11277-11294`.
44. **Function parameter checks run twice.** `UV:16125` (called at `UV:16696`) and `UV:16705-16723`.
45. **Range-authority helpers.** `UV:2175` and inline `UV:1980-2004`; `UV:2141` and `UV:2158` differ only in the kernel function called. `SEM/lib.rs:103-124` and `:126-147` are line-for-line the same except the last call.
46. **Provenance of a node.** `UV:2630` (recomputed expectation, accepts extra trailing rows) and `UV:6413` (duplicates across nodes limited to terminal edge sites); "They check different sides" (`unit_026_gap_011.md`).

### `candidates.rs` families

47. **"This operand is a specific integer literal."** `CAND:8834`/`8889`, `CAND:8852`, `CAND:6433`, `CAND:5460`, `CAND:6730`.
48. **Scalar identity classifiers and validators.** `CAND:4931` and `CAND:6038`; validators `CAND:5538` and `CAND:5891`.
49. **Block-merge replay closures.** `CAND:596-631` and `CAND:757-803`, differing in which provenance is extended.
50. **Ownership-frontier identity.** `CAND:6847` (3 sites) and `CAND:7461` (6 sites); same empty-facts fallback (`CAND:6864-6866`, `CAND:7483-7485`).
51. **Effect counter walk.** `CAND:1017-1068` inline, `CAND:7590`, `CAND:10566-10577`; validator side `UV:6526-6535`.
52. **Inline obligation-fact checks.** `CAND:2837, 3189, 4139, 4185, 4226, 4515, 4615, 5985` (8 sites).
53. **Rule-name tables.** `CAND:404`, `CAND:8403`, `CAND:8907` (plus `4806`, `5523`, `6493`).

### `current_ownership.rs` families

54. **Two storage-overlap models.** `Region`/`Projection` with `regions_overlap_at` (`CO:1120, 1133, 1197, 1213`) and `ReferenceOrigin` with `origins_overlap` (`CO:5137`). "Suspected duplicate job" (`unit_025_gap_009.md`, read both).
55. **Move or copy live reference leaves.** `CO:4774` (`windows::transfer`), `CO:5932` (`establish_constructor`), `CO:5506` (`copy_reference_carriers`).
56. **Partial-move bookkeeping.** `CO:8321-8338`, `CO:8361-8383`, `CO:8386-8416`, also in `cleanup` and `block_parameters`; `UV:763-866` does its own and calls `CO` helpers at `UV:715` and `UV:872`.
57. **Claim-path expected versus actual.** `CO:8039-8086`, `CO:8107-8137`, `CO:8156-8237`.
58. **Last node of a block.** `CO:2485`, `CO:2715` (`checked_sub`, error on empty) and `CO:3488`, `CO:3580` (`saturating_sub`, ordinal 0 on empty).
59. **Descriptor catalog graph construction.** Eight wrappers at `DR:905-1087`; built at `CO:163`, `CO:2865`, `CO:4197`, `UV:1214-1226`, `UV:8784`, `UV:12842` (once per call when the cache misses), `UV:15567`. "Suspected: the same whole-unit graph is rebuilt in several phases" (`unit_026_gap_012.md`).

### Analyses, runtime and foundation

60. **Greedy view choice with footprint overlap and interference.** `SIA/spill_choice.rs:452`/`:482` and `SIA/fixed_precolored_segment_homes.rs:79`/`:255`/`:138`; live overlap test also `SIA/fixed_precolored_segment_homes.rs:130-136`.
61. **Small scans.** `block_instructions`: `SIA/rewriting.rs:41`, `SIA/liveness.rs:1215`, `SIA/recovery_classification.rs:319`. Block id to index: `SI/control_flow.rs:45`, `SIA/liveness.rs:151-201`, `SIA/liveness.rs:1640`. Register mention in a successor: `SIA/rewriting.rs:64`, `SIA/rewriting.rs:427`, `SIA/liveness.rs:1652`, with different coverage.
62. **"Plain two-operand `CopyI64`."** `SI/calls.rs:619`, `SI/calls.rs:740-762`, `SIA/rewriting.rs:261-320`.
63. **Root checks and carried counts.** `SIA/allocation_legality.rs:1045`, `SIA/fixed_precolored_segment_homes.rs:744`, `SIA/allocator_availability.rs:39-52`; carried counts at `SIA/lib.rs:855`, `SIA/allocation_legality.rs:1140-1143`, `SIA/live_ranges.rs:1565-1573`.
64. **Process-wide catalog caches.** `SI/effects/catalog.rs:1595` and `SIA/machine_effects.rs:63`.
65. **Value uses in the legalized graph.** `LO/control_flow/scalar_graph.rs:127-413` (`visit_scalar_operands`) and `:1376-1420` (`references_value`, which skips cleanup calls and case payloads).
66. **Linear find by machine id.** `MC/fragments.rs:52`, `MC/layout.rs:214`, `PAM/lib.rs:81`, `SI/functions.rs:283`, `TO/lib.rs:41`, `AO/lib.rs:62`; `OU/lib.rs:200` uses binary search.
67. **Byte cursor.** Six `struct Cursor` definitions: `I/native-provider-runtime/src/wire.rs:65`, `F/optimization-core/src/cursor.rs:17`, `SI/effects/program/encoding.rs:9`, `SIA/lib.rs:571`, `OU/lib.rs:366`, `psi/foundation/optimization/src/lib.rs:57` (only the first two compared; others suspected).
68. **Windows named pipe with peer-pid check.** `F/bounded-process/src/private_observation/windows.rs:77-134` and `I/native-provider-runtime/src/channel.rs:144-208` (read both).
69. **Stage then publish a file.** `F/platform-custody/src/file_publication.rs:27` and `F/platform-custody/src/record_file/stage.rs:53-100`; temp directory at `F/platform-custody/src/temporary_directory.rs` and a separate one at `I/native-provider-runtime/src/native_provider_runtime.rs:118-130`.
70. **C symbol locator match and owned-cell check.** `I/terminal-runtime/src/execution/host_imports.rs:192` and `.../absolute_value.rs:221-267`; the owned single-scalar cell check three times (`absolute_value.rs:152-181`, `allocation.rs:247-285`, `allocation.rs:320-358`).
71. **Optimization name list.** `F/optimization-core/src/optimization_core.rs:167-389` and a separate `PsiOptimization` enum in `psi/foundation/optimization/src/lib.rs`; the mapping is exhaustive at `optimization_core.rs:391-409`.

### Suspected only (other site not read by any record here)

- `constructor_arguments`: `CO:9422` and `terminal-psi/src/control_flow.rs:1585`.
- `leaf_paths`, `leaf_referent`, `reference_source_accessible`: `UV:14756, 14830, 15001` and pipeline 02 `references.rs:373, 475`, pipeline 03 `legalization_reference_custody.rs:4236, 4329, 4399`.
- Value-range reasoning: `CVR` and pipeline 01 `value_ranges.rs`.
- Residual computation: `CO:8848-9096` and `psi/.../validation/src/value_custody_core.rs`, `terminal-psi/semantics/src/structural.rs`, pipeline 06 `emission_cleanup.rs`.
- `validate_structural_fields` pairing rules and Terminal Psi `types_and_values.rs`.
- Placement-shape rules in `LO/validation.rs:273-354` and the `calling_conventions` crate.

---

## Fixed lists of shapes

### optimization-unit-semantics: rewrite validators (`candidates.rs`)

- 11 rule rows in `rule_rows!` (`CAND:404-416`).
- Adjacent merge: target is exactly the next block; first node of the target is one of three listed forms (`CAND:518-543`).
- Any block with `structural_parameters` makes merge, thread and fusion reject (`CAND:1994-2005`).
- Linear thread: exactly one empty-block node and one incoming predecessor (`CAND:1135`, `CAND:1170`). Shared fusion: target block has exactly one return-shaped node, at least 2 incoming edges (`CAND:1550-1569`).
- Dead scalar: three families, operation list `CAND:2531-2544`; all storage, call and atomic variants return `None` (`CAND:2545-2592`).
- Total scalar identity: 25 kinds across 4 classifiers (`CAND:5240-5280`). Compatible-policy CSE: shift and Add/Sub/Mul only (`CAND:3512-3514`). Total CSE: listed operations, `_ => return None` (`CAND:3940`).
- Integer-constant laws: 5 (`CAND:6539`); operand-substitution rules: 6 (`CAND:6493-6500`).
- Boolean evaluation: five operations (`CAND:8449-8457`). Range comparisons: six one-sided and three pair kinds (`CAND:8011-8043`, `8144-8148`).
- Case-membership, field-value and state-argument specialization reject any machine whose block graph has a cycle (`CAND:238`, `3039`, `10147`).
- Dispatch block: quote `CAND:9651`: `block.id == function.entry || !block.structural_parameters.is_empty()` rejects. Predecessor must be `Jump` or `Conditional` (`CAND:9882-9993`).
- Node-removal rewrites exclude the last node of a block because `retire_node_custody` needs a next node (`CAND:10649-10652`; checks at `4107`, `4473`, `7055`).
- Value ranges: two support kinds only, integer types only (`CVR:148-283`).

### optimization-unit-semantics: ownership replay (`current_ownership.rs`)

- Alias building handles five forms, `_ => None` (`CO:376-474`, `CO:419`).
- "Cannot retain alias" operation list (`CO:2384-2415`); anything else pins all dominated blocks.
- Edge transfer: three shapes (Unrestricted copy, projected move, whole move) (`CO:2472-2551`).
- Call write summaries exist only for functions with no `Call`, `AtomicEvent`, `PortWrite`, owned boundary arguments, nominal cleanup or recursion (`CO:2951-3026`).
- Cleanup action forms: `[RetireHome]`, `[DiscardRoot]`, whole-place `[InvokeNominal]`, or a partition plus `RetireHome` (`CO:3424-3436`).
- Nominal cleanup receiver: record whose live fields are Boolean or 8/16/32/64-bit integers, address integers 64-bit (`CO:3662-3688`); exactly one cleanup function per nominal type (`CO:3616-3622`).
- Reference leaves: at most 4,096 incoming (`CO:3800`, `CO:5263-5275`).
- `apply_operation` arms (`CO:6020-6461`); reference results only from `EstablishReference`, `EstablishAccessReference`, structural `CallStructural` (`CO:6683-6740`).
- Terminators handled: `Return`, `ReturnUnit`, `ReturnStructural`, `Crash`; `_ => {}` (`CO:8567-8692`).
- Partial-move residuals: `Record`, literal-length `FixedArray`, `Sum`/`Mixed`; quote `_ => return None` (`CO:9009`). Fully consumed: partial-affine roots, or a `Linear` parameter that is a literal-length `FixedArray` with every `[FixedIndex(i)]` moved (`CO:9396-9419`).
- `observations` closed list, `_ => Ok(())` (`CO:9635-9730`).

### optimization-unit-semantics: unit validation

- `activation_storage::operation` projects a fixed list; error string "activation replay has no origin-preserving projection for this structural operation" (`UV:352-356`); `_ => Ok(None)` (`UV:357`).
- Cleanup groups on an edge: `[..., RetireHome]`, `[DiscardRoot]`, `[InvokeNominal with empty path]` (`UV:669-681`).
- Range authority walks a fixed operation list, `_ => {}` (`UV:2117`).
- Operations that do not change metadata: closed list, `_ => true` (`UV:3507-3539`).
- Byte predicates: `copied_field_predicates_at_invocation` handles one `StructuralLeafCopy` shape (`UV:4064-4169`); `may_change_owned_source` closed list ending `_ => false` (`UV:4451-4481`).
- `fixed_bytes`: `FixedArray` of unsigned 8-bit non-address integers only (`UV:5117-5122`); `valid_representation_count`: `ElementView` or literal-extent `FixedArray`, `_ => false` (`UV:5179-5202`).
- `ByteSequenceWrite` must be preceded in the same block by `ByteSequenceLength` (`UV:6314-6334`).
- "Exact terminal" list for repeated provenance (`UV:6438-6443`).
- `validate_structural_root_operations`: one `match`, `_ => {}` (`UV:7324-8506`); three store destination kinds (`UV:7255-7296`); `ReleaseReference` paths `Field`/`FixedIndex` only (`UV:7951-7957`).
- Block structural parameters: one of nine accepted forms, decided by an ordered `else if` chain (`UV:9000-9008`, `UV:9077-9270`).
- Normal-completion disposition by last node: closed list, other last nodes are the error "unclassified current control node" (`UV:9773-9788`).
- Callee shapes: exactly one parameter, one block, one node (`UV:9849-9860`); five recognisers (`UV:9841`, `10514`, `10700`, `10906`, `11582`).
- Constructor qualification rows: paths starting `Field` or `Case, Field` only (`UV:11071-11095`).
- Numeric windows with no comment: `ActivateTaskRegister` entries in `2..=8192` (`UV:11871`), `ReloadSegmentDescriptors` in `1..=8192` (`UV:11886`).
- `reference_call_matches`: 17 named conditions, labels discarded (`UV:12749-12825`).
- Argument paths under `StructuralProjectionPolicy::Unit`: empty, static borrowed, all-`Field`, `[FixedIndex]`, `[FixedIndex, FixedIndex]`, or owned partial-affine (`UV:13423-13446`); `Projected` and `Boundary` accept any shape (`UV:13447-13448`).
- `is_material_write_only_type` shape list (`UV:13826-13870`).
- Reference-typed results only from `EstablishReference`, `EstablishAccessReference`, structural `CallStructural`; reference-containing results only from `EstablishRecord`, `EstablishStructuralArray`, structural `CallStructural` (`UV:15249-15277`).
- Trivial affine local type must be an empty `Record` (`UV:17048-17051`); byte literal a `BorrowedView` (`UV:17001-17006`).
- Compressed trivial-affine-local form only for a scalar-free linear-in, linear-out signature with affine unqualified extra parameters (`UV:18115-18134`).
- Field type by relevance: exhaustive match with no wildcard (`UV:17740-17805`).
- `DR`: `scalar_expressions::expression` denotes six operations (`DR:855-886`); `case_test` two (`DR:632-660`); `access_results::summarize` four origins, `_ => None` (`DR:184-186`).

### optimization-unit and abstract-operations

- 19 rewrite patch kinds (`OU/rewrite.rs:1198`, arms `:2609-2997`); 26 total and 21 proof-certified scalar identity kinds (`OU/rewrite.rs:711-773`).
- Field values: Boolean and Integer only, floats excluded (`OU/structural_places.rs:282-288`); `canonicalize_ordinary_path`: `Field` and in-range `FixedIndex` only (`OU/structural_places.rs:239-273`).
- Interval extraction: `Equal`, `LessOrEqual`, `LessThan` between a fixed-carrier value and a literal of the same type (`OU/range.rs:189-269`).
- Machine references: `Call`, `InvokeNoNormal`, `CallStructural` (`OU/lib.rs:229-239`).
- Lifecycle operands: exact argument counts 4, 3, 2, 2 (`AO/access_lifecycles.rs:35-86`).
- `contains_only_scalars` shape list (`AO/atomic.rs:112-171`).
- 15 (policy, operator) pairs (`AO/integer_arithmetic.rs`), enumerated again at `OU/identity.rs:4355-4375`.
- `operates_on_byte_sequence_storage`: 9 variants (`AO/operations.rs:1239`).

### Later representations

- Exit policies: 8 ABI and frame combinations (`MC/functions.rs:42-51`). Layout policies: 4 (`MC/layout.rs:62-67`), with the comment "Entry-first conditional-fallthrough chains; remaining chain roots follow selected block roster". Branch predicates: 3 (`MC/layout.rs:95`). Many single-variant policies (`MC/fragments.rs:135, 155`, `MC/layout.rs:371`, `MC/layout/text_section.rs:41`, `MC/storage/frames.rs:42, 197`).
- `PhysicalAddressOperation`: 16 variants (`PAM/instructions.rs:35`); choice rule single variant with the comment "Target catalogs partition physical-home configurations so exactly one alternative applies" (`PAM/instructions.rs:11`).
- Recovery: `RecoveryClassification` two outcomes, 7 no-recovery reasons (`RH/recovery.rs:129-159`); policy comment "Named bounded policy for classifying the already selected pressure victim" (`RH/recovery.rs:81`); spill policy comment "Structural policy for the first locally witnessed pressure point" (`RH/recovery.rs:699`).
- Post-allocation transformations: 17 (`RH/post_allocation_manifest.rs:61`).
- Allocation recovery rule catalog: exactly two entries; zero or one may be selected (`SIA/lib.rs:403-444`).
- Early clobber: one long boolean shape condition (`SIA/live_ranges.rs:305-338`). Copy affinities: `CopyI64` with exactly two operands (`SIA/live_ranges.rs:584-625`).
- Split requirements: tree topology only (at most one incoming connector per block, source earlier), no tied registers (`SIA/fixed_precolored_split_requirements.rs:325-332, 552-612`).
- Spill choice: single block, one fragment, no tied pairs, no early clobbers at the pressure point (`SIA/spill_choice.rs:109-271`).
- Rematerialization: non-address unsigned 64-bit, origin `InstructionResult`, defined by `MaterializeI64`, single fragment in the block (`SIA/recovery_classification.rs:134-230`).
- Liveness unit sets: `u64` mask when all unit ids are below 64, else `BTreeSet` (`SIA/liveness.rs:221-232`); legality bitmask path when a class has at most 64 units (`SIA/allocation_legality.rs:669`).
- Sign-extension widths 8, 16, 32 (`SIA/liveness.rs:1488-1493`).
- Atomic forms: 40 (`SI/atomic.rs:120-153`); retry-until-commit hard-codes AArch64 for four operations (`SI/atomic.rs:105-112`).
- Foreign call return transport: three exact result shapes followed by `CopyI64` copies only (`SI/calls.rs:314-498`); "private transport" three kinds (`SI/calls.rs:725`); dynamic call encoded size 36 bytes (AArch64) or 24 (x86-64) (`SI/calls.rs:1454-1462`).
- Slot summary: `Local` frame slots at offset 0 only (`SI/blocks.rs:514-596`).
- Machine-effect validation: operand rosters by fixed position (CopyBytes 3+2, ZeroBytes 2+1, ScanByteSequence 2+3, CompareByteSequences 4+4, `SI/effects/catalog/validation.rs:3045-3125`); the memory/stack/trap triple is a closed whitelist ending `_ => return Err(())` (`SI/effects/catalog/validation.rs:2952-3479`).
- Trapping forms: 72 (`LO/control_flow.rs:210-256`); saturating carriers: fixed 8/16/32/64 (`LO/control_flow.rs:78-93`); `InstallStaticInterrupt` exactly 6 arguments and 2 calls (`LO/control_flow/scalar_graph.rs:417-422`).
- Call placement: integer widths 1/2/4/8 or float 4/8, one register or one stack slot; aggregates of 1 to 16 bytes in 1 to 2 registers (`LO/validation.rs:273-329`).
- Descriptor input guard: exactly one structural parameter and `parameter_order == [Scalar, Structural, Scalar]` (`TO/descriptor_input.rs:35-52`); scalar types exactly `[i32, uN]` with N 32 for `WindowsCrt` else 64 (`TO/descriptor_input.rs:147-155`).
- IEEE float operations: 70 by hand-placed ordinal (`TO/boundary/execution.rs:179-304`). Atomic storage integer widths 8/16/32/64 (`TO/operations.rs:784`).
- Float-class layout literal: 8 bytes, tag at 0 width 4, cases 1..4 with a 1-byte field at offset 4 (`TO/boundary.rs:66-88`).

### Foundation and interpreter

- 55 optimizations, 9 phases (`F/optimization-core/src/optimization_core.rs:88, 167-389`); 21 analysis kinds (`.../contracts/analysis.rs`).
- Process preparation: at most 4,096 arguments, 1,024 environment entries, 4 MiB charged bytes (`F/bounded-process/src/preparation.rs:208`).
- Native parameter geometry: three accepted kinds, else "native parameter storage disagrees with its exact physical value transport" (`I/native-provider-runtime/src/native_provider_runtime.rs:438-482`). Worker packet tags 1 to 7 (`.../worker.rs:153`).
- Host import adapters: a fixed ordered list tried in sequence (the record names 14), else "no trusted host implementation matches boundary ..." (`I/terminal-runtime/src/execution/host_imports.rs:455-619`).
- `memset` accepts fill value 0 only (`I/terminal-runtime/src/execution/host_imports/allocation.rs:356`).

---

## Accepts when it does not know

Counted: distinct entries the readers classed, not mentions. 120 "open", 12 "stated", 37 "traced".

### Classed "open" (120)

Each line gives the site, what is accepted, and the wrong program the reader named, where one is named.

**optimization-unit-semantics, `candidates.rs` (8)**

1. `CAND:1581-1586` shared terminal fusion calls `checked_scalar_substitutions(.., |_, _| true)`; the dominance predicate used by the merge paths is always true here. No comment at the site.
2. `CAND:1813-1832` a function missing from the ordinal map is silently not mapped; the index at `CAND:1782` would panic.
3. `CAND:1834-1879` a machine referenced only through something the reference walkers do not expose would be pruned (the reader notes this fails toward removal, not acceptance). Not traced.
4. `CAND:4225-4249` compatible-policy CSE key erases policy (`CAND:3466-3516`), so a Saturating leader can replace an Exact redundant operation; soundness "rests on the Exact obligation proving no overflow (inferred, not traced)".
5. `CAND:7315` `O::StructuralCase { .. } => {}`: a `StructuralCase` carrying a use of the substituted value would not be rewritten.
6. `CAND:9824-9851` `call_result_constant` uses the callee's unique `Return`; no check for recursion or other effects; producer and validator share the rule, "so a wrong rule is not caught by the replay (inferred)".
7. `CAND:2274-2277` versus `CAND:2113` and `CAND:7293`: `WriteOnlyPrimitiveStore` with a range obligation is skipped by one rewriter and not the others; which arm covers it was not read.
8. `CAND:10686-10692` unit-level tables (`pruned_machines`, `accepted_obligation_facts`) are protected only by revalidation and the identity recompute, except where individual validators compare them.

**optimization-unit-semantics, `current_ownership.rs` (22)**

9. `CO:204` if the descriptor catalog graph fails to build, sources and effects stay empty and `validate_compatibility` returns `Ok` on empty requirements (`CO:716`). Wrong program: "a function for which the descriptor catalog graph fails to build would produce no loans from stored call effects, so those conflicts would not be reported".
10. `CO:553-562` `access_location` falls back to the longest known prefix path, including the empty path.
11. `CO:1069-1073` for return operations the moved list is emptied. Wrong program: "a return that moves a place still backing a live loan".
12. `CO:1160`, `CO:1276` `zip` of bindings with destination parameters truncates; extra bindings are never checked as disturbed.
13. `CO:1620-1623` an edge whose target is missing or empty is dropped from the ordering graph; the analysis could "believe fewer paths exist".
14. `CO:2207-2211` a binding into an exclusive-borrow parameter whose source has no contract or is not `Owned` skips the owner-liveness checks.
15. `CO:2202` same `zip` truncation in `check_edge`.
16. `CO:2467`, `2554`, `2589` same `zip` truncation in edge transfer; the final count check covers only claims.
17. `CO:3156-3173` a `Linear` owned place at return with no claims or partial paths is removed from the owed set when its empty-claim frontier validates with `&|_, _| true`. Wrong program: "a function that returns while holding a Linear place whose type owes nothing under an empty claim frontier".
18. `CO:3490`, `CO:3582` `saturating_sub(1)` gives ordinal 0 for an empty block instead of an error.
19. `CO:6461` `_ => {}` in `apply_operation`: unlisted operations get no access check, no argument overlap logic and no stored-effect staging. Wrong program: "a read through a suspended referent by one of the unlisted structural operations". The reader notes descriptor-loan checks list the same operations at `CO:1368-1406`.
20. `CO:5824-5826` `discard_subtree` does nothing when the place has no structural contract; reachable from `CO:2518` with no prior contract check.
21. `CO:4302-4308` `remaining_at_return` with no effect summary returns all live references; the caller was not read.
22. `CO:6008-6019` the "operation result remains pinned" check is skipped when `psi_operation()` is `None`.
23. `CO:6636-6669` two ordinary (non-reference-bearing) arguments to one call are not checked against each other here.
24. `CO:7458-7460` `validate_atomic_access` returns `Ok` when `event.location()` is `None`.
25. `CO:9530-9542` `moving` returns `Ok` for any non-`Owned` root or a path that leaves inline storage. Wrong program: "moving out of a borrowed root or through an indirection is not checked here".
26. `CO:9578-9580` `observing` the same for a non-`Owned` root.
27. `CO:9609-9622` `observing` returns at the first prefix that leaves inline storage; a claim frontier below an indirection is not checked by this function.
28. `CO:9730` `observations` ends `_ => Ok(())`; `StructuralLeafCopy` with a non-`Ordinary` path falls there.
29. `CO:9799-9816` `install_store` removes the stored value's claims and reinserts them only when the destination is owned. Wrong program: "a stored value's claims vanish when the destination is a borrowed parameter".
30. `CO:9943-9947` a content-entry claim whose root is not a structural parameter is recorded with `input: None`; checks that filter by `claim.input == Some(place)` will not see it.

**optimization-unit-semantics, `descriptor_results.rs` (4)**

31. `DR:2340-2341` a caller-supplied `preserves_boundary` closure returning `true` makes a boundary call non-escaping; non-default users at pipeline 03 `legalization_descriptor_inputs.rs:64` and pipeline 02 `references.rs:1536` not traced.
32. `DR:2242-2272` a call to a machine outside the catalog is non-escaping when the result is Unit or Scalar, there are no claim transfers and every non-shared argument has closed storage.
33. `DR:187-191` `Empty` origins are dropped from a slot's source list; `[Empty, Captured(x)]` resolves to `x`.
34. `DR:1123` any `None` from `adapt` makes the whole graph `None`; callers that treat `None` as "no descriptor info" were not read.

**optimization-unit-semantics, `unit_validation.rs` (40)**

35. `UV:357` activation replay drops every unlisted operation; when the function has no activation-claim operations, unlisted structural-result operations are dropped too.
36. `UV:883` the retained-ownership check returns `Ok` when `ownership_frontier_facts` is empty; whether another phase requires facts is open.
37. `UV:2057-2063`, `UV:2089-2099` `EstablishCase` on a non-`Sum` type and `EstablishRecord` on a non-`Record` type `continue` past the range-authority check.
38. `UV:2067`, `UV:2100` `zip` of declared fields with initializers truncates.
39. `UV:2235-2237` `validate_range_fact` returns `Ok` when the kernel returns no requirement.
40. `UV:2637`, `UV:2641` provenance: anything is accepted for jump, conditional and case nodes; extra trailing provenance is accepted for the rest.
41. `UV:3773-3817` a `ByteSequenceWrite` is accepted with no predicate check when `byte_write_predicates` returns `None`.
42. `UV:3750-3752`, `UV:3771` a fixed byte store is accepted with no ASCII freshness when `requires_ascii` is `None` or `Some(false)`; "a destination whose byte predicates cannot be computed is accepted here".
43. `UV:4479` `may_change_owned_source` ends `_ => false`; `ElementViewWrite` is not in its list.
44. `UV:5455-5457` ASCII establishment trusts a declared qualification on any structural result at the root.
45. `UV:5793` the cycle error is skipped for admitted machines; which machines are admitted is decided by callers of `UV:18229` (pipeline 00 `artifact_admission.rs:1126`, pipeline 01 `pass_manager.rs:1313`), not read.
46. `UV:6724-6727` a place with no recorded producer is skipped in the availability check. Wrong program: "an operation reading a place that nothing produces".
47. `UV:8506` `_ => {}` in `validate_structural_root_operations`.
48. `UV:7517-7550` `Field` operands of `ByteSequenceEqual` and `ByteSequencePredicate` are not examined. Wrong program: "a `Field` operand whose field/length does not exist or is erased".
49. `UV:8246` `continue` after `TransformContent` skips the generic added-membership requirement check.
50. `UV:8770-8772` `reference_result_memberships_match` returns `true` when the callee is not a function in the unit (includes dynamic callees). Wrong program: "a call to an external or dynamic callee whose result carries projected qualifications is accepted by this check".
51. `UV:8868-8870` `view_memberships_match` returns `true` on `Empty` payload evidence even when the result claims projected qualifications.
52. `UV:9999-10029` atomic coherence edge mapping returns `None` to the kernel, which decides; not read.
53. `UV:10208-10210` a dynamic slot's claim path that contains a `Case` segment does not have to be transferred. No comment.
54. `UV:10220` `zip` of arguments with callee parameters truncates.
55. `UV:10749` `payloadless_selected_evidence_surface_matches` returns `true` for anything that is not a structural `CallStructural`.
56. `UV:10803` `exact_claim_free` true returns early past the claim-shape check.
57. `UV:11544-11558` an element value with unknown origin gives `None` to the kernel's qualification check.
58. `UV:11856, 11896, 11927, 12055-12072, 12085, 12106, 12225` about twenty operations return `true` from `operation_scalar_types_match` with no operand type check (for example `PortWrite`, `PrimitiveScalarRead`, `EstablishScalarArray`).
59. `UV:12086-12088` `FloatResultPolicy` delegates to `accepts_operand_types`; not read.
60. `UV:12437-12442` `.all(..)` over `call_machine_references` is true on an empty iterator; then no published-ceiling check runs.
61. `UV:12452` service check ends `_ => true`; only five operations are examined.
62. `UV:13639-13642` a `MutableBorrow` source may supply an `Owned` argument when the path is non-empty; for an `Affine` formal this is accepted without `is_partial_affine_path` (`UV:13722-13727`). Wrong program: "moving an `Affine` field out of a mutable borrow".
63. `UV:13447-13448` `Projected | Boundary => true` for path shape.
64. `UV:13951-13955` `structural_source_contract` takes the first producing node with `.find`; a producer-uniqueness check was not traced.
65. `UV:15482-15484` a missing dispatch-table row adds no dependency edge.
66. `UV:15623-15625` dynamic `CallStructural` operations are not given to the runtime-evidence validator.
67. `UV:15709-15715` values with no scalar expression are omitted from the evidence inputs.
68. `UV:16024-16046` `refresh_root_service_reach` overwrites the declared reach with the derived one. `unit_026_gap_013.md` read the order: refresh at `UV:18223`, validation at `UV:18225`, so for rewrite output the comparison at `UV:15920` is of the derived value with itself.
69. `UV:16084-16144` boundary parameters' `projected_qualifications` are not checked in these two functions.
70. `UV:16763-16764` `ByteSequenceLiteral` and `TrivialAffineLocal` places are accepted as known types here; a literal or local place with no producing operation is not checked in this function.
71. `UV:16919-16920` a missing identity maps to `""`, which would equal a claim segment `""`.
72. `UV:17919-17921` an attachment naming an unknown structural type is kept in the storage set with no error.
73. `UV:18044` the explicit trivial-affine-local form returns `Ok` without examining the returns' cleanup actions or parameter shape. Wrong program: "a function with explicit trivial locals but a wrong `ReturnStructural.cleanup_actions` passes this function".
74. `UV:18076-18078` a `ReturnStructural` with no hidden locals is skipped for the discard check.

**optimization-unit (9)**

75. `OU/construction.rs:608, 741, 910` wildcard arms: "a new operation variant that reads a value or defines one would be recorded with no use/definition". (I checked the validator side: see check 7.)
76. `OU/construction.rs:406-490` `operation_obligation` ends `_ => None`: an unlisted obligation-carrying operation gets no fact.
77. `OU/dominance.rs:66-72` unreachable blocks keep large dominator sets; a consumer can get `true` for many candidates.
78. `OU/identity.rs:1272-1364` seven `BoundaryMachineDeclaration` fields (`declaration`, `numeric_intrinsic`, `crash_routes`, `scalar_domain_requirements`, `scalar_requires`, `scalar_ensures`, `result_reference_regions`) are not hashed; `unit.identities` is not hashed (`OU/identity.rs:2361-2458`). Whether they are derived from hashed fields: not traced.
79. `OU/manifest.rs:116-203` decode does not cross-check `final_unit`/`initial_unit` against the ledger.
80. `OU/observation.rs:313-317` a missing frontier fact gives `identity: None`, not failure.
81. `OU/observation.rs:230-235` live-out detection ignores `structural_bindings` and ownership events.
82. `OU/rewrite.rs:2618-2629` the constant-replacement arm checks provenance only, not the witness kind.
83. `OU/structural_places.rs:1025-1034` a zeroed value is taken to be in the first declared case without verifying that zero inhabits it.

**abstract-operations (2)**

84. `AO/operations.rs:946`, `AO/operations.rs:1075` new variants default to "no structural result" and "no paths".
85. `AO/storage_mutations.rs:84` `ReleaseReference`, `PortWrite`, `RegisterAccess`, `GlobalDescriptorTable`, `ActivateTaskRegister` are treated as mutating nothing.

**machine-code (4)**

86. `MC/encoding.rs:152-161` `bytes()` returns `&[]` for an out-of-range span; an `Encoded` row with a corrupt span hashes as zero bytes.
87. `MC/lib.rs:288, 298` `PlacedImageAddress` accessors return `None` for other reference kinds; callers not traced.
88. `MC/layout.rs:326-332` the layout identity does not hash `normalized_foreign_call_fixup`. Wrong artifact: a value whose fixup differs but whose other hashed fields match has the same identity.
89. `MC/layout/text_section.rs:282-308` the text-section identity does not hash `callback_materializations` (same remark).

**register-homes (9)**

90. `RH/constraints.rs:48, 112` sortedness stated in field comments is not checked at decode.
91. `RH/post_allocation_manifest.rs:171-265` statistics accepted unchecked at decode.
92. `RH/recovery/fixed_view_copy_codec.rs:688, 696` `structural: None` stays if `decode_contracts` does not fill it (not traced).
93. `RH/recovery/selected_blocks.rs:436` `decode_case` accepts any `case_tag` and cleanup actions with no cross-check.
94. `RH/recovery/selected_instructions.rs:243-249` `ReturnAggregate` fragment count and `Store` byte size are not checked.
95. `RH/recovery/structural_contracts.rs:184` memory-access rows accepted with any place, offset and count.
96. `RH/recovery/structural_calls.rs:1003-1009` Import and Table bindings are not re-checked with `has_valid_entry_mechanism()`; only System and Component are.
97. `RH/recovery/structural_calls.rs:852` the registrar application commitment is read as raw bytes and not recomputed.
98. `RH/recovery/structural_calls.rs:1476`, `RH/recovery/structural_declarations.rs:518`, `RH/recovery/structural_contracts.rs:404-405` three fields are dropped on the wire; whether the plan identity covers them was open in the record (see check 20).

**selected-instruction-analyses (5)**

99. `SIA/fixed_precolored_split_requirements.rs:455` the result of `use_transition` is ignored. Wrong program: "a fragment where the shared view set becomes empty at a fixed-use point and `entry_transitions` has no (or two) matching entry for that site; the cut is still recorded".
100. `SIA/live_ranges.rs:740-742, 773-775, 816-818, 837-839, 847-849` an operand or liveness entry naming an undeclared virtual register produces no fragment and no error.
101. `SIA/live_ranges.rs:1145-1149` duplicate register ids yield rows with empty fragments and no error.
102. `SIA/machine_effects.rs:953` `ValidatedPreAllocationMachineEffects::new` is `pub` and takes any plan and receipt (grep: 1 call site, `SIA/machine_effects.rs:881`).
103. `SIA/spill_choice.rs:329` `choice: None` is returned when the simplified model finds a view for every register; whether it can under-report pressure was not traced.

**selected-instructions (6)**

104. `SI/blocks.rs:346-363` the terminator loop never sets `fixed_definition_bits`, `materialize_definition_bits` or instruction spans, so `has_fixed_definition` (`SI/blocks.rs:717`) ignores terminator definitions.
105. `SI/blocks.rs` `block_references_register` and `block_unit_*` return false/`None` for bad indexes; not traced.
106. `SI/effects/catalog.rs:1599-1613` a cached canonical catalog is returned for the same (target, constraint identity) even if the builder differs (suspected, not checked).
107. `SI/effects/catalog/identity.rs:282` and `:440` memory-effect tag 19 is written for both `ReadTableFieldV1` and `CatalogFenceV1`; a concrete collision was not constructed.
108. `SI/effects/program/encoding.rs:1789-1813` `decode_target` builds a target from any pointer size and architecture/format pair.
109. `SI/effects/catalog/validation.rs:2682-2685` for `OptimizationSelected` alternatives, clobbers need only be a superset.

**legalized-operations (5)**

110. `LO/identity/calling.rs:221-227` the runtime-index rows of a structural argument of an ordinary call are not encoded.
111. `LO/identity/structural_types.rs:123-172` `TargetStructuralArgument.indices` is not written. Together with the previous entry these rows "do not reach `legalized_operation_plan_identity`". No test mutates them.
112. `LO/identity/structural_types.rs:285-394` `quotient_carrier` is never written.
113. `LO/identity/calling.rs:393-397` five boundary realization payloads are encoded as a bare tag.
114. `LO/validation.rs:226` a `TargetFamily` origin is accepted for any non-empty `target` string.

**target-operations (2)**

115. `TO/descriptor_input.rs:35-48, 73-87, 110, 136-145` `Ok(None)` means no guard. Wrong program: "a descriptor-reading call whose declaration/argument shape is not the exact 3-parameter form returns no guard, so no byte limit is attached".
116. `TO/values.rs:185-189` `projected_family` leaves origins with a non-`Projected` extent unextended.

**optimization-core (1)**

117. `F/optimization-core/src/optimization_core/decisions/baseline.rs:162-178` decode accepts a self-consistent log whose Choose is outside its roster or whose roster is not in canonical order. Traced within the file; whether pipeline 01 re-checks is open.

**bounded-process (1)**

118. `F/bounded-process/src/process_child.rs:251` on non-Unix, `ErrorKind::InvalidInput` from termination is treated as "container already closed".

**terminal-runtime (2)**

119. `I/terminal-runtime/src/execution.rs:103-115` `comparison_unavailable` marks a run unavailable instead of failed for `MissingProvision` and four interpreter error kinds; how the CLI treats it was not read.
120. `I/terminal-runtime/src/execution/host_imports/allocation.rs:360` the `memset` handler's liveness test does not exclude retired allocations, so a freed address passes the handler check (partly traced: later resolution errors at `allocation/backing.rs:54`).

### Classed "stated" (12)

1. `MC/fragments/publication.rs:5-6` and `MC/layout/text_section_publication.rs:7-9`: manifest statistics and stage labels are accepted unchecked. Quote: "stage labels and summary statistics are retained claims that must be verified against admitted fragments". Tests confirm (`publication.rs:355-370`, `text_section_publication.rs:431-439`).
2. `PAM/codec.rs:249-391`: operand, slot and alternative references are not cross-checked. Stated by the test name `reauthenticated_physical_data_is_still_only_a_proposal` (`PAM/codec/tests.rs:846-855`) and `PAM/lib.rs:24-26`.
3. `RH/lib.rs:410-455`: any assignments decode if the identity matches. Quote `RH/lib.rs:31-37`: "A register-home plan or recovery artifact on the wire is a candidate record, not proof that interference, fixed-view constraints, or live-range preservation succeeded."
4. `SI/functions.rs:201-231` and `SI/plan_identity.rs:232-235, 272-285`: `prefix_verified` reuse trusts the caller; only `debug_assert!(len >= old_len)` (`SI/functions.rs:229`). Stated by the function name.
5. `SI/effects/catalog/validation.rs:3473-3478`: a declaration for any semantic not named earlier, with no memory, unchanged stack and either trap value, is accepted (for example `ExactAddI64` with `MayArchitecturalFaultV1`). Stated by the test at `:1507-1517`, not by a comment.
6. `SI/effects/catalog/validation.rs:2692-2699`: for `ReturnIndirectRegisterV1`, encoded implicit uses may be a non-empty subset. Stated by the test at `:1541-1557`.
7. `LO/validation.rs:274-276`: a zero-size aggregate result is accepted with no registers. Stated at `LO/validation.rs:13-15`.
8. `LO/tests.rs:886, 1006`: shape validation accepts a one-byte result for Boolean or u8; quote "fixed integer result geometry is valid; source replay checks supported scalar semantics".
9. `F/platform-custody/src/lib.rs:134-136, 149-151`: `ENOENT` from the ACL query maps to `Ok(false)`; explained in the header at `:11-15`.
10. `I/native-provider-runtime/src/worker.rs:182`: the install digest arrives in the same packet as the bytes (integrity only); authority is with the parent (`native_code.rs:6-8`).
11. `MC/storage/frames.rs:9-10`: frame records are "non-authoritative representation data whose invariants ... are independently replayed by the backend".
12. `RH/recovery.rs:8-13`: "None of these artifacts grant allocation, spill, rematerialization, stack-slot, or publication authority on decode alone."

### Classed "traced" (37)

Readers followed these and reported no wrong program, or reported the direction as cautious.

- `CAND:4590-4596` leader availability in phi translation (no wrong program seen).
- `CAND:6863-6868`, `CAND:7482-7486` empty ownership facts accepted when the function has no structural parameters, entry claims or declared places (inferred that there is no frontier to disturb).
- `CAND:8200-8205` same-value range comparison (sound for SSA values).
- `CAND:9708-9750` skipped incoming edges give fewer rewrites.
- `CVR:63-82` binary search on `dominated_blocks` (sorted by construction and equality-checked first).
- `CO:1476` missing descriptor flow becomes a conservative flow.
- `CO:1448-1456` default arm still records whole-place reads.
- `CO:2154-2161`, `CO:2192-2199` requirements pushed into a discarded vector are re-exposed through `descriptor_requirements()` (the consumer `UV:1250` was read by a different record).
- `CO:2171-2173`, `CO:2270-2272` no pin for the block means `Ok`.
- `CO:2554-2569` unmatched reference stays carried at the old place.
- `CO:2964-2966`, `CO:3062-3063`, `CO:3081` call summaries fall back to the conservative write model.
- `CO:3748-3750` Unrestricted results are not tracked.
- `CO:4447-4466`, `CO:4469-4471`, `CO:5138-5142`, `CO:5245-5250` reference effects: unknown becomes an error or an overlap.
- `CO:3388-3404` uninstalled home (first part traced; `projected_root_is_fully_consumed` not read at that time, later read at `CO:9364`).
- `CO:7116-7121` `transfer_return` returning `Ok(false)` is not an accept path.
- `CO:9126-9129`, `CO:9191-9194` unknown type or missing contract gives the cautious answer.
- `CO:9358-9361` `None` residuals mean "not drained".
- `DR:2230-2241` exact-argument calls rely on the callee's own graph.
- `UV:6045-6047`, `UV:6265-6267` mutable-view skips unreachable with a bad edge.
- `UV:16462` direct index cannot panic.
- `OU/range.rs:232-243` unrecognised conjuncts only weaken the interval.
- `OU/rewrite.rs:2623-2653` `location.unwrap()` unreachable for non-prune patches.
- `OU/structural_places.rs:1503-1505` unknown block means not dominating.
- `AO/atomic.rs:386` ordering without custody is by design for `Store`.
- `SIA/allocation_legality.rs:934-938` a unit with no row is never live.
- `SIA/allocation_legality.rs:1140-1143` carried counts trusted; only `analyze_*` call `admit`.
- `SIA/fixed_precolored_segment_homes.rs:303-308`, `:1213-1234` skipped neighbours and dropped affinities affect preference only.
- `SIA/lib.rs:1007-1014` ignoring the `Err` of the full liveness upgrade cannot hide a failure the cheap form did not hit (first written "open", later traced in the same record).
- `SIA/liveness.rs:281-285, 306-332, 849` cache misses do not fire for valid block indexes and the constraint flag covers `UseDef` (traced through `SI/blocks.rs:281-286, 882-885`).
- `SIA/liveness.rs:1693` functions with no edges skip transport validation.
- `SIA/live_ranges.rs:1451-1458` `zip` with liveness cannot truncate unless the liveness plan was forged.
- `SI/effects/catalog.rs:33-161` four keys outside `in_identity_order` still enter the identity through declarations.
- `SI/effects/catalog/validation.rs:2328, 2388` `_ => NoneV1` / `_ => NeverV1` defaults for new semantic kinds (traced by reading).
- `SI/effects/program/encoding.rs:213-216` self-consistent forged plans decode `Ok` (traced in tests).
- `TO/boundary/realizations.rs:209-211` report identity 0 is refused by its two callers in the unit.
- `I/terminal-runtime/src/execution.rs:170-198` omitted provisions skip a comparison; later installation checks still run.
- `I/native-provider-runtime/src/host_filesystem.rs:243` unknown guest descriptors map to fd -1.

---

## Silent drops

Counted: distinct sites where something is left out with no row and no diagnostic. 60 in all. "Accept" consequences are listed above; this list is about the missing row itself.

**optimization-unit-semantics (17).** `CO:204` (no loans when the graph is missing); `CO:430-438` (four origin kinds dropped from alias lists); `CO:1160/1276`, `CO:2202`, `CO:2467/2554/2589` (three `zip` truncations); `CO:1620-1623` (ordering edge dropped); `CO:5824-5826` (discard does nothing); `CO:9799-9816` (claims dropped on store to a non-owned destination); `UV:357` (operations dropped from activation replay); `UV:2067/2100` and `UV:10220` (`zip`); `UV:6724-6727` (place with no producer); `UV:15623-15625` and `UV:15674` (calls dropped from the evidence collector); `UV:15709-15715` (values without expressions); `UV:15482-15484` (dispatch row); `UV:17919-17921` (unknown type not expanded); `DR:187-191` (`Empty` origins).

**optimization-unit (9).** Six wildcard extractors in `OU/construction.rs` (`:488`, `:608`, `:741`, `:910`, `:994`, `:1048`); `OU/observation.rs:313-317`; `OU/observation.rs:230-235`; `OU/range.rs:232-243` (traced as weakening only).

**abstract-operations (3).** `AO/operations.rs:946`, `AO/operations.rs:1075`, `AO/storage_mutations.rs:84`.

**selected-instruction-analyses (6).** `SIA/fixed_precolored_segment_homes.rs:1213-1234`; `SIA/live_ranges.rs:740-849` (skipped ids); `SIA/live_ranges.rs:1145-1158`; `SIA/fixed_precolored_split_requirements.rs:455`; `SIA/lib.rs:1007-1014`; `SIA/spill_choice.rs:315` (later registers not examined after the first pressure point, by policy).

**selected-instructions (2).** `SI/blocks.rs:346-363` (terminator definitions not recorded); `SI/blocks.rs:255-268` (copy affinities from body instructions only).

**register-homes (4).** Wire field drops: `RH/recovery/structural_calls.rs:1476` (`qualification_establishments`), `RH/recovery/structural_declarations.rs:518` (`projected_qualifications` of target parameters), `RH/recovery/structural_contracts.rs:404-405` (`realization`); `RH/recovery/structural_declarations.rs:406` (field storage always `None` for record kinds 2/13 and Mixed).

**machine-code (5).** `MC/encoding.rs:151-162`; `MC/lib.rs:288, 298`; `MC/boundary.rs:51-72` (sources without receipts ignored); two identity omissions (`MC/layout.rs:326-332`, `MC/layout/text_section.rs:282-308`).

**legalized-operations (7).** Identity omissions: `LO/identity/calling.rs:221-227`, `LO/identity/structural_types.rs:123-172`, `:285-394`, `:464` (`BorrowedView` payload), `LO/identity/calling.rs:393-397`, `LO/identity/structural.rs:354-369` (my check 20); `LO/control_flow/scalar_graph.rs:1390-1419` (`references_value` skips cleanup calls and case payloads).

**target-operations (5).** Four `Ok(None)` groups in `TO/descriptor_input.rs` (`:35-48`, `:73-87`, `:110`, `:136-145`); `TO/values.rs:185-189`.

**bounded-process (2).** `F/bounded-process/src/process_child/limits.rs:22-29` with `process_child.rs:74` (`let _ = limits;`): on Unix `active_processes`, `process_memory_bytes` and `aggregate_memory_bytes` are not applied; `F/bounded-process/src/process_child/descriptors.rs:85-102` (non-Linux Unix: descriptors opened after enumeration are not marked).

No silent drops recorded for post-allocation-machine, optimization-core, platform-custody, function-identity, artifacts, native-provider-runtime, terminal-runtime.

---

## Unused or unwired

Counts are the readers' greps over `omega-rust`, with the pattern they gave.

**optimization-unit-semantics**
- `abstract_catalog_graph` (`DR:905`): `grep -w` gives 1 occurrence, the definition. Zero call sites.
- `optimized_borrowed_storage_catalog_graph` (`DR:950`): `pub`, 1 call site, inside the crate (`UV:8784`).
- `ConflictingInterpretations` (`DR:217`): used only in `DR`.
- `parameter_establishment_order` (`CO:2787`, re-exported `CO:9836`): 4 occurrences, all in `CO` (definition, import `3114`, call `3223`, re-export). No user outside the file.
- `has_descriptor_call_origins` (`CO:2923`): one caller, `UV:12837`.
- `validate_total_cfg`: 2 occurrences; `block_reaches`: 6, all in `UV`.
- Five plain-call recognisers (`UV:9841, 10514, 10700, 10906, 11582`): 3 occurrences each, all in `UV`; no outside crate uses them.
- `is_reference_projection` (`UV:14975`) and `storage_projection` (`UV:14984`): no use inside `UV`; used from `CO` (`CO:6601`, import `CO:3790`).
- Dead statements: `for settlement in &node.work { let _ = settlement; }` (`UV:6475-6477`); `let _ = argument;` (`UV:10307`).
- `validate_provider_attachment_specialization` (`UV:17357`): both map parameters unused (leading underscore); 1 call site (`UV:9587`). It rejects every function that has a `ProviderAttachment` place, while `UV:16765-16767` accepts such places.
- `refresh_root_service_reach`: 1 call site (`UV:18223`). `validate_psi_structural_catalog` (`UV:18240`): 3 call sites, all test support.
- `call_origins` (`DR:1144`): no match for `descriptor_results::call_origins` outside the definition; other import spellings not grepped.
- `preserve_edge_custody`: 1 call site (`CAND:10525`); `set_node_effect_link`: 1 (`CAND:10576`); `reconstruct_conditional_fold_accounting`: 1 (`CAND:901`); `independently_post_dominates`: 1 (`CAND:1578`).
- `AcceptedObligationFactIndexMismatch` is reused for about 20 non-index failures in `UV:1543-2215`; `catalog_mismatch(function)` for 30+ structural failures from `UV:7315`; `InvalidStructuralTypeIdentity` / `InvalidStructuralDomainIdentity` for non-identity failures (9 occurrences in `UV`).

**optimization-unit**
- Ranking-certificate types (`OU/graph.rs:340-380`): no constructor in the file; users not grepped.
- `LoopInvariantScalarMotionCandidate::consumed_facts` returns `Vec::new()` (`OU/rewrite.rs:1024-1026`) and `provenance()` returns `&[]` (`OU/rewrite.rs:1354-1358`).
- `scalar_refinement_identity_bytes`, `access_view_catalog_identity_bytes`: 4 hits in 3 files; each has an outside caller.

**abstract-operations**
- `structural_leaf_copy_pair_pattern!` (`AO/operations.rs:1343-1348`) names `AbstractOperation::StructuralCaseLeafCopy`, which is not a variant of the enum; `grep abstract_operations::structural_leaf_copy_pair_pattern` finds 0 uses.

**machine-code**
- `ScalarControlFlowEvidence` (`MC/control_flow.rs`): 5 hits in 3 files, all in machine-code; `ScalarControlFlowEvidence::<variant>`: 0 matches anywhere. The four branch-evidence type names: 13 hits, all in machine-code.
- `exit_policy_tag` (`MC/functions.rs:330`): 1 call site (`:225`).
- `ResolvedBranchEvidence::as_conditional` (`MC/layout.rs:76`): the record reports 1 occurrence and zero call sites. That does not match the code; see check 9.
- `callback_materializations` is never read inside machine-code (1 occurrence, `MC/layout/text_section.rs:208`); 110 occurrences in 45 files elsewhere.

**physical-instructions**
- Free function `target_cost_model` (`PAM/costs.rs:16`): no call outside `costs.rs`; the 7 other matches are `.target_cost_model()` methods on other objects. Whether the cost-model types have a real consumer is open.
- `QualifiedPhysicalRead` (`PAM/operands.rs:13`): one real use (`MC/encoding.rs:229`).

**register-homes**
- `retain_projected_qualifications = false`: every production caller passes `true` (`RH/recovery/structural_calls.rs:221, 321, 700, 767`); the `false` branches at `RH/recovery/structural_declarations.rs:808, 822` are unreachable from this crate.
- `PostAllocationOptimizationManifestError` (`RH/post_allocation_manifest.rs:448`): not used by the decoder; 23 occurrences, 17 in pipeline 05 `assignment.rs`.
- `decode_locator` (`RH/recovery/structural_calls.rs:1047`): test-only.
- `RecoveryClassification` scalar type: the encoder writes tag 4 for floats (`RH/recovery.rs:650`), the decoder accepts 0..3 (`RH/recovery.rs:391`); no test encodes a float through it.

**selected-instruction-analyses**
- Error variants never constructed by qualified path (grep 0): `LiveRangeError` 6, `LivenessError` 7, `AllocationLegalityError` 3, `MachineEffectError` 4, `SpillChoiceError` 3, `RecoveryClassificationError` 2, `FixedPrecoloredIntervalError` 2, `FixedPrecoloredSplitRequirementError` 2, `AllocatorAvailabilityError` 2, `FixedPrecoloredSegmentHomeError` 2 (33 variants; a `use Enum::*` construction was not searched).
- `CutIndex::finish` (`SIA/fixed_precolored_split_requirements.rs:169-178`) cannot fail by reading; its error is unreachable.
- `seal_pre_allocation_machine_effects` (`SIA/machine_effects.rs:877`) always returns `Ok`.
- `compute_function_units_filtered`, `block_unit_writer_indices`, `has_materialize_definition`: 1 caller each, in pipeline 05 `runtime_spill_and_split.rs`.

**selected-instructions**
- `selected_function_digest` (`SI/plan_identity.rs:324`), `selected_block_digest` (`SI/plan_identity.rs:500`), `SelectedFunctions::identity_digests` (`SI/functions.rs:293`): `#[allow(dead_code)]`, no caller besides the definitions.
- Version flags `allow_i64_less_than`, `allow_scalar_call`, `allow_jump`: every caller passes `true, true, true` (`SI/effects/program/encoding.rs:190, 773`).
- `CatalogFenceV1` decode arm (`SI/effects/program/encoding.rs:1037`) is unreachable because `19 if allow_scalar_call` at `:1027` comes first and the flag is always true.
- `encoded_effects` takes `architecture` and discards it (`SI/calls.rs:1485`). The wire module is named `v6` while `VERSION` is 49 (`SI/effects/program/encoding.rs:99, 1894`).

**foundation and interpreter**
- `has_single_hard_link` (`F/platform-custody/src/lib.rs:302`): 1 grep hit, its definition.
- `OptimizationWorkUsage::within(budget)` (`F/optimization-core/.../manifest/work_usage.rs:18`): the pass record stores usage but holds no budget, so budget compliance is not checked at construction or decode.
- bounded-process limits not applied on Unix (see Silent drops); Unix `capture_private_observation` runs `command.output()` with no timeout or ceiling (`F/bounded-process/src/private_observation.rs:127-132`) while the Windows twin runs under the bounded runner (`.../windows.rs:190-243`).

---

## Stated limits

Quoted as the records quote them.

- `SEM/lib.rs:7-9`: "structural validation alone grants no execution or publication authority."
- `UV:352-356`: "activation replay has no origin-preserving projection for this structural operation".
- `UV:8142`: "transfer requires the same owned carrier, multiplicity and fresh result without issuance".
- `CO:5701`, `CO:5751`: "projected reference custody is not yet supported".
- `CO:4464`: "ordinary call has no complete stored reference effects".
- `CO:6498`: "reference-bearing owned calls require a whole internal argument".
- `CO:6519`: "projected reference argument path is not a declared record edge".
- `CO:6767`: "reference call result leaf is unsupported".
- `CO:5273`: "incoming reference leaf reconstruction capacity exceeded".
- `OU/construction.rs:7-8`: "This seed carries no admission authority until verified Terminal module catalogs and frontier facts are attached."
- `OU/lib.rs:35-38`: "A bare seed reconstructed without verifier custody leaves `runtime_evidence` and the frontier/proof tables empty, so downstream admission rejects unauthorized seeds immediately."
- `OU/dominance.rs:210-211` (test): "An absent entry is intentional too: the old equations do not impose reachability".
- `AO/lib.rs:24-26`: "every projection helper fails closed on duplicate or unverified declarations."
- `AO/access_lifecycles.rs:84`: "access lifecycle operands differ from the operation's ownership roles".
- `MC/lib.rs:20-29`: constructing or decoding "checks wire and digest consistency only".
- `MC/storage.rs:61`: "Valid only with exact ABI pointer placement; never denotes an invented stack home".
- `MC/layout/text_section_publication.rs:7-9`: decoding "without granting object container, symbol, or executable publication authority".
- `RH/constraints.rs:6-10`: "Incompatible fixed views remain explicit transition or split requirements rather than being silently reconciled".
- `RH/recovery.rs:717`: "First pressure point only; None proves the bounded walk encountered no pressure".
- `RH/recovery/fixed_view_copy_codec.rs:8-13`: "Decoding verifies both the transformed payload's content digest and canonical re-encoding before returning the unchecked plan."
- `SI/atomic.rs:4`: "baseline forms provide sequentially consistent machine ordering; weaker source requests may use that stronger realization".
- `SIA` error names used as limits: `UnsupportedUseDef`, `UnsupportedTiedOperand`, `UnsupportedEarlyClobber` (`SIA/live_ranges.rs:1799`), `UnsupportedEarlyClobberFixedConstraint` (`SIA/fixed_precolored_intervals.rs:143`), `UnsupportedTiedRegister`, `UnsupportedCrossBlockRange` (`SIA/fixed_precolored_split_requirements.rs:325, 552`), `UnsupportedPressureShape` (`SIA/spill_choice.rs`).
- `TO/descriptor_input.rs:265-273`: "descriptor scalar prefix has no native byte codec".
- `F/artifacts/src/compile_timings/counters.rs:3-7`: "not semantic authority or per-invocation snapshots".
- `F/bounded-process/src/lib.rs:27-28`: no filesystem, executable, credential or network sandboxing.
- `F/platform-custody/src/file_publication.rs:13-14`: staging name is process-scoped, "callers must serialize publication".
- `I/native-provider-runtime/src/channel.rs:10-15`: "The pipe name supplies uniqueness rather than authentication".
- `I/native-provider-runtime/src/worker.rs:200-204`: "native borrowed result needs an exact returned-home correspondence".
- `I/terminal-runtime/src/execution/host_imports.rs:113-122`: "residue obligations this runtime does not supply".

Records that found no limit comments: `candidates.rs` (only the header at `CAND:1-9`), `CVR`, `DR`, `CO:7101-9965` ("Limits are encoded as refusals only"), `UV:5751-18270` (no migration-debt comments; `UV:17751-18270` has no comments at all).

---

## Disagreements

1. **Memory-access role tags.** `unit_028.md` says the register-homes decoder has "22 roles: tags 0..21" (`RH/recovery/structural_contracts.rs:222-319`). `unit_030.md` says the selected-instructions encoder has "role tags 0-21, with gaps" (`SI/plan_identity.rs:869-1020`). Code: the encoder writes literal tags 0, 1, 2, 3, 6, 7, 10 to 21, plus 8 and 9 through an expression at `SI/plan_identity.rs:888-890` and 4 and 5 at `:1012` and `:1016`. Every tag from 0 to 21 is written. `unit_028.md` is right; there are no gaps.
2. **End of `function_structure`.** `unit_026.md` says it runs "to about 9675"; `unit_026_gap_011.md` says it ends at `UV:9674`. Code: `mod identity_indexes` opens at `UV:9675` and the closing brace is on `UV:9674`. The second pass is right.
3. **`validate_total_cfg`.** `unit_026.md` recorded only its signature (`UV:5745`); `unit_026_gap_011.md` flags this as a difference and records the body (`UV:5751-5804`). Not a contradiction about the code.
4. **Terminator wire layout.** `unit_028.md` suspects a mismatch risk because the decoder (`RH/recovery/selected_blocks.rs:95-162`) and encoder live in different crates and it did not read the encoder. `unit_030.md` read the encoder (`SI/terminator_encoding.rs:17-123`). The two records' tag lists are the same ten numbers. No mismatch in what the records report; I did not re-open these two files.
5. **Access tables.** `unit_026_gap_012.md` says `UV:14204` and `UV:14940` have the same truth table and cites `UV:5124-5129` as "a third copy". `unit_026.md` quotes `covers` at `UV:5124-5129` with `MutableBorrow` covering `SharedBorrow` only. Code (check 4): the first two agree; `covers` is different in one cell. `unit_026.md`'s quotation is right; "third copy" overstates it.
6. **Partial-move check in `affine_authority`.** `unit_026.md` calls `UV:709-720` and `UV:861-872` "duplicated in shape with" `CO` helpers "(called at :715, 872)"; `unit_025_gap_010.md` lists `UV:715` and `UV:872` as external callers of those `CO` functions. Both hold: `UV` calls the `CO` helpers and also has its own overlap logic (`UV:861-866`).
7. **Within `unit_029.md`.** `SIA/lib.rs:1007-1014` and `SIA/liveness.rs:281-285, 306-332, 849` are first classed "open" and later "traced" in the same record after `SI/blocks.rs` was read. I list them as traced.
8. **Macro resolution.** `unit_027.md` says unqualified invocations of `structural_leaf_copy_pair_pattern!` in psi and pipeline 00 "presumably resolve" to the Terminal Psi macro. Code: those invocations are written `terminal_psi::structural_leaf_copy_pair_pattern!()` (for example pipeline 00 `lowering_storage.rs:404`), so they are qualified. The conclusion (the abstract-operations copy has no user) stands.

No second-pass record contradicts a first-pass statement about code beyond items 2 and 3.

---

## What I checked myself

26 checks. Verdicts: matches / partly / does not match.

| # | Claim (record) | Site | Verdict | What the code shows |
|---|---|---|---|---|
| 1 | `abstract_catalog_graph` has zero call sites (`unit_026`) | `DR:905` | matches | Word grep over `omega-rust` `*.rs`: 1 line, the definition. |
| 2 | Retained-ownership check returns `Ok` when there are no frontier facts (`unit_026`) | `UV:880-885` | matches | `if unit.ownership_frontier_facts.is_empty() { return Ok(()); }` |
| 3 | `reference_result_memberships_match` returns `true` when the callee is not in the unit (`gap_011`) | `UV:8761-8772` | matches | Two `else { return true; }` arms: non-structural-call, and callee not found. |
| 4 | Three access tables are the same job (`gap_012`, `unit_026`) | `UV:5126`, `UV:14204`, `UV:14942` | partly | `structural_access_can_supply` and `access_covers` agree. `covers` lets `MutableBorrow` cover only `SharedBorrow`. |
| 5 | `install_store` drops claims when the destination is not owned (`gap_010`) | `CO:9798-9816` | matches | `current.claims.remove(&identity); if owned { ...insert... }`. |
| 6 | `validate_provider_attachment_specialization` rejects every function with a `ProviderAttachment` place and ignores two parameters (`gap_012`) | `UV:17357-17377` | matches | Parameters are `_boundary_machines`, `_types`; any such place returns the error. |
| 7 | Whether the validator re-derives definitions and uses or calls the constructor's functions (`unit_027`, unsure) | `UV:2649-2675` | matches the suspicion | `expected_definitions` calls `optimization_unit::operation_definition`; `expected_uses` calls `optimization_unit::operation_uses`; ownership is re-exported the same way (`UV:2679`). A variant missed by the wildcard arms at `OU/construction.rs:608, 741` is missed on both sides. |
| 8 | `apply_operation` ends `_ => {}` and omits `ElementViewRead` (`gap_009`) | `CO:6461`; grep in `CO` | matches | `_ => {}` at `CO:6461`; `ElementViewRead` occurs at `CO:1368` and `CO:9707` only. |
| 9 | `ResolvedBranchEvidence::as_conditional` has zero call sites (`unit_028`) | `MC/layout.rs:76` | does not match | 7 call sites: machine-emission `fragment_emission.rs:691`, `frame_application.rs:2179, 2466`, `function_realization.rs:738`; pipeline 09 `x86_branch_relaxation.rs:260, 464, 1081`. It is used. Removed from the unused list. |
| 10 | `ScalarControlFlowEvidence` variants are never constructed or matched (`unit_027`) | `MC/control_flow.rs` | matches | `ScalarControlFlowEvidence::` 0 matches; the name occurs 5 times in 3 machine-code files. |
| 11 | `structural_leaf_copy_pair_pattern!` names a nonexistent variant and has no qualified use (`unit_027`) | `AO/operations.rs:1343-1348` | matches | Macro body names `StructuralCaseLeafCopy`; the only other hit in the abstract-operations tree is a Terminal Psi value at `UV:163`; 0 qualified uses. All invocations elsewhere are `terminal_psi::`-qualified. |
| 12 | Memory-effect tag 19 is written for two variants and the decoder picks the first (`unit_029`, `unit_030`) | `SI/effects/catalog/identity.rs:282, 440`; `SI/effects/program/encoding.rs:1027, 1037` | matches | `&[19]` for `ReadTableFieldV1` and for `CatalogFenceV1`; decoder has `19 if allow_scalar_call =>` before `19 =>`. |
| 13 | `use_transition` result ignored; `finish` is tautological (`unit_029`) | `SIA/fixed_precolored_split_requirements.rs:145-178, 455` | matches | `cuts.use_transition(&prior, site, destination_view);` with no use of the `bool`; `used` is filled only from `self.transitions.get(&key)` and checked against the same map. |
| 14 | Register names select availability views (`unit_029`) | `SIA/legality.rs:167-170` | matches | `["rax", "rcx"]` / `["x0", "x1"]` passed to `view_named`. |
| 15 | `BaselineDecisionLog::decode` allocates from an unverified count and does not check Choose membership (`unit_031`) | `F/optimization-core/.../baseline.rs:154-178` | partly | `Vec::with_capacity(candidate_count as usize)` on a raw `u32`; no membership test in `decode`. Whether `BaselineDecisionRecord::record` sorts the roster was not opened. |
| 16 | `has_single_hard_link` is never called (`unit_031`) | `F/platform-custody/src/lib.rs:302` | matches | 1 grep hit. |
| 17 | `target_cost_model` free function has no outside caller (`unit_028`) | `PAM/costs.rs:16` | matches | Other calls are at `PAM/costs.rs:186-238`, inside the file's tests. |
| 18 | Dominators are computed twice (`unit_025`, `unit_027`, suspected across units) | `CAND:3224`; `OU/structural_places.rs:571` | matches | Both compute dominator sets; the first restricts to reachable blocks and uses `BTreeSet`s, the second calls `dominance::solve` on all declared blocks. |
| 19 | A second `cyclic_machines` exists in pipeline 01 (`unit_025`, suspected) | `CAND:9584-9641`; pipeline 01 `control_flow_analyses.rs:133` | matches | Same question per function; DFS marks versus strongly-connected components. |
| 20 | Whether the selected plan identity covers `qualification_establishments` (`unit_028`, open) | `SI/` tree; `LO/identity/structural.rs:354-369` | partly | The field name has 0 occurrences under `selected-instructions/src`; `encode_operation_result` writes place, type, multiplicity, qualifications, two projected lists and claims, and not this field. So no encoder in these two crates binds it (inferred: a non-empty value would be dropped by the codec without an identity mismatch). Encoders in other crates were not searched. |
| 21 | `provenance_proven_unreachable_at` is defined twice (`unit_025`, suspected) | `CAND:10488`; pipeline 01 `control_flow_cleanup.rs:1491` | matches | Identical bodies. |
| 22 | `attachment_storage_types` is a line-for-line copy (`gap_013`) | `UV:17908-17947`; `foundation_validation.rs:8566-8605` | matches | Diff shows two differing lines: visibility and a `use`. |
| 23 | Two integer-type encoders with different contents (`unit_027`) | `OU/identity.rs:125-131`; `OU/range.rs:390-400` | matches | No carrier byte in `identity.rs`; sign tags reversed between the two. |
| 24 | Role tags 0..21 (`unit_028` versus `unit_030`) | `SI/plan_identity.rs:869-1020` | `unit_028` matches | See Disagreements 1. |
| 25 | `function_structure` ends at 9674 (`gap_011`) | `UV:9674-9675` | matches | See Disagreements 2. |
| 26 | Explicit trivial-affine-local form returns `Ok` early (`gap_013`) | `UV:18044` | matches | `return Ok(());` closes the explicit-form block. |

Corrections applied to this digest from the checks: `as_conditional` is not unused (check 9); the access-table "third copy" differs (check 4); the open question about `qualification_establishments` is narrowed (check 20); the open item on `OU/construction.rs` wildcards now records that the validator calls the same functions (check 7).

---

## Not covered

- **Records.** All 12 records for units 025 to 031 were read in full. None skipped.
- **First-pass gaps closed by the second pass.** `CO:1101-9965` (gap_009 `1101-7100`, gap_010 `7101-9965`) and `UV:5751-18270` (gap_011 `5751-11750`, gap_012 `11751-17750`, gap_013 `17751-18270`). Together with the first pass every line of the seven unit files is reported read.
- **Source in these crates outside this area's units.**
  - `SEM/access_references.rs` (120 lines) belongs to unit 024; no record here covers it. The crate total is 43,116 lines; 42,996 are covered here.
  - terminal-runtime: only 9 of 44 files (2,548 of 14,328 lines) are in unit 031. The other 35 files are in unit 032, outside this area. Everything this digest says about terminal-runtime concerns `execution.rs`, `execution/{components,entry,host_imports}.rs` and the three adapters `absolute_value`, `affine_transform`, `allocation` (with `admission`, `backing`).
- **Read at lower detail (the readers' own statements).** Test modules in `SIA/live_ranges.rs`, `SIA/liveness.rs`, `SI/calls.rs`, `SIA/lib.rs`, `SI/effects/catalog/identity.rs:781-2335` (`unit_029.md`); `SI/effects/catalog/validation.rs:7-2053`, `SI/effects/program.rs:101-1464`, `SI/plan_identity.rs:1053-1671`, `LO/tests.rs`, `LO/tests/identity.rs` (`unit_030.md`); test code in unit 028 recorded as one-line summaries.
- **Not read by any record here and relied on by open items.** `terminal_semantics` kernels (`owned_structure`, `descriptor_results`, `scalar_ordering`, `evidence_calls`), pipeline crates 00 to 09, `calling_conventions`, `isa_x86_64`/`isa_aarch64` catalog builders, Terminal Psi codec other than the two functions named in duplicate jobs 6 and 7.
- **Counts not split per name by the readers.** Several combined greps (for example `constructible_record|boundary_requirements_match|boundary_completion_matches|reconstruct_fact_index` = 11; `storage_projection|is_reference_projection|...` = 6 in `CO`); no per-name call count is claimed for those.
- **My own checks** covered 26 claims; the other open, unused and duplicate entries are reported as the records state them.
