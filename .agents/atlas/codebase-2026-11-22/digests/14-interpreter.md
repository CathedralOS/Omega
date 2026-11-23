Built from records `unit_076`, `unit_076_gap_034`, `unit_076_gap_035`, `unit_077`, `unit_078`, `unit_079`, `unit_079_gap_036`, `unit_080`, `unit_081` (its Part A, interpreter files only) and `crate-map/14-interpreter.md`, at commit `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`.

# Digest: 14-interpreter

Path abbreviations. `I/` = `source-rust/subsystems/interpreter/`. Inside it: `E/` = `I/embedding/` (with `P` = `E/program.rs`, `BE` = `E/boundary_effects.rs`, `BI` = `E/boundary_inputs.rs`, `BSI` = `E/boundary_storage_inputs.rs`, `BV` = `E/boundary_values.rs`); `IN/` = `I/instructions/`; `M/` = `I/memory/` (with `X/` = `M/extents/src/`, `MAT/` = `M/materialization/src/`, `SA/` = `M/storage-access/`); `H/` = `I/host/src/execution/`, `HI/` = `H/host_imports/`, `NP/` = `HI/native_provider/`; `CL/` = `I/host/clock/`; `FS/` = `I/host/filesystem/`; `W/` = `I/host/native-worker/src/`.

Words used here. **Psi** is the portable compiled form of a program; the interpreter runs its last form (Terminal Psi; internal name). A **place** is a numbered slot naming a structural value (`PlaceId`). **Custody** is the interpreter's runtime record of who owns a value: live claims (`LiveClaim`) and the "affine frontier" (the parts of an owned value still live). A **boundary call** is a call from the guest program to a host import, provider or component. `malformed()` (`BV:721`) is the generic "the verified artifact is inconsistent" refusal; it records the Rust source line of the failing check instead of a message.

"Suspected", "inferred", "traced" and "not traced" are the record readers' words. Counts are of places the records cite, not of defects. Where I checked a claim myself it is marked "(checked, §10)".

## 1. What this area does

This area runs Psi. Given a compiled Terminal artifact (bytes, a borrowed view, or a retained view), runtime provisions and a host request, it loads an immutable `TerminalProgram`, creates a `TerminalInstance`, binds host imports and components to the program's boundary slots, and steps blocks, operations and terminators until the entry returns, crashes, suspends for an effect, or runs out of work budget. While it runs it checks ownership (claims, loans, frontier), storage geometry and declared contracts, and refuses with `malformed()` or a typed error when they disagree. Outputs are `TerminalExecutionResult`/`TerminalExecutionStatus` (`I/interpreter.rs:130-134`) for the engine and `TerminalRuntimeObservation` (`I/host/src/execution.rs:90`) for the hosted runtime. The compiler's Build stage also uses the engine directly for compile-time evaluation through `TerminalProgram::instantiate_compiler_evaluation` (`P:3526`). The area is nine crates, 103,372 lines (crate map; `units.tsv`). Loading does not verify proofs (crate map, from `I/README.md`).

Per crate, in pipeline order (storage services, engine, host):

- **extents** (`X/`, 4 files, 4,690 lines). Conserved authority over address ranges: an `Extent` is a linear carrier whose split, partition, attenuate, loan and merge give the carrier back on failure. In: identity values from `extent_contracts`. Out: `Extent`, `ExtentLoan`, root grants. Work in `Extent::merge`/`validate_merge` (`X/extent.rs:237`, `:327-385`) and `ExtentRootGrant::mint` (`X/lib.rs:347`). Also holds activation-claim ledgers, external (DMA-style) loans, device ordering events (`X/loans_and_claims.rs`) and address-space mappings (`X/mapping.rs`); only part of this is used in the workspace (§7).
- **storage-materialization** (`MAT/`, 4 files, 5,305 lines). Reads and writes field bytes according to an admitted layout plan; never chooses a layout (`MAT/lib.rs:1-3`). In: `LayoutPlanReport`, field values. Out: staged or decoded bytes, or a `SymbolicMaterializationPlan`. Work in `stage_aggregate_layout` (`MAT/materialization.rs:798-1109`), `decode_scalar_layout` (`:1111-1249`) and `derive_symbolic_materialization_with_inner_layouts` (`MAT/symbolic_materialization.rs:46-529`).
- **storage-access** (`SA/`, 4 files, 2,665 lines). Binds a physical resource profile to an exact admitted extent and loan. Work in `AdmittedResourceProfile::restrict_to_loan` (`SA/src/lib.rs:210-257`) and `authorize` (`SA/src/authorization.rs:146`).
- **terminal-interpreter** (`I/`, 22 files, 68,734 lines; library root `I/interpreter.rs`). The engine. `P` loads the program, instantiates, binds imports and components (`TerminalInstance::bind_import` `P:1751`, `bind_component` `P:1821`), and runs through `invoke_to` (`P:2397`), which calls `resume_with_components`. The dispatch loop is `resume_operations` (`IN/execution.rs:3213-3603`), a macro-driven match over every `OperationKind` and terminator with no wildcard arm (`IN/execution.rs:2407-2415`, `:3565-3592`). A pre-run capability walk, `capabilities::check` (`IN/execution.rs:241-730`), finds which imports, providers and external completions a program needs. Calls live in `IN/calls.rs` (`enter_call` `:442`, `execute_boundary_call` `:3099`, about 410 lines), value construction in `IN/structural.rs`, block ends and returns in `IN/terminators.rs` (`prepare_block_bindings` `:431-1052`, `settle_return_structural` `:1949`). Storage is in `M/`: scalar homes and the paid row table `RuntimeRows` (`M/primitives.rs:4910`), byte and element views (`M/bytes_and_views.rs`), custody (`M/custody.rs`), records and arrays with staged `ConstructorStorage` (`M/records_and_arrays.rs:1105`), provider-backed storage (`M/providers.rs`), and references (`M/references.rs`, with `capture_reference` `:2325-3078` and `validate_reference_return` `:4296`). Host-boundary checking is in `E/` (`commit_boundary_result` `BE:3084`, `BoundaryStorageInputs` `BSI:2368`). Resuming a paused Build in a new publication is `continue_checked_entry` (`P:2279`) and `relocate_continuation_storage` (`E/continuation.rs:107`).
- **interpreter-numeric-imports** (`I/numeric-imports/numeric_imports.rs`, 526 lines). Interprets selected IEEE float operations against the demanded import signature. Entry `bind_numeric_import_slot` (`:29`), work in `evaluate` (`:192-378`).
- **native-provider-runtime** (`W/`, 16 files, 5,710 lines). Runs compiler-produced native adapters in a separate worker process over a private channel with shared file-backed memory, so a foreign fault kills only the worker. Parent side `NativeProviderProcess` (`W/native_provider_runtime.rs:154`); worker loop `run_worker_if_requested` (`W/worker.rs:100`). Also owns the Linux filesystem syscalls (`W/host_filesystem.rs:120`), an in-memory Linux-shaped filesystem for test runs (`W/private_filesystem.rs`), and `read_host_clock_nanoseconds` (`W/host_clock.rs:12`).
- **native-provider-sponsor** (`I/host/native-sponsor/sponsor.rs`, 349 lines). Launches the worker with fixed process limits and installs adapter images matched by runtime identity. Entry `for_adapters` (`sponsor.rs:22`; corrected, §10), match in `lookup_adapter` (`:65-73`).
- **terminal-runtime** (`I/host/`, 42 files, 15,101 lines; `#![forbid(unsafe_code)]`, `I/host/src/lib.rs:26`). The hosted entry. `execute` / `execute_sliced` / `execute_with_image_installation` (`I/host/src/execution.rs:179`, `:185`, `:200`); `HostedTerminalProgram::prepare` (`:230`) admits the request; `bind` (`H/host_imports.rs:409-681`) builds the import table; `execute_installed` (`H/hosted_invocation.rs:12`) runs the instance in work-sized slices. Host adapters cover clock (`CL/`), filesystem (`FS/`), buffered and descriptor I/O, emulated `calloc`/`free`/`memset`, `abs`, `lldiv`, `strtol`, `inet_makeaddr`/`inet_lnaof`, `_setmode`, CoreGraphics `_CGAffineTransformMake`, and native providers (`HI/native_provider.rs`).
- **retained-image-runtime** (`I/host/retained-image/`, 292 lines). When a program observes linked code layout, checks a retained native image against the target and manifest, builds the code-layout environment, then runs through terminal-runtime (`execution.rs:23`, `:50`). With no image it forwards to `terminal_runtime::execute_sliced` (`:35-37`).

## 2. How it decides

Almost every refusal in the read code is `malformed()` with no message; records 076, gap_034 (about 40 sites in `BSI:1568-2698`), 079 and 080 each note this. Decisions group as follows.

**Identity** (most decisions; about 30 places cited across the engine, 10 in the host and worker, 8 in extents and storage-access).
- Host buffers are matched to captured storage by `Arc<()>` pointer identity plus argument index (`BE:3857-3898`; `BSI:1639-1642`, `:1681-1685`, `:2179`).
- Import adapters fit a slot by whole-value equality of `TerminalImportSignature` (`P:402`, `P:1809`). Components by provider, receiver and supply identity (`P:311-322`, `P:425-427`). Process-exit admission compares the domain `Arc` by pointer (`P:1607`).
- Program-level admission compares execution identity, target-machine identity, layout identity (`P:2936`, `P:2988-2993`, `P:3307-3368`), and the five scalar-input fields (`P:3866-3877`, `P:3897-3902`).
- Resumption requires both frames to share one invocation account (`P:2311-2318`) and the exact program counter after the call (`P:2319-2333`); entry-call watches compare `(machine, block, next_operation)` and the caller stack (`P:1522-1552`).
- Byte and element views: same `opaque_identity`, `path` and `structural_type` as the descriptor home (`M/bytes_and_views.rs:3126`, `:3706`, `:616-629`). Custody aliasing: shared `opaque_identity` with overlapping paths (`M/custody.rs:1145-1183`). Returned references: rebuilt expected backing must equal the actual referent (`M/references.rs:4375`, `:4495-4520`). Provider windows: arena handle plus range (`M/providers.rs:4379-4406`), same allocation by `Arc::ptr_eq` (`:144`, `:733-744`).
- Extents merge only on equal address space, rights, provenance, era, sharing, lineage root and origin with sibling paths (`X/extent.rs:328-379`); receipts compare 12 fields (`X/loans_and_claims.rs:678-704`) or whole mapping structs (`X/mapping.rs:248-262`). `restrict_to_loan` compares address space, provenance, era, origin and lineage root (`SA/src/lib.rs:214-238`).
- Host: worker adapter digest (`W/worker.rs:200`), era equality for ids (`W/native_provider_runtime.rs:336`, `:385`, `:415`), sponsor adapter match (`sponsor.rs:71`), retained image manifest and view identity (`retained-image/execution.rs:67-74`), private entry RAM recipe by `runtime_position` (`H/entry.rs:67-85`).

**Place and path** (about 15 places, all in the engine).
- Case activity: the stored case at each `Case` prefix must equal the segment; a missing stored case is an error, never "inactive" (`BI:2278-2305`; `BSI:1710-1745`).
- Qualification changes on an edge: binary search on `(machine, edge, argument_ordinal)` (`IN/terminators.rs:404-427`); claim re-homing to the parameter place (`:934-960`).
- Endpoint choice for a scalar place follows a fixed eight-step order (`M/primitives.rs:3369-3593`); byte-descriptor lookup follows a seven-step order ending in a first-match scan of the current frame and every call-stack frame (`M/bytes_and_views.rs:588-740`, `:715-739`; element views `:4012-4050`).
- "Current frame or `call_stack[depth]`" by `home.depth == self.call_stack.len()` (7 sites in `M/primitives.rs`, grep count, 080).
- Subtrees are selected by range scan on `(opaque_identity, path)` keys (`M/records_and_arrays.rs:5957-5969`); retirement GC builds live roots from every frame (`:5588-5804`). Carrier access by place (`M/references.rs:2126-2175`).

**Value** (about 15 places).
- Contract `requires`/`ensures` are evaluated three-valued; `Some(false)` and `None` both refuse (`BE:150-160`, `:231-241`). Entry preconditions likewise, with caller-established clauses skipped (`P:3135-3153`).
- Crash admission by cause bucket and guards (`BE:2126-2156`). External completion return code must be a signed 32-bit integer (`IN/terminators.rs:2896-2909`).
- Host values: clock selectors per namespace (`CL/bindings.rs:73-83`), poll count and timeout (`:423`), x86 RFLAGS mask `0x3f7fd7` (`BE:4489`), `memset` fill must be 0 (`HI/allocation.rs:407`), allocation sizes overflow to address 0 like C (`:76-102`).

**Position** (about 10 places).
- `AggregatePlan` requires the nth plan leaf to equal the nth payload field in path, type and range; extras and missing leaves refuse (`BV:1847-1898`).
- Dynamic dispatch picks a slot by index, or by scanning later slots for an equal family tuple (`IN/calls.rs:2975-3013`).
- Native formals must be in exact semantic order (`HI/native_provider.rs:496-520`); C word records place field `n` at `n * 8` (`HI/scalar_record.rs:146`, `:191`).
- Worker ordinals are dense and mirrored (`W/worker.rs:194`, `:256`); activation claims release strictly LIFO (`X/loans_and_claims.rs:382-394`).

**Spelling** (about 15 places; a decision compares source text or a formatted string).
- `instantiate_compiler_evaluation` recognizes the layout-target argument by identity text `"toolchain::LayoutTarget"` (`P:3579-3585`).
- Component calls compare type identity TEXT across two programs (`IN/calls.rs:1342-1354`, `:1743-1755`, `:2129-2141`).
- Native service reach: owner is the `package:` prefix before the first `::` (`NP/service_reach.rs:12-16`, `:71`).
- Filesystem service reach accepts six exact legacy label strings (`FS/bindings.rs:528-550`).
- Provider layout entries are matched by field name and runtime fields are given names `format!("field:{:?}", field.id)` (`M/providers.rs:1657`, `:3363-3369`, `:3526`, `:3949`, `:4116`).
- Target names as strings (`P:2942`, `P:2949`; `accepts_target_name` `P:3334`, `IN/execution.rs:339-356`); the `native-provider-retirement` capability string (`I/host/src/execution.rs:139`).
- Windows filesystem argument rules switch on symbol bytes (`FS/windows.rs:250-311`); C locator spellings are listed per adapter (`CL/admission.rs:45-70`, `CL/windows.rs:48-76`, `HI/absolute_value.rs:221-267`).
- Materialization joins unnumbered fields by name and numbered ones by member identity (`MAT/materialization.rs:36-40`).
- `runtime_boundary.rs` tests read manifests and source as text (`I/host/src/runtime_boundary.rs:4-31`).
- Tests that pin error text with `contains(...)`: `E/instance/storage_observation/tests.rs` (9 asserts, 370-1121), `E/reference_completion_tests.rs:2233-2234`, `W/physical_tests/provider_references.rs:367-376`, `SA/src/tests.rs:410`, `:931`.

## 3. The same job in more than one place

Engine, embedding layer:
- Case-activity walk: `case_projection_in_value_is_active` (`BI:2241`) vs `normal_qualification_subject` (`BE:2407-2444`); `case_projection_with_membership_is_active` (`BI:2267`) called three times with different closures (`BSI:1739`, `:1778`, `:2104`).
- "u8, unsigned, not address" predicate: `BI:699-702`, `:898-901`, `:1019-1027`. `IntegerType::new(Unsigned, 8)` is spelled 15 times in `M/` (8 in `primitives.rs`, 6 in `bytes_and_views.rs`, 1 in `records_and_arrays.rs`; grep, 080).
- Type-shape walks for "needs materialized custody": `requires_materialized_host_custody` (`BE:3003`) vs `requires_contents` (`BI:3198`) (suspected, not compared).
- Normal-qualification coverage block twice in `BSI` (`:1872-1887`, `:2123-2139`); "find the binding with this argument index" five times (`BSI:1667`, `:1766`, `:1873`, `:2094`, `:2124`).
- `StructuralScalarRuntimeField` and `StructuralByteSequenceRuntimeField` are near copies (`BV:2475-2481`, `:2492-2498`).
- Component slot checks in both `bind_component` bodies (`P:416`, `P:1821`); import signature check in both `bind_import` bodies (`P:401-406`, `P:1808-1811`).
- `admitted_scalar_inputs` and `scalar_application_inputs` check the same five fields with different comparisons (`P:3869-3875` vs `P:3898-3902`). `import_declaration` (`P:3085`) and `imports` (`P:3475-3484`) apply the same predicate (suspected).
- `HeldCoordinates` checks and `Coordinates` rewrites the same shapes in `E/continuation.rs` (`:320`, `:561`).

Engine, instructions:
- "Which callees can this call reach": the pre-run walk (`IN/execution.rs:372-598`) vs the run-time call code (`IN/calls.rs`). The walk follows only the declared slot of a dynamic call (`IN/execution.rs:505-529`) while the runtime can select a later slot (`IN/calls.rs:2990-3012`) (checked, §10).
- Register-operation refusal list twice (`IN/execution.rs:429-464`, `:3396-3431`).
- Result dispatch on `{Unit, Scalar, Structural}` three times (`IN/calls.rs:2924-2950`, `:3017-3043`, `:3321-3368`); `execute_call_structural` and its scalar-argument twin (`:3047`, `:3073`).
- Retiring a moved owned source: five sites with different missing-value handling (`IN/calls.rs:195-204`, `:1649-1653`, `:3668-3695`; `IN/structural.rs:1700-1712`; `IN/terminators.rs:105-128`).
- Aggregate staging (`ConstructorStorage::new` ... `publish`): `IN/execution.rs:3711-3758`, `IN/structural.rs:139-157`, `:252-264`.
- Block-argument vs call-argument loan classification (`IN/terminators.rs:431` vs `IN/calls.rs:77`).
- Handler names wider than their scope: `execute_exact_integer_add` handles five arithmetic forms (`IN/execution.rs:4355-4429`), `execute_integer_bitwise_and` handles or/xor (`:4272`), `execute_wrapping_integer_shift_left` all four shifts (`:4307`).

Engine, memory:
- Descriptor-home equality three times (`M/bytes_and_views.rs:3126`, `:3706`, `:616-629`); `relocated_descriptor` twice (`:129-146`, `:3592-3604`); unsigned count extraction at least eight times (`:3969-3988`, `:2992-3001`, and inline copies).
- Frontier splitting three ways in `M/custody.rs` (`:144-201`, `:1790`, `:277-297`); claim matching by place and path prefix four times (`:1411`, `:2443`, `:2352-2358`, `:381-395`); `bind_arguments` vs `bind_boundary_arguments` (`:815`, `:847`).
- Row reservation: `reserve_claim` (`M/primitives.rs:48-59`), `reserve_row` (`:4893-4907`), and inline in `M/providers.rs:445-450`, `:769-775`, `:2079-2084`.
- Scalar byte codec: `encoding::encode/decode` (`M/primitives.rs:1785`, `:1820`) wrapped twice in `M/providers.rs` (`:3399-3414`, `:3847-3867`) and a third inline version (`:1577-1627`).
- Five access-lifecycle operations repeat one contract-check / claim-bind / commit skeleton (`M/providers.rs:819`, `:989`, `:1866`, `:2125`, `:2909`), with `establish_record` (`M/records_and_arrays.rs:2537-2665`) a sixth; window arithmetic five times (`M/providers.rs:1164-1174`, `:1539-1550`, `:2089-2106`, `:3081-3092`, `:3261-3270`); two allocation-overlap loops (`:601-613`, `:745-753`); image encoder and decoder walk the same shapes (`:3774`/`:3837` vs `:3333`).
- Shape walkers over the type graph: `contents::collect` (`M/records_and_arrays.rs:4728`), `ConstructorStorage::stage_array` (`:1656`), `ValueImageReader::structure` (`M/providers.rs:3894`) (suspected). "Resident cell keeps its original backing" twice (`M/primitives.rs:5680`, `M/records_and_arrays.rs:3804`).
- Repeated-array offset and stride validation twice (`MAT/materialization.rs:225-267`, `:966-1018`).
- Lineage split and merge in `X/extent.rs:134-173`, `:237-267` and again for activation claims (`X/loans_and_claims.rs:397-503`); accessor matches on `LoanBacking` (`X/lib.rs:90-153`) and on `MappingSource` (`X/mapping.rs:106-171`); receipt mismatch chains (`X/loans_and_claims.rs:674-704`, `X/mapping.rs:242-267`).

Host and worker:
- Two C `abs` adapters: `AbsoluteValueImport` (`HI/absolute_value.rs`) and `IntegerAbsoluteImport` (`HI/integer_absolute.rs`). The first runs earlier in the chain and claims every locator the second accepts, so the second is unreachable (inferred in 077; checked, §10).
- Locator matching: `matches_platform_c_symbol` (`H/host_imports.rs:215`) has 6 call sites in `I/host/` (my grep; 077 says seven adapters), but the clock family uses its own loops (`CL/admission.rs:45`, `CL/windows.rs:48`).
- Linux syscall-number selection per family: clock (`CL/admission.rs:22-33`), filesystem (`FS/linux.rs:292`), descriptors (`HI/descriptor_io.rs:218`).
- Three filesystem call tables: Linux kernel (`FS/linux.rs:176-215`), libc (`FS/bindings.rs:261-437`), Windows (`FS/windows_contracts.rs:39-453`); the private filesystem dispatch must agree with the Linux table by operand shape (`W/private_filesystem.rs:220`, `:369`).
- Guest descriptor and handle numbering, four rules: libc path refuses 0..=2 and allocates from 3 (`FS/bindings.rs:790`, `:864`); Linux kernel host table from 0, skipping modeled streams (`W/host_filesystem.rs:179`); private fs lowest free (`W/private_filesystem.rs:654`); Windows handles (`FS/windows.rs:42`) (checked, §10).
- Two schema walkers over the record/array graph: `record_fields.rs::Schema` and `record_shape.rs` (`NP/record_fields.rs:91`, `NP/record_shape.rs:409`); `scalar_location` vs `expand` (`NP/original_homes.rs:744-800`, `HI/borrowed_storage_geometry.rs:65`).
- Native-signature classification plus `admit_exact_foreign_call` repeated in `HI/network_address.rs:236-260` and `HI/scalar_record.rs:107-221`; the calling-plan check repeated in `FS/windows_contracts.rs:534`.
- Deadline range and an era counter `NEXT_ERA` in both sponsor and runtime (`sponsor.rs:109`, `:112`; `W/native_provider_runtime.rs:173-174`).
- Image path re-admission: `admit_runtime_supplies` runs twice on the retained-image path (`retained-image/execution.rs:42`, `I/host/src/execution.rs:208`); component preparation repeats target-machine admission (`H/components.rs:30-37`) and the `Component` mechanism skip (`H/components.rs:42-47`, `I/host/src/execution.rs:320-325`).

## 4. Fixed lists of shapes

Engine, embedding:
- `commit_boundary_result` accepts seven `(OperationResult, BoundaryMachineResult, TerminalEffectResult)` pairings; anything else is `malformed()` (`BE:3090-3186`). `supports_aggregate_result`: Record, FixedArray, PrimitiveScalar, ByteSequence, ElementView, and Sum only with plain leaves (`BE:2964-2980`). Scalar loans need `PrimitiveScalar` and Shared/Mutable (`BE:3749-3763`).
- `AggregatePlan::new`: PrimitiveScalar, shared-readable ByteSequence, ElementView with None/SharedBorrow, Record, literal-length FixedArray; else `malformed()` (`BV:1598-1667`). `declared_scalar_fields`: `(Record|FixedArray|PrimitiveScalar, None)` or `(Sum, Some(case))` only (`BV:1793-1826`).
- `bind_byte_arrays` paths only of `Field`/`FixedIndex` (`BI:1075-1082`); `entry::validate_type` Record and FixedArray only (`BI:3183-3188`).
- `capture_fields`: owned with matching access and type, or unrestricted borrow (`BSI:2427-2469`); reference outputs only to a `MutableBorrow` parameter with a `Reference` endpoint (`BSI:1666`, `:1733-1736`); host reference results only from `Parameter` origins with empty path (`BSI:2018-2025`); `select_reference` refuses ByteView (`BSI:2299-2326`).
- Call observations: integer scalar or owned plain scalar-case argument only (`E/call_observations.rs:178-232`).
- Continuation: refuses any retained artifact-bound state (`E/continuation.rs:120-150`, `:204-227`); suspended callers only Unit/Scalar/Structural (`:56-86`); bindings only `Immutable`/`MutableArray` (`:751-770`); `continue_checked_entry` refuses a long fixed list of authority fields (`P:2334-2368`).
- Adapters `TerminalScalarImport`/`TerminalUnitImport`: normal return, no structural parameters, scalar or unit result (`P:117-237`). Component returns: Unit, Scalar, Structural; ScalarCase and ScalarArray refused (`P:839`). Storage observation: `observe_case` only `[]` or `[Case, Field]` (`P:1241-1248`), `observe_array` only `FixedIndex` to a scalar (`P:1297`, `:1324`).
- External completion kind has one variant, `ProcessExit` (`P:1589-1591`; `IN/terminators.rs:2910`).
- `instantiate_compiler_evaluation`: storage `Borrowed` or `Retained` only (`Owned` refused, `P:3541-3545`); the only structural argument is an opaque owned `toolchain::LayoutTarget` (`P:3585`).

Engine, instructions:
- Successor argument classification: a fixed ten-step order ending in `malformed` (`IN/terminators.rs:478-785`). Borrowed-descriptor returns only for an Affine unqualified signature from a `MutableBorrow` view parameter (`:2122-2174`).
- `bind_byte_sequence_arguments` three forms then `malformed` (`IN/calls.rs:304-343`); plain `execute_call` needs a scalar result with no structural parameters or claims (`:3755-3761`); component results Unit/Scalar/Structural only (`:1706-1861`); component inputs Owned or reading borrow, no projected qualifications (`:1326-1336`); `rehome_path` refuses `Referent` and `Elements` (`:1986-1990`, `:2108-2112`); `family_actual_value` integers and booleans only (`:3786-3799`).
- `execute_native_code_layout` expects exactly `[entry, regions]` with elements `[base, length]` (`IN/execution.rs:3669-3694`); atomic read-modify-write five integer operations (`:188-197`); trapping integer eight operations (`:4663-4701`).
- Host reference payloads Aggregate/Resident/Zeroed, resident only a shared borrow with no nested references (`IN/structural.rs:271-275`); nine zeroed-leaf variants (`:798-938`); `byte_carrier_leaf_copy` Boolean and Integer only (`:2284-2294`).

Engine, memory:
- `narrow_byte_sequence_binding`, `execute_byte_sequence_store`, `execute_element_view_subslice`, `scalar_width` (8/16/32/64 non-address integers, binary32/64) (`M/bytes_and_views.rs:2514-2644`, `:2337-2396`, `:4987-4991`, `:5412-5423`).
- `split_affine_frontier_at_projection`: Record, literal FixedArray, Sum/Mixed; else `AffineProjectionNotRepresentable` (`M/custody.rs:1790-1908`); `decompose_owned_subject` Record only (`:1682-1691`); `retains_source_lender` lists accepted (lender access, multiplicity, source access) triples (`:1004-1067`).
- `resolve_scalar_view_storage` walks only `(Case, Sum|Mixed)`, `Field`, literal `FixedIndex` (`M/primitives.rs:3658-3762`); `capture_primitive_loan` a five-source chain (`:2537-2707`); `endpoint_identity` and `location_alignment` handle a fixed subset of the seven endpoint kinds (`:6129-6149`, `:6226-6346`); address path segments `FixedIndex`, `Field`, `Case` only (`:5984-5995`).
- Provider lifecycle calls have fixed arity (`M/providers.rs:827`, `:1002`, `:1880`, `:2135`, `:2918`); image shapes PrimitiveScalar, literal FixedArray, Record, Sum, Mixed, depth 128 (`:3452-3554`, `:3894-3970`); `record_layout` five storage recipes (`:4098-4210`); resource profiles read a fixed schema with 32 regions and 16 transfers (`:2349-2617`).
- `contents::collect` lists PrimitiveScalar, AtomicCell, Record, ElementView, FixedArray, Sum/Mixed; every other shape becomes an `Opaque` leaf rather than a refusal (`M/records_and_arrays.rs:4971-4977`; see §6).
- References: `collect_scalar_fragments` Record and nonzero literal FixedArray, depth 128 (`M/references.rs:261`, `:364`); `execute_access_boundary` refuses `Take` on ordinary RAM (`:1844-1846`).
- Materialization: scalar placements at most 8 bytes (`MAT/materialization.rs:166-170`, `:880-885`); `At` cannot mix with scalar placements (`:171-176`, `:865-873`); symbolic paths only as listed in quoted refusals, depth 64 (`MAT/symbolic_materialization.rs:337-406`, `:614-626`, `:734`).
- `authorize` matches every `AccessOperation` with no wildcard (`SA/src/authorization.rs:23-65`).

Host and worker:
- The import chain in `bind` is the complete list: Linux kernel filesystem, libc/Windows filesystem (with descriptor I/O sent to the native import), `abs`, allocation, clock, descriptor I/O, affine transform, `abs` again, `lldiv`, `strtol`, network address, `_setmode`, native provider; anything else refuses naming the boundary (`H/host_imports.rs:483-639`, checked §10). Builtins `LinuxWriteLine` and `HostedWriteByteI32` go to buffered output; `HostedReadByte` is retired with an error (`:448-452`).
- Filesystem: 30 libc rows (`FS/bindings.rs:261-437`), 24 Linux kernel operations (`FS/linux.rs:182-208`), 27 Windows contract rows (`FS/windows_contracts.rs:39-453`) and 22 symbol argument rules (`FS/windows.rs:250-310`); paths only `UnsignedArray(4096, 8, SharedBorrow)` (`FS/bindings.rs:258`).
- Clock: five operation shapes (`CL/admission.rs`), selector 32-bit, poll with zero descriptors only (`CL/bindings.rs:17-18`, `:423`).
- Native words: `Integer` or single-field records (`HI/native_word.rs:38-55`); C word records of N 8-byte words (`HI/scalar_record.rs:43-69`, `:248-255`); scalar-leaf record codec (`NP/values.rs:72-82`); network address two operations (`HI/network_address.rs:147-198`); scalar codec Boolean, binary32, binary64, integer (`HI/scalar_codec.rs:36-49`).
- Native provider results Unit/Scalar/Record/Reference/ProviderReference (`HI/native_provider.rs:136`, `:299-418`); worker wire tags 1-9 (`W/worker.rs`); result geometry not ByteView (`:218-227`); `Geometry::read` fixed borrow 8/8, byte view 16/8 (`W/wire.rs:174-200`).
- Numeric imports: classification result is a 5-case sum with 4 fields in fixed ordinal order (`numeric_imports.rs:48-52`, `:159-165`).

## 5. Accepts when it does not know

Class words are the records': **stated** (a comment or test says it is intended), **traced** (the reader followed it to a refusal elsewhere or showed it harmless), **open** (not traced). I have not raised any class; where my check changes the picture I say so.

Engine, embedding:
1. `TerminalEffectHandler::poll_effect_readiness` and `poll_host_computation` default to `Ok(true)` (`BE:4223-4235`). Open (076). My check: the defaults only mean "do not suspend"; the effect is then completed through the handler and its result checked by `commit_boundary_result` (`IN/calls.rs:3512-3556`). Only Build evaluation handlers override them (§10).
2. `normal_qualification_is_intrinsic` treats a membership as covered when `scalar_projection_entails_membership` says the type entails it (`BE:2300`, `:2329`). Open; traced to the helper name only.
3. `validate_input_requirements` returns Ok when `scalar_requires` is empty; non-scalar `requires` are not evaluated there (`BE:113`, `:770`). Open.
4. `initialization::validate` overlap check: an out-of-range `other_index` yields "no overlap" through `is_some_and` (`BI:3012`). Open.
5. `reference_outputs::stage` and `reference_result` skip the case-membership check when the selected source has no structural binding (`BSI:1766-1793`, `:2094-2117`). Traced in part; open for the claim that every non-structural source lacks cases.
6. `E/continuation.rs` `field`/`key` fall back to `unambiguous_field_id` when the owner is unknown (`:708-710`). Open. `covered_place`/`covered_field` accept when every declared type translates (`:346-386`). Stated.
7. `take_call_observations` returns an empty `Vec` when observations are absent or incomplete, so "no calls" and "did not finish" look the same (`P:1719-1725`). Open. `established_erased_fields` returns `&[]` on a closed instance (`P:1928-1934`). Open, low impact.
8. `continue_checked_entry` checks `call_observations` on the old frame only (`P:2344`). Open (gap_035). My check: confirmed, and `issued_layout_target` is not in the refused list either (§10).
9. `ProgramStorage::Owned` and `Fixture` report no pending normal completions (`P:2862-2873`). Open.
10. `precondition_with` skips clauses the caller says are established (`P:3142`). Stated (`P:3133-3134`); the only outside caller passes none (`build/evaluation/activation.rs:235`).
11. `resolve_entry` with no installation and no folded-constant target passes through (`P:2940`). Stated.
12. `observe_completed_entry_result` ignores nested calls and unrequested results (`BV:129-141`). Stated by test `BV:325`. `charge_completed_result` returns Ok with no account (`BV:1118-1120`). Stated by shape.
13. `EntryCallWatch::covers_pending` returns true for any group of two or more paths (`P:1515-1520`). Stated (`P:1518`) and traced to `select_entry_calls` (`P:2083-2100`).

Engine, instructions:
14. RFLAGS write of a missing or non-unsigned value does nothing and an out-of-range value saturates (`IN/execution.rs:3391-3393`). Open (checked, §10).
15. `AtomicFence` advances with no effect (`IN/execution.rs:3462-3464`). Open (single-threaded interpreter, inferred).
16. Capability walk: a `CallMachine` whose callable analysis returns `Ok(None)` is not explored (`IN/execution.rs:473-486`), and `CallDynamic` follows only the declared slot (`:505-529`). Open; the requirement list could under-report, while a missing provider still fails at run time (`IN/calls.rs:3282-3288`).
17. `terminal_scalar_belongs_to_type` returns true for every `Callable` value (`IN/execution.rs:4726`). Open (checked, §10).
18. `execute_select_implementation` writes `Unsigned(table)` into the result integer type without checking the type admits it (`IN/calls.rs:2878-2890`). Open (checked, §10).
19. Component input already forwarded as a host atom adds no carrier (`IN/calls.rs:1480-1500`). Open.
20. `execute_structural_leaf_copy` falls through from the fast path on a type mismatch (`IN/structural.rs:2033-2080`); later checks refuse unless shared-descriptor or quotient (`:2116-2127`). Open.
21. `execute_establish_byte_sequence_literal` accepts a repeated literal when identity, type, path, qualifications and bytes match (`IN/structural.rs:667-679`). Stated (test `IN/calls.rs:919`).

Engine, memory:
22. `prepare_byte_descriptor_argument` returns `Ok(None)` for borrowed storage without a ceiling, for `Owned`, for a place with no value, and when `resolve_live_projection` fails (the error is swallowed) (`M/bytes_and_views.rs:927-946`); `prepare_owned_byte_descriptor_argument` likewise (`:831-847`). Open.
23. `execute_element_view_subslice` skips the projected-qualification subset check when the view is empty (`M/bytes_and_views.rs:5019-5023`). Open.
24. `byte_field_read` returns Immutable and Representation bindings without `validate_presented_referent` (`M/bytes_and_views.rs:432-438`). Open. `PartialEq for ByteSequenceView` compares backing pointer and window, not content (`:3029-3033`). Open.
25. `require_owned_value_claims` returns Ok for non-owned arguments and when the projection leaves inline storage (`M/custody.rs:589-614`). Open.
26. `bind_entry_claims` inserts a claim with no place when no parameter matches (`M/custody.rs:2071-2080`); `transfer_claims` adds placeless callee entry claims with no caller check (`:2360-2366`). Open.
27. `consume_projected_affine_complement` returns `Ok(false)` when no frontier entry relates; a projection with matching claims but no frontier entry passes (`M/custody.rs:1491-1507`, `:1456-1458`). Open.
28. `restore_store_frontier` leaves the frontier unchanged through a `Referent`, into a non-owned place, or when `owned_window_projection` gives `None` (`M/custody.rs:93-125`, `:319-348`). Open. `stage_window_move_custody` returns claims unchanged for an unrestricted result with no overlap (`:396-404`). Open.
29. `RuntimeActivationLedger::split`/`merge` turn any ledger error into the rejected-case branch (`M/primitives.rs:100-113`). Open (fails toward the rejected branch).
30. `ScalarStorageRows::append` calls the resident-replacement check with `root = None`, which skips the "every resident key has a replacement" check (`M/primitives.rs:5669-5696`). Open.
31. `execute_primitive_store` ASCII check runs only when predicates include `ValidUtf8` and not `AsciiOnly` (`M/primitives.rs:4519-4531`). Open.
32. `capture_reference` adds each requested shared-borrow membership whose domain passes a local "observable" test without evaluating the predicate there; the contract check that follows compares against the rows just added (`M/references.rs:2765-2812`, `:2882-2906`). Open (checked, §10). The same function adds memberships the type entails (`:2829-2854`). Open.
33. `execute_access_boundary` returns `Ok(false)` when no plan names the boundary or no access reference resolves (`M/references.rs:1817-1828`). Open.
34. `forward_host_atom` returns `None` when no row exists (`M/records_and_arrays.rs:6128`). Open.
35. `X/mapping.rs:748`: `map_with_source` replaces rights with `grant.mapped_rights`, which may exceed the destination's. Open (unused in the workspace, §7). `OrderingEvent::from_admitted_provider` does not check that the named mapping exists (`X/loans_and_claims.rs:1188`). Open.
36. `ExternalLoan::complete` with an empty completion set accepts any facts (`X/loans_and_claims.rs:700`). Stated by test.
37. `stage_scalar_field` and `stage_aggregate_layout` may leave the destination partly written on error (`MAT/materialization.rs:25-50`, `:798`). Stated by test (`:358-369`).

Host and worker:
38. `has_c_scalar_storage` returns true when a field has no storage row (`HI/scalar_record.rs:249`). Open (078). My check: the native field offsets are checked separately at `ordinal * 8` (`:191-196`), so the cited wrong program is caught there (§10).
39. Filesystem service reach accepts an identity text equal to one of six legacy labels without checking the schema (`FS/bindings.rs:545`). Stated.
40. Windows handle `translate(0) = 0` and `acquire` passes 0 and -1 through (`FS/windows.rs:35`, `:43-45`). Open.
41. Private filesystem `fsync`/`fdatasync` return 0 after only checking the descriptor; `utimensat` ignores flags (`W/private_filesystem.rs:341-347`). Stated for ownership, open for the ignored flags.
42. `_setmode` stores any mode value (`HI/descriptor_mode.rs`). Open.
43. Windows CRT `read`/`write` on an unsupported descriptor return -1 instead of refusing, while the module comment says unsupported descriptors refuse before I/O (`HI/descriptor_io.rs:495-520`, `:14-22`). Open (comment and code differ).
44. `comparison_unavailable` returns true for every `MissingProvision` (`I/host/src/execution.rs:121`). Stated by test (`:411-420`).
45. Native provider service reach is skipped when a filesystem call object is present; the descriptor-I/O branch ignores its namespace admission (`HI/native_provider.rs:205-222`; `FS/bindings.rs:471-474`). Traced to descriptor I/O imports with modeled streams; stated in `HI/descriptor_io.rs:11-14`.
46. `execute_installed` skips the entry-machine check when there are no provisions (`H/hosted_invocation.rs:34-37`). Traced (inferred).
47. The worker checks an adapter digest sent in the same packet as the bytes; on Unix the parent is authenticated only by an inherited descriptor (`W/worker.rs:200`; `W/channel.rs:51`). Stated (`W/native_code.rs:77-78`).
48. `runtime_boundary.rs` dependency tests match strings; a manifest line with different spacing would not match (`I/host/src/runtime_boundary.rs:24`). Open (inferred).

## 6. Silent drops

Places where something that cannot be planned, resolved or lowered produces no row and no diagnostic:
- `caller_sites` drops a caller whose machine, block or operation is missing through `?` inside `filter_map` (`P:2218-2225`).
- `invoke_to` drops call observations on any status other than complete, unfinished or sponsor-exhausted, without recording why (`P:2461-2471`). `take_call_observations` returns an empty list for absent or incomplete observations (`P:1719-1725`).
- `continue_checked_entry` replaces the successor's execution with the old frame, so any `call_observations` (and `issued_layout_target`, my observation) on the successor are dropped (`P:2344`, `:2386-2392`).
- `contents::collect` turns any unlisted shape into an `Opaque` leaf (`M/records_and_arrays.rs:4971-4977`), and `capture` skips `Opaque`, `ByteField` and `ByteArray` leaves (`:5250-5252`). Whether the boundary caller later requires a complete roster is not traced (080).
- `bind_byte_sequence_arguments` and `bind_element_view_arguments` `continue` past borrowed byte or element-view parameters with no reference ceiling (`IN/calls.rs:276-287`, `:368-378`); the matching descriptor preparation returns `Ok(None)` for the same case (`M/bytes_and_views.rs:927-935`). Where those bindings are created is not traced (078, 079).
- `prepare_byte_descriptor_argument` swallows a `resolve_live_projection` error as `Ok(None)` (`M/bytes_and_views.rs:942-946`).
- RFLAGS writes of a missing or wrong-typed value leave the flags unchanged (`IN/execution.rs:3391-3393`).
- `representation_scalar_leaf` skips a leaf whose offset or width cannot be computed (`M/bytes_and_views.rs:5397-5408`); stated (`:5399-5400`), and a test pins that a missing descriptor still errors (`:5558-5584`).
- `ReferenceOutputs::admit` drops layouts whose codec fails (`.ok()`, `NP/reference_outputs.rs:59-66`); a later lookup refuses with "has no exact codec" (traced), so the drop is silent only at admission.
- `UnmappedExtents` returns no source for a borrowed mapping (`X/mapping.rs:872-875`); by design.
- `Sponsor::finish` with a recorded failure drops the process without `close` (`NP/original_homes.rs:152-156`); stated by test (`:1020-1030`).

## 7. Unused or unwired

Counts are the records' greps over `source-rust/` unless marked; I re-ran several over the whole worktree (§10).
- `TerminalScalarImport`, `TerminalUnitImport`: definitions and the re-export at `I/interpreter.rs:148` only; no constructor (gap_034; checked over the whole worktree, §10).
- `TerminalReturnedLoan`: definition, impl, `pub use` and re-export `I/interpreter.rs:153`; no constructor (gap_034; checked, §10).
- `TerminalExecution::start_artifact` (`I/interpreter.rs:182`): 0 callers in `source-rust/`; callers only in `_recycling/` (078; checked, §10).
- `TerminalProgram::load_envelope` (`P:3280`): one test call (`P:3952`) and `_recycling/` tests (gap_035; checked, §10).
- `precondition_with` (`P:3135`): no caller outside `P` (gap_035; checked).
- `TerminalExecution::structural_primitive_values` (`IN/execution.rs:3099`) and its delegate: 0 outside callers; `live_affine_frontier()` method (`:3121`): 0 matches (078; checked).
- `TerminalClockSnapshot::capture_host_epoch` (`CL/bindings.rs:113`): 1 occurrence (definition) in `source-rust/`; my whole-worktree grep finds callers only in `_recycling/samples/` (checked, §10).
- `IntegerAbsoluteImport` (`HI/integer_absolute.rs`): constructed once in the chain (`H/host_imports.rs:572`) but unreachable behind `AbsoluteValueImport` (`:521`) (inferred, 077; checked, §10).
- `identity_path` (`NP/service_reach.rs:18`, `pub(super)`): used only in its file (078).
- `replace_reference_address`, `require_result_captures`, `observe_result_sequence_length`: one caller each (`HI/integer_parsing.rs:261`; `IN/calls.rs:3450`; `build/evaluation/values/projection/values.rs:393`) (gap_034).
- `RuntimeRows::try_clone` (`M/primitives.rs:5058`): one non-test caller, `IN/terminators.rs:207`, suspected in 080; confirmed, because `structural_values` is a `RuntimeRows` (`IN/execution.rs:2163`) (§10).
- `complete_claims` has an unused parameter `_boundary_parameters` (`M/custody.rs:2447`) (079).
- extents: external loans (`begin_external_loan`), mappings (`map_owned`, `map_borrowed`, `stable_loan`, `range_receipt_context`) and ordering events (`admit_role_coverage`, `admit_acquisition_after_completion`) have no caller outside the extents crate (079; checked). 079's wider claim, that only `ActivationClaimLedger` is used outside the crate and that `ExtentRootGrant` mint paths and `from_admitted_provider` constructors have no non-test caller, is wrong: `HI/allocation/backing.rs:70-80` calls `ExtentRootGrant::from_admitted_provider(...).mint(...)` and `ResourceProfileGrant::from_admitted_provider`, `M/providers.rs` uses `Extent`, and `SA/src/lib.rs:7` uses `Extent`, `ExtentLoan`, `ExtentRootOrigin` (corrected, §10).
- storage-materialization: `derive_symbolic_materialization(_with_inner_layouts)`, `materialize_resolved_into`, `materialize_scalar_layout_into`, `materialize_aggregate_layout_into`, `decode_scalar_layout`, `update_scalar_field` and `apply_write` have no caller outside the crate's tests; the one other user is in `_recycling/` (079; checked over the worktree). Called from outside: `stage_scalar_field` (`M/providers.rs:1594`), `decode_aggregate_field_into` (`:1612`, `:3378`, `:3531`) and `stage_aggregate_layout` (`:4253`). 079 listed `stage_aggregate_layout` as uncalled (corrected, §10; 080 cites the call).
- Unwired in `TerminalEffectHandler`: defaults for `poll_*` are overridden only by Build evaluation handlers (`compiler/build/evaluation/host/bindings.rs:388`, `:407`; `requests.rs:344`) and forwarded by `ImportDispatch` (`P:2618-2629`); no `I/host/` adapter overrides them (my grep, §10).
- Not counted by any record: `TerminalInstance::established_erased_fields`, `external_completion`, `unbound_external_completions`, `StructuralScalarRuntimeField::case_payload`/`relocated`, several `pub(crate)` functions in `M/bytes_and_views.rs` (listed in 079), `translate_aggregate_view_field` and `backing_scalar_path` (`M/references.rs:369`, `:389`), `from_established_program_local` (`X/lib.rs:320`).

## 8. Stated limits

Host boundary and storage inputs: "does not support boundary storage loans" (`BE:4268`); "handler does not supply structural boundary results" (`BE:4299`) and "original storage homes" (`BE:4254`); case replacement "cannot yet transfer or remove opaque child storage" (`BSI:191`), "cannot establish new opaque child storage" (`BSI:202`), "lacks a semantic codec for new byte fields" (`BSI:210`); "byte-view reference outputs require an exact descriptor lifetime contract" (`BSI:2567`); "native original reference graph must have a finite acyclic declaration" (`BSI:1474`); "contents cannot establish erased bindings" (`BI:2452`).

Continuation and entry calls: "continuation retains an artifact-bound address, authority or fact" (`E/continuation.rs:134`); "requires a separate handoff" (`P:2370`); "cleanup has no authored call operation" (`P:2199`); "an unfinished program must arm every pending normal completion before running" (`IN/execution.rs:3156`); `PendingNormalCompletion` reached without a checked continuation refuses (`IN/execution.rs:3566-3569`).

Compiler evaluation: "compiler call observation requires an invocation without host imports" (`P:1706`); "compiler invocation requires its admitted artifact, entry, installation and target" (`P:3533-3539`, one message for every failure); a compiler-owned boundary may not be replaced except numeric primitives as host imports (`P:3436-3446`).

Engine operations: "Resident completion currently establishes one plain referent" (`IN/structural.rs:165-167`); "moving an aggregate storage interpretation" (`IN/structural.rs:1324`); "host reference results require an explicit captured-input interface" (`IN/calls.rs:3436`); "component contents cannot cross a reference" (`IN/calls.rs:2108-2112`); scalar entry parameters with qualifications are `ScalarEntryQualificationUnsupported` (`IN/execution.rs:2587-2593`); "owned cleanup requires its ordinary call transaction" (`M/custody.rs:722`); "erased component decomposition needs its retained custody" (`M/custody.rs:1689`).

Storage addresses and providers: "composed scalar observations have no single storage-address endpoint" (`M/primitives.rs:6144`); "storage-address exposure requires a contiguous original scalar endpoint" (`:6149`); "resident take requires partial custody transfer" (`M/providers.rs:1351-1353`); "resident scalar events require Stable observation" (`:1401-1403`); "external field write cannot synthesize an unadmitted read-patch-write" (`:1535-1537`); "host aggregate storage has no resident codec for placed cells" (`M/records_and_arrays.rs:5085`). The `ADDRESS-EXPOSURE` task covers device access and symbolic storage written after handoff (`TASKS.md:274`, crate map).

Host services: clock "CPU accounting, signals, and descriptor readiness remain unsupported" (`CL/bindings.rs:17-24`); Windows `Sleep(INFINITE)` "requires an indefinite-wait model" (`CL/windows.rs:164`); invalid `nanosleep` on Darwin "requires a hosted errno model" (`CL/timespec.rs:160`); Windows filesystem rejects pseudo-handles, async requests and pointer-valued optional parameters (`FS/windows.rs:1-4`); "The host explicitly assumes these implementations are equivalent" (`HI/descriptor_io.rs:21`); "native inet implementation is unavailable on this host" (`HI/network_address.rs:325`); private fs "blocking conflicts report EDEADLK rather than hang the runner" (`W/private_filesystem.rs:1228-1230`).

Literal limits with no named source at the site (077): worker start timeout 30 s (`NP/original_homes.rs:56`); process limits `120, 8 GiB, 64 MiB, 128, 16, 1 GiB, 2 GiB` (`sponsor.rs:139-146`); `MAXIMUM_PACKET_BYTES` 16 MiB (`W/wire.rs:6`); deadline cap 3600 s (`sponsor.rs:109`, `W/native_provider_runtime.rs:174`, `W/worker.rs:116`); shared home at most 16 MiB (`W/native_provider_runtime.rs:306`); reclaim thresholds 1024 and 4096 (`M/records_and_arrays.rs:6160`, `:6184`, `:6248`); depth 128 for provider images and reference fragments (`M/providers.rs:3457`, `M/references.rs:261`). The private filesystem constants are documented as modeled kernel values (`W/private_filesystem.rs:12-18`, `:68-73`). Materialization's record path depth 64 is the compiler's bound (`MAT/symbolic_materialization.rs:65`, tests `MAT/contract_tests.rs:1996`).

Tests: one ignored test needs a real CLI artifact through `OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT` (`P:3969`); physical worker tests need an installed C compiler (`W/physical_tests/native_calls.rs`).

## 9. Disagreements between records, and with the crate map

- **extents use outside the crate.** 079 says only `ActivationClaimLedger` is used outside, and that `ExtentRootGrant` mint paths and `from_admitted_provider` constructors have no non-test caller. 077 describes `AllocationStorage::resolve_range` minting `ExtentRootGrant`/`ResourceProfileGrant` (`HI/allocation/backing.rs:40`), and 080 and 081 describe `M/providers.rs` and `SA/` using `Extent`. The code supports 077/080/081 (§10).
- **`stage_aggregate_layout` callers.** 079 lists it among functions with no caller outside the crate; 080 cites `M/providers.rs:4253`. The code has the call (§10). 079's grep ran over the primary checkout, not the worktree (079 says so).
- **sponsor.rs size and entry line.** The crate map says `sponsor.rs` is "about 490 lines" and puts `for_adapters` at `:165` and matching at `:164-232`; 077 reads 349 lines with `for_adapters` at `:22` and `lookup_adapter` at `:65`. The file has 349 lines and `for_adapters` at `:22` (§10).
- **runtime_boundary.rs line.** The crate map cites `runtime_boundary.rs:72-78` for the `target-vocabulary` rule; the file has 38 lines and the rule is at `:25-31` (078; §10).
- **`bind_numeric_import_slot` line.** Crate map `~35`; 081 and the code `:29`.
- **`case_projection_with_membership_is_active` location.** gap_034 places it at `BI:1726` (grepped, not read); 076 gives `1726` as the module start; the function is at `BI:2267` (my grep).
- **`matches_platform_c_symbol` users.** 077 says seven adapters use it; my grep finds 6 call sites in `I/host/`.
- **Linux libraries for `matches_platform_c_symbol`.** 078 is unsure which Linux library it accepts because `HI/network_address.rs:116-121` passes only macOS and Windows names; 077 read the function: Linux always uses versioned `libc.so.6` (`H/host_imports.rs:236-247`).
- **`poll_*` overrides.** 076 is unsure whether real handlers override the `Ok(true)` defaults; my grep shows only Build evaluation handlers do (§7).
- **Materialization test counts.** 079 counted 40 tests from a capped list; gap_036 counts 41 in `MAT/contract_tests.rs`; the crate map's 46 for the crate equals 41 plus 5 in `MAT/materialization.rs` (my count). Consistent once read.
- **Engine location in comments.** `I/interpreter.rs:12-19`, `I/host/src/lib.rs:17` and `I/host/README.md` say the engine lives in `psi/`; it is `I/` (crate map, 078).
- **Borrowed byte parameters without a ceiling.** 078 (`IN/calls.rs:276-287`) and 079 (`M/bytes_and_views.rs:927-935`) report the same skip from the two sides; neither traced where those bindings come from.

## 10. What I checked myself

1. **Claim (077, inferred):** `IntegerAbsoluteImport` is unreachable because `AbsoluteValueImport` runs first and claims the same locators. **Code:** the chain calls `AbsoluteValueImport::admit` at `H/host_imports.rs:521` and `IntegerAbsoluteImport::admit` at `:572`. `absolute_value::matches_locator` accepts msvcrt, glibc 2.2.5/2.17 and both libSystem names for Linux, macOS and Windows (`HI/absolute_value.rs:221-267`); `integer_absolute` calls `matches_platform_c_symbol` with the same libraries plus a host-target check (`HI/integer_absolute.rs:32-42`; `H/host_imports.rs:215-270`). After a locator match, `AbsoluteValueImport::admit` returns `Some` or `Err`, never `Ok(None)` (`HI/absolute_value.rs:37-122`). **Matched.**
2. **Claim (077):** `capture_host_epoch` has 0 callers. **Code:** `git grep` over the worktree finds the definition (`CL/bindings.rs:113`) and eight callers, all under `_recycling/samples/`. **Matched** (none in active source).
3. **Claim (gap_034, suspected):** `TerminalScalarImport`, `TerminalUnitImport` and `TerminalReturnedLoan` are never constructed. **Code:** whole-worktree grep finds only definitions and impls in `P` (`:101-211`, `:1375-1427`) and re-exports at `I/interpreter.rs:148`, `:153`. **Matched.**
4. **Claim (078, gap_035):** `start_artifact`, `load_envelope` and `precondition_with` have no outside callers. **Code:** `start_artifact` callers only in `_recycling/`; `load_envelope` only in `_recycling/` tests besides `P`; `precondition_with` none outside `P`. **Matched.**
5. **Claim (078):** `structural_primitive_values` and `live_affine_frontier()` have 0 callers. **Code:** `IN/execution.rs:3099-3123`; no `.structural_primitive_values(` or `.live_affine_frontier()` call anywhere. **Matched.**
6. **Claim (079):** in extents, only `ActivationClaimLedger` is used outside the crate; root-grant mint paths and `from_admitted_provider` have no non-test caller. **Code:** `HI/allocation/backing.rs:18`, `:70-80` imports and calls `ExtentRootGrant::from_admitted_provider(...).mint(...)` in production; `M/providers.rs:16` uses `Extent`; `SA/src/lib.rs:7` uses `Extent`, `ExtentLoan`, `ExtentRootOrigin`. External loans, mappings and ordering events do have no outside caller (only an unrelated `OrderingEventId` in `compiler/psi/access/extents`). **Partly right.** Corrected in §7 and §9.
7. **Claim (079):** of storage-materialization, only `stage_scalar_field` and `decode_aggregate_field_into` are called from outside; `stage_aggregate_layout` is test-only. **Code:** `M/providers.rs:4253` calls `storage_materialization::stage_aggregate_layout`. The other "test-only" names have 0 outside matches in active source. **Partly right.** Corrected in §7 and §9.
8. **Claim (076, open fail-open):** default `poll_effect_readiness`/`poll_host_computation` return `Ok(true)`, so a handler that does not override reports ready. **Code:** defaults at `BE:4223-4235`. `boundary_effect_wait` uses them only to decide whether to suspend (`IN/calls.rs:3512-3556`); the effect is then completed by `complete_boundary_effect` and the handler's result is still checked. Overrides exist only in `compiler/build/evaluation/host/bindings.rs:388`, `:407` and `requests.rs:344`. **Partly right:** the default is as stated, but it decides suspension, not acceptance of a result.
9. **Claim (078, open):** `has_c_scalar_storage` returning true for a missing storage row lets a record whose real position differs from `ordinal * 8` through. **Code:** `HI/scalar_record.rs:242-249` does return true for `None`; but the native field graph is checked separately for offset `ordinal * 8`, size 8 and alignment 8 (`:186-196`) and the call passes `admit_exact_foreign_call` (`:218`). **Partly right:** the shortcut exists; the cited wrong program is caught at the native geometry check.
10. **Claim (081, open):** `capture_reference` adds shared-borrow memberships without evaluating their predicates, and the check that follows compares against those added rows. **Code:** `M/references.rs:2765-2812` pushes every requested domain that passes `observable` (no content projection, no establishment routes, closed or byte predicate); `:2882-2906` then checks `required_projected` against `projected` built from the same rows. **Matched** (whether later reads re-evaluate those predicates is not traced).
11. **Claim (078, open):** `execute_select_implementation` writes `Unsigned(table)` without checking the result type admits it. **Code:** `IN/calls.rs:2871-2891` checks only that the result is an integer type and the table exists, then inserts. **Matched.**
12. **Claim (078, open):** an RFLAGS write of a missing or wrong-typed value is silently accepted. **Code:** `IN/execution.rs:3391-3393`, `if let Some(Integer{Unsigned(bits)})` with no else, then `Advance`. **Matched.**
13. **Claim (078):** in `install_return`, the second `NoNormal`/`ExternalCompletion` arms are unreachable. **Code:** early returns at `IN/terminators.rs:1699-1710`, same arms again at `:1716-1727`. **Matched.**
14. **Claim (gap_035, open):** `continue_checked_entry` checks `call_observations` on the old frame only. **Code:** `P:2344` checks `frame.call_observations` only; `entry` is then discarded (`P:2386-2392`). The successor can carry observations because `install_compiler_observations` needs only an unopened execution (`P:1699-1716`) and the check at `P:2296-2302` refuses only an invoked successor. `issued_layout_target`, set by the same function, is in neither side's list. **Matched**, with that addition.
15. **Claim (078, open):** the capability walk follows only the declared slot of `CallDynamic`, while the runtime may choose a later slot. **Code:** walk `IN/execution.rs:505-529` reads `table.rows.get(slot_index)`; runtime `IN/calls.rs:2975-3013` scans later slots for an equal family tuple. **Matched.**
16. **Claim (080, suspected):** `RuntimeRows::try_clone` has one non-test caller at `IN/terminators.rs:207`. **Code:** `execution.structural_values.try_clone()?` at `:207`, and `structural_values` is `RuntimeRows<PlaceId, TerminalStructuralValue>` (`IN/execution.rs:2163`). **Matched.**
17. **Claim (077):** guest descriptor numbering differs: the libc path refuses 0..=2 while the Linux kernel table hands out 0. **Code:** `FS/bindings.rs:790-794` refuses 0..=2; `W/host_filesystem.rs:179-181` takes the lowest number from 0 that is not a modeled stream and not in use. **Matched.**
18. **Claim (crate map):** `sponsor.rs` is about 490 lines; `for_adapters` at `:165`, matching at `:164-232`. **Code:** `wc -l` gives 349; `for_adapters` is at `sponsor.rs:22`. **Wrong.** Corrected in §1 and §9.
19. **Claim (crate map):** the `target-vocabulary` dependency rule is enforced at `runtime_boundary.rs:72-78`. **Code:** the file has 38 lines; the rule is at `:28` (within 078's `:25-31`). **Wrong** (line citation). Corrected in §9.

Totals: 13 matched, 4 partly right, 2 wrong.
