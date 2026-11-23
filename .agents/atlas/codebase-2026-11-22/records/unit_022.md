# Record: unit_022 (area 11-native-stages-11-15), image-emission `function_fragments`

Paths are under `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`.
THIS RECORD IS PARTIAL. `function_fragments.rs` was read in full only for lines 1-7800 of 21313. Lines
7801-21313 were not read (ran out of context). The three small files were read in full.

## 1. `function_fragments.rs` lines 1-7800

### Purpose
The file is the image-emission crate's "prepare the issued body for image composition" module. Its single
public entry `prepare_object_for_image` (`function_fragments.rs:2769`) takes a `PublishedFunctionObject` from stage 14
and builds an `ObjectArtifact`: per-function stack and call-site records, semantic code attribution rows, port
effects, boundary settlements, and pool records (byte literals, code descriptors, native code layouts, image
residence). The rest of the lines read are inline modules that re-check ("validate") that those pools, import
rows and attribution rows still equal what the producing stage would rebuild. The crate map (`crate-map/11-...md:371-373`)
already notes that the module tree is inline in this file; I confirm this in the module list below.

### Inline module map (lines read), with `file:line`
- `mod attribution` (`:8-1514`): semantic code attribution. `produce_current` (`:143`) turns a `FunctionFragment`
  into `SemanticCodeAttribution` rows (site, operation_ordinal, code_offset, byte_count). `validate_current` (`:309`)
  rebuilds the exact-interval check. `ordinal` (`:133`) looks up an authored ordinal in
  `target_vocabulary::NativeFunctionPublication`. Helpers: `PhysicalSpans` (`:509`, canonical sorted spans with
  `possible_sites` index), `AttributionRows` (`:485`), `overlap_range` (`:639`), `supports` (`:650`),
  `interval_is_exact` (`:704`, the older full-scan version). About 700 lines of `#[cfg(test)]` (`:785-1513`).
- `mod byte_literals` (`:1516-1568`): `validate` (`:1522`) compares artifact pool, pool symbol and relocation rows
  to `pool_contents` rebuilt from the publication.
- `mod image_addresses` (`:1570-1665`): one function `rows_for_symbols` (`:1572`) selecting relocation records whose
  `symbol_handle` or `RelocationOrigin::Materialization` names a given symbol. One test.
- `mod image_residence` (`:1667-1714`): `validate` (`:1673`) checks the retained-entry-storage symbol is undefined
  before placement and the relocation rows equal `records(...)`.
- `mod code_descriptors` (`:1716-2047`): `validate` (`:1727`), `validate_image_data_relocations` (`:1829`),
  `validate_data_relocation_roster` (`:1890`). Rebuilds the descriptor pool with `place` into a scratch `ObjectPlan`.
- `mod imports` (`:2049-2197`): `validate` (`:2060`) checks the normalized-import roster, import symbols and callback
  private-function references against `object_file::body_object::imports::readback`.
- `mod mixed_scalar_abi` (`:2199-2258`): `admit` (`:2205`), a field-by-field equality check of the mixed scalar and
  structural ABI against abstract, target and selected function.
- `mod native_code_layouts` (`:2260-2404`): `validate` (`:2268`), `image_data_records` (`:2362`).
- `mod port_effects` (`:2406-2674`): `produce` (`:2413`) and `append_function_effects` (`:2438`) join physical
  port-write facts to `PortWrite` instruction spans.
- `mod private_addresses` (`:2676-2753`): `validate` (`:2685`), `validate_symbols` (`:2701`), `validate_relocation_tail` (`:2733`).
- `mod production` (`:2755-2972`): the body of `prepare_object_for_image` and `ProjectedObject(())` (`:2764`, an
  unforgeable marker field `projected` on `ObjectArtifact`).
- `pub(crate) mod replay` (`:2974-3068`): `FragmentReplay` (`:2991`, holds
  `Arc<StagedOptimizedRelocationFreeObjectContainer>`), `attach_to` (`:2997`), `ObjectArtifact::attach_issued_replay`
  (`:3036`), `validate` (`:3062`) forwarding to `validation::validate_available` (not read).
- `mod source` (`:3070-...`, extends past 7800): contains `descriptor_tables` (`:3078`), `installed_calls` (`:3689`),
  `installed_interrupts` (`:3933-6804`, with submodules `guards`, `inputs`, `instructions`, `pairs`,
  `publication`, `refusal_result`, `table`, `writes`), `static_interrupts` (`:6806-7088`), `selected_constant_operand`
  (`:7090`), `correspondence` (`:7121-7658`), and `aggregate_results` (`:7660-...`, continues past 7800).

### Inputs and outputs
- `prepare_object_for_image(object: PublishedFunctionObject) -> Result<ObjectArtifact, Error>` (`:2769`).
  Reads: `object.into_parts()` (publication, `FunctionObjectSections`, replay); `ObjectEmissionInput::from_publication`;
  `source::*` accessors (fragments, frame, function facts, ABI, temporary stack, stack calls, callable invocations);
  `attribution::produce_current`; `structural::{settlement_attributions, populate, settlements, requires_call_graph_replay}`
  (modules not read); `image_data::ObjectImageData::prepare`; `port_effects::produce`.
  Produces: `ObjectArtifact` with `functions: Vec<ObjectFunction>`, `semantic_code_attribution`, `port_effects`,
  `boundary_settlements`, pools passed through unchanged from stage 14 (`:2961-2963`).
- Per function it fills `ObjectFunction` (`:2854-2898`). Most record vectors are created EMPTY here (for example
  `internal_unit_calls`, `unit_scalar_homes`, `unit_parameters`, `unit_continuations`, `structural_return: None`
  at `:2880-2897`) and are filled afterwards by `structural::populate` (`:2908`, not read). So the rows that stay
  empty depend on code outside what I read.
- Stack math (`:2828-2853`): `local_peak = frame_bytes + temporary_stack`; for each call
  `caller_live_bytes = frame_bytes + linkage`, with `linkage` set by match on `text.target.architecture`:
  x86-64 is 8, AArch64 is 0 (`:2813-2816`). Unit functions get `ObjectUnitCallStack`, others `ObjectScalarCallStack`
  (`:2836-2852`). Default alignment 16 when the function has no frame row (`:2812`).

### How it decides (keys)
- Attribution is keyed on `SemanticCodeSite` (an `Operation(OperationId)` or `Edge(EdgeId)` identity) and the
  authored ordinal from the publication (`:133-141`); identity, not spelling. Rows are sorted by
  `(operation_ordinal, code_offset, byte_count)` and adjacent same-site rows are merged only if contiguous
  (`:265-291`); overlap is an error (`:281`).
- `supports` (`:650`) decides if an instruction may own a site: operation sites need the instruction's
  `provenance.operations` to contain the id; edge sites match on the instruction's `control` variant and, for
  conditional fallthrough, a zero-length row at `branch.when_fallthrough_offset` (`:684-691`).
- `implementation_successor` (`:27`) accepts a non-semantic successor only for `CaseDispatchContinuation` with a
  `psi_edge`, or `OperationContinuation` owned by an operation in the instruction's provenance with no edges/work.
- Import validation matches roster rows by locator equality and symbol-name equality
  `symbol.name != normalized_import_symbol_name(locator)` (`:2138`) and counts symbols with the same `name`
  (`:2144-2149`): this is a string comparison on symbol names.
- `private_addresses::validate_symbols` compares `object_file::private_function_symbol_name(*identity)` to
  `symbol.name` (`:2717-2718`): identity-derived name compared by string.
- Descriptor-table checks (`source::descriptor_tables`) key on `OperationId` and virtual-register ids; memory
  footprint roles are compared as sets (`validate_memory`, `:3636`).
- Installed-interrupt validation (`:6069-6428`) keys on `(machine, psi_operation, placement_index)` and
  admitted-cohort rows; entries sorted by `placement.application_index` (`:6111`).

### Cases by shape (fixed lists that refuse the rest)
- Operand roles for descriptor hardware ops are fixed per `RegisterOperation` (`:3452-3508`): `SegmentRead` and
  `TaskRegisterRead` (result only), `SegmentReload` (operands `[0,2]`, selector 1, extent 3, result 4),
  `TaskRegisterActivate` (`[0,1,3]`, selector 2, extent 4, result 5), `GlobalDescriptorTable::{Load,Store}`
  (`[0]`). Anything else returns "unexpected descriptor hardware operation" (`:3507`).
- Byte counts for descriptor memory: `entries * 8`, `104` for task state in activation, otherwise `10`
  (`:3528-3537`), hard-coded.
- Installation call roster is fixed to exactly two calls per `InstallStaticInterrupt`: position 0 is `Validate`
  (Boolean result, argument index 0), later positions are `Work` (argument index 5) (`:3841-3851`); a
  "validation call and work call" pair (comment text "exactly its validation and work calls", `:3855`).
- Installation control allocation must be 128 bytes with alignment 16 (`:5064`); gate counter at home+64,
  arrival 72, stack 80, count 88, root 96, captured range 112 and 120, scratch 48 (`:4055-4072`, `:5297-5301`,
  `:6499-6543`). Fixed control-word offsets.
- Counted instruction roster for an installation: exactly 1 validation call, 1 work, 1 CPU check, 1 task commit,
  1 table publication (`:4593-4598`); instruction kinds outside the listed allow-list are refused with
  "unaccounted instruction or side effect" (`:4651-4655`).
- Footprint counts: 2 for IDT publication, 4 for task commit, 3 for CPU check (`:5414-5421`).
- Gate bit-placement: the installed address must be fully covered by `Bits` fragments over 64 source bits, with
  hardware bit mapping `source < 16 ? source : source + 32` and gate base a multiple of 128 bits (`:5628-5692`).
- Acknowledgement entries must be `NoErrorCode` frame and gate number in 32..=255 (`:4211-4219`); stack alignment must be a power of two and at least 16
  (`:6165-6166`).
- `static_interrupts::root_accesses` (`:6974`) allows reading fixed root-word offset sets per operation kind
  (`Context`, `BeginCompletion`, `FinishCompletion`); `Install` returns false (`:7037`).
- `correspondence::validate_block` (`:7135`) requires the block's cleanup calls to be a suffix of `target.operations`
  after the first `CleanupCall` (`:7145-7150`), and every body row to carry a `source`.
- `aggregate_results::instruction_operation` (`:7670`) lists 18 `TargetUnitOperation` variants by name; others
  return `None`.

### Same job elsewhere
- Fragment-vs-source interval checking exists twice inside `attribution`: `PhysicalSpans::interval_is_exact`
  (`:591`) and `interval_is_exact` (`:704`). The comment at `:514-515` says the second is kept for "other input
  shapes" and "its error order". Both are called from `validate_with_ordinals` (`:330-333`).
- The "operation-trap / jump / conditional / call successor" control matching is written three times in
  `attribution`: `produce_with_ordinals` (`:170-263`), `validate_with_ordinals` (`:372-440`),
  `PhysicalSpans::prepare` (`:536-577`) plus `supports` (`:662-701`). Same `Control` variants, four matches.
- "Unique row or error" helper exists as `unique` (`:6052`), `unique_index`/`find` (`:3713-3729`), and inline
  `carriers` iterators (`:2168-2172`). Grepped `fn unique` across `source-rust/` was NOT done; suspected duplicates elsewhere.
- The pool validators (`byte_literals::validate`, `code_descriptors::validate`, `native_code_layouts::validate`,
  `image_residence::validate`) each rebuild the pool and compare with `rows_for_symbols` (8 occurrences of the
  name, all in this file, from `grep -c` count mode).
- `descriptor_tables::{source_operation,target_operation}` (`:3101`, `:3193`) mirror each other over the same five variants.
- The installed-interrupt guard builder and the verifier of those guards live partly in
  `installation_admission::selected` (imported at `:3935`, `:3946`, `:6050`); that crate was not read (suspected shared logic).

### Unused or one-caller code (greps)
- `prepare_object_for_image` (pub): `Grep` count mode over `source-rust/`: 6 occurrences in 4 files (this file 2,
  `lib.rs` 1, `native-artifact/src/fragment_object_publication.rs` 1, `psi-to-native/tests/physical_publication/mod.rs` 2).
  So production callers: 1 (native-artifact, matching the crate map).
- `selected_constant_operand`: 8 occurrences in 5 files (this file 4, `entries.rs` 1, `entry-planning/src/body_cpu.rs` 1,
  `installation-admission/src/selected/{cpu_origins.rs,mod.rs}` 1 each). The function at `:7090` is `pub(crate)`, a
  forwarder to `installed_interrupts::selected_constant_operand`, itself a re-export of `installation_admission::selected_constant_operand` (`:6050`).
- `ObjectArtifact` impl `#[cfg(any(test, feature = "test-support"))] impl crate::ObjectArtifact {}` at `:3059-3060` is an empty impl block (read).
- `build_ordinal_index` and `test_publication` (`:47`, `:98`) are `#[cfg(test)]` only (read).

### Stated limits
- `:7096-7099` `original physical instructions were not retained (ReplayUnavailable)`: the constant-operand
  lookup refuses without a retained original encoding.
- `:4465-4467` "installation view derivation requires complete selected replay": view derivation refuses unless
  the view is `Established` with `Fixed` extent and `WholeCollection` region.
- `:2766-2768` docs: "Explicit audits retain their original stage inputs alongside this view."
- `:514-515` "Other input shapes retain the original full scan and its error order."

### Fail-open risks
- `PhysicalSpans::prepare` returns `None` for non-canonical input (`:517-525`) and the validator then falls back to
  the slow scan (`:330-333`); it does not accept, it changes the algorithm. Classification: stated (`:514-515`).
- `source::installed_calls::PreparedCalls::new` returns an empty prepared set when `targets.is_empty()` (`:3760`);
  an installation call in `selected.calls` with no target op is rejected earlier (`:3755-3758`), so a program with
  no installation ops is simply not checked here. Traced within the file: fine, but whether another stage checks
  helper calls for non-installation owners is not seen. Classification: open.
- `descriptor_tables::DescriptorReplay::new` returns an early `Ok` with empty `definitions` and `accesses` when
  `operations.is_empty()` (`:3369-3377`); hardware descriptor instructions in `selected` whose operation is
  neither a descriptor op nor an installation op are rejected at `:3359`/`:3630-3633`. Traced: no accept path
  for an unowned hardware instruction. Classification: traced.
- `installed_interrupts::publication::reads`: a load whose address is in `inputs`, in the private control words,
  or `within` the table is skipped without a guard check (`:5291-5311`). Which wrong program passes: a read of
  another word inside the 16-byte-stride table interval is accepted because the table interval `[table, table_bytes)` is
  exempt. Classification: open (no comment says why the whole table is exempt; the table reads are separately
  covered by the `ReadKind::Table => {}` arm at `:5368`).
- `fresh_observations` exempts any load whose address equals one of the operand pointer loads
  (`:5522-5534`). Classification: stated (the exemption list is the operand roster, `:5492-5498`).
- `port_effects::append_function_effects`: `effects.len() - start != spans.len()` check (`:2500`) means PortWrite
  spans with no physical origin are refused, not accepted. None seen.
- `byte_literals::validate` accepts `(None, None)` when `artifact.text_bytes.len() == pool_text_offset`
  (`:1528-1534`). A program with no literals passes only if no trailing bytes exist; traced, none.
- None other seen in lines 1-7800.

## 2. `function_fragments/current_image_selections.rs` (199 lines, read in full)
- Purpose: wraps `machine_code::image_origins::ImageAddressOrigins` as `CurrentImageSelections`
  (`current_image_selections.rs:8`) so image-address rows can be checked against carried physical origins without
  reopening instruction graphs (doc comment `:1-2`).
- Shape: `prepare_for(addresses, origins)` (`:11`) returns `Ok(None)` unless some address has a reference that
  `ImageAddressOrigins::indexes(address.reference)` accepts (`:15-20`); otherwise clones the origins or maps the
  `&'static str` error to `Error::Mismatch`. `require(address, result)` (`:26`) delegates to `ImageAddressOrigins::require`.
- Decision key: the `SelectedImageAddressReference` kind via `indexes`; the check itself is in `machine_code` (not read).
- Callers: `ObjectImageData::prepare` (`image_data.rs:22`), and `code_descriptors::validate` / `native_code_layouts::validate`
  take `Option<&CurrentImageSelections>` (`function_fragments.rs:1729`, `:2270`).
- Fail-open: when no address needs an index, `prepare_for` returns `None` (`:15-20`) and `code_descriptors::validate`
  only demands `Some` when `!pool.addresses.is_empty()` (`function_fragments.rs:1765-1768`). Traced: acceptable
  because `indexes` is the same predicate that would require a selection. Classification: traced.
- 140 of the 199 lines are one test with a hand-built fixture (`:35-198`).

## 3. `function_fragments/image_data.rs` (36 lines, read in full)
- Purpose: `ObjectImageData` (`image_data.rs:11`) with fields `addresses: Vec<PlacedImageAddress>`,
  `selections: Option<CurrentImageSelections>`, `native_layouts: Arc<[NativeCodeLayout]>`, built once by
  `prepare` (`:18`) from the unresolved image addresses and `publication().native_code_layouts()`.
- Inputs: `ObjectEmissionInput` `text_section().unresolved_image_addresses`, `emission_facts().physical.image_origins`.
- Decision: none beyond propagating errors (`Error::NativeCodeLayout`). No fail-open seen.
- Called from `production` at `function_fragments.rs:2925`.

## 4. `function_fragments/internal_calls.rs` (326 lines, read in full)
- Purpose: joins the closed internal-call plans (`machine_code::internal_call_plans`) to the instruction spans in
  a function's fragment, producing `InternalUnitCallRecord` values with code offsets and the argument bytes.
- Shape: `error` (`:11`) maps `InternalCallPlanError` to `Error`; `plan` (`:19`) and `publication` (`:31`) read
  `source.emission_facts().physical.internal_calls`; `instruction` (`:41`) finds exactly one span for an id;
  `argument_extent` (`:55`) computes `(start, length)` of an argument either for a call (`PhysicalArgumentSpan::Call`)
  or for a `Copy` (extent + transfers, both must be disjoint and coincide at the ends, `:114`); `place_calls` (`:120`).
- Decision keys: call-site owner is matched by `CallSiteOwner::{Operation, InvokeNoNormal, CleanupAction}`
  -> `SemanticCodeSite` (`:130-139`); any other owner returns "Unit call has no attributed span" (`:138`). The
  attribution row is found by `rows.iter().find(|row| row.site == site)` (`:140-143`), the call span by instruction
  id, the fixup check is `fixup.callee != plan.target` (`:145-149`).
- Cases by shape: only three owner kinds attributed (`:130-138`); error text "Unit call" in names suggests unit-result
  functions but the code does not check the result kind here.
- Unused code: `place_calls` callers: not grepped by me in this pass (called from `structural` module per
  `super::structural::fragment` use at `:126`; the caller itself is in unread lines). Marked "suspected".
- Fail-open: `argument_extent` with a non-`Copy` plan returns the call span after checking
  `fragment.bytes.get(start..end) == Some(call.bytes)` (`:61-68`); no accept-on-missing path seen. None seen.
- Tests: 2 at `:268`, `:290` (mutation loop of 6 mutations refusing missing, duplicate, overlapping, empty transfers).

## Coverage
- `function_fragments.rs`: read in full lines 1-7800 (7800 lines). Skimmed: 0. NOT read: lines 7801-21313
  (13513 lines), which include the rest of `aggregate_results`, and every module after it in the file
  (`structural`, `structural_homes`, `placed_reference`, `validation`, tests, and others I have not seen the names of).
- `function_fragments/current_image_selections.rs`: 1-199 read in full.
- `function_fragments/image_data.rs`: 1-36 read in full.
- `function_fragments/internal_calls.rs`: 1-326 read in full.
- Total read in full: 8361 of 21874 lines in this unit (the unit has 21313+199+36+326 = 21874 lines).
- Gaps in the notes above: no counts for pub function users beyond the three greps shown (commands:
  Grep count mode for `prepare_object_for_image`, `selected_constant_operand`, `fn rows_for_symbols|rows_for_symbols\(`
  over `source-rust/`).

## Five things a maintainer most needs to know about this unit (lines read)
1. `prepare_object_for_image` (`function_fragments.rs:2769`) builds most `ObjectFunction` vector fields EMPTY
   (`:2880-2897`) and relies on `structural::populate` (`:2908`) to fill them; reading only this function gives the wrong picture of what an `ObjectArtifact` contains.
2. Most of the file is verification that rebuilds a pool, import roster or attribution table and compares it with
   the artifact (`byte_literals`, `code_descriptors`, `native_code_layouts`, `imports`, `private_addresses`, `attribution`). This is validation at the trust boundary, not production.
3. About 3,000 lines (`:3933-6804`, `installed_interrupts`) re-derive a symbolic model of the installed-interrupt
   installation sequence (expression trees, loop counters at fixed home offsets 64/72/80/88/96/112/120, root
   words) and check the selected instructions against it. The offsets and byte counts are hard-coded constants in
   that module (`:4055-4072`, `:6499-6543`).
4. `attribution` has two interval-exactness implementations (`:591`, `:704`) and four copies of the `Control`
   variant match; the older full-scan path is kept as a fallback for non-canonical input (`:514-515`).
5. Symbol-name string comparisons appear in import and private-address validation (`:2138`, `:2144-2149`, `:2717-2718`, `:1548`), on names built by `object_file` helpers, not on identities alone.

## What I am unsure about
- Everything in lines 7801-21313 of `function_fragments.rs`: not read, nothing claimed.
- Whether `structural::populate` and `structural::settlements` (unread) fill every vector left empty at `:2880-2897`.
- Whether `installation_admission::selected` (imported at `:3935`) duplicates the symbolic model built here.
- I did not grep `place_calls`, `unique`, `unique_index` for users; those are "suspected" only.
