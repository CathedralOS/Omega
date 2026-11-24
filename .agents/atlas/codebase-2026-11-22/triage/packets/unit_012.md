# Triage packet: unit_012 (area 10, stage 05 instruction selection)

Packet complete.

None of the 13 correctness claims in the unit_012 record shows a wrong result
in the current code. Nine claims are intended behavior: each one matches the
Psi reference check, the spec or a later stage that refuses the bad case. One
claim (borrowed byte-descriptor access) was already repaired in #5379. Three
claims remain open and are listed individually under
[Unresolved claims](#unresolved-claims). The review changed no stage 05 code.

While running the stage 05 fixtures, I found that `main` no longer compiled
stage 05 or anything built on it. #5393 added `Proposition::DeclarationIdentity`,
but two native crates had exhaustive matches without an arm for it. This branch
adds both arms, each with its own fixture; see
[Build break found during validation](#build-break-found-during-validation).

The unit covers five stage 05 files under
`source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/`,
abbreviated `S05/` below. `SG` is `S05/selection_scalar_graph.rs`. No
second-pass gap range in `second_pass_ranges.tsv` covers these files, so this
packet owns the whole unit. Line numbers in the "Original location" column are
the record's and the digest's, at survey commit `4f3c255d`. The "Code path"
column names the function and its current line where it moved.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. Code was read at
`a0152d26` (two commits later; neither touches stage 05). Open PRs when the
review started: #5409, #5405, #5368, #5338, #5336, #5307. None of them changes
these files, and no claim in `cargo xtask claims` covered them.

## Correctness claims

| # | Original location | Code path | Contract | Evidence | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Record §1 "Fail-open risks", `:970`; digest line 166 | `S05/selection_access_lifecycles.rs` `resource_profile::compatibility::run::assess`, `u64::MAX.checked_div(footprint)` | [Typed runs](../../../../../wiki/spec/resources/typed_runs.md): zero-footprint cells have an explicit count and occupy no bytes. The Psi reference `validate_run_relocation_resources` (`psi/access/validated-placements/resources.rs:709`) checks only cell 0 when the footprint is zero. | With footprint 0 the emitted check still requires `count * 0 == length`, so the length must be 0. It then checks cell 0 only, as the reference does. No byte of a zero-size cell is read or written, so an unbounded count cannot reach memory. | Intended |
| 2 | Record §1 "Fail-open risks", `:1003-1004` | Same function: `if matches!(role, SupplyRole::Source) { return Ok(()) }` before the per-cell `check_cell` calls | Typed runs: relocation moves a `Live` run into compatible vacant backing. The reference checks the source with `require_stable_region_supply(read)` over the occupied range and runs per-cell `validate_initialization_resources` only on the destination. | The emitted code does the same. The source gets the Stable read coverage pass; per-cell placement and alignment checks run only for `Destination`. The source cells' placement was established when the run became `Live`. | Intended |
| 3 | Record §1 "Fail-open risks", `:1803`; digest line 166 | `S05/selection_access_lifecycles.rs` `assess_cell`, `if operation != View` around the whole-extent `extend_coverage` | [Access policies](../../../../../wiki/spec/resources/access.md#establishment-and-retirement): View interprets established content; Initialize needs exclusive Stable storage. The reference splits the same way: `validate_access_resources` (used for views, for example `psi/semantics/src/access/access_ram_correspondence.rs:210`) checks each field descriptor and region, and only `validate_initialization_resources` adds the whole-image Stable supply. | For View the emitted code still checks each field descriptor against one covering region, its reach and its alignment, and checks each field region under its own observation model. Requiring Stable coverage of the whole extent would wrongly refuse External or Atomic views. | Intended |
| 4 | Record §1 "Fail-open risks", `:6774`; digest line 166 | `S05/selection_access_lifecycles.rs` `ScalarImageCodec::field` and `read` (`:6760-6930`) | The owned image must round-trip the field value through its access placement. | `signed` only changes `read` when the stored width is under 8 bytes, where it sign-extends. The owned width differs from the access width only when authored storage gives a narrower `stored_scalar`, and the code takes the sign from that stored scalar. With no authored storage the owned width equals the access width (`ScalarStorageCodec::from_scalar`), so extension cannot change the stored bytes. The access side takes its sign from `IntegerAt.interpretation`. Fixture `resident_authored_value_codec_keeps_signed_fields_fragments_and_array_strides` covers signed stored fields. | Intended |
| 5 | Record §1 "How it decides", `:6528-6538`; record "What you are unsure about"; digest §10 item 15 (line 276) | `S05/selection_access_lifecycles.rs` `transfer_image` | Join each schema field to its placement in one validated placement. | The schema field is found by id (`field.id == *identity`). The access slot and the layout entries are joined by `entry.field == slot.field()`, which compares two names from the same `ValidatedPlacement`. A missing name leaves `placements` empty, and `ScalarImageCodec::placement` or `aggregate_chunks` then refuses ("resident scalar has no image placement"). Several entries with one name are the repeated-element case `aggregate_chunks` handles. | Intended; matches digest §10 item 15 ("partly right") |
| 6 | Record §2 "Fail-open risks", `:386-391`; digest line 165 | `S05/selection_call_abi.rs` `borrowed_argument::validate_borrowed_argument`, `!parameters.is_empty() && !accepts_graph(..)` | The enclosing function's call plan must match its structural parameters. | `parameters` are the selected function's own structural parameters, not the callee's. `structural::entry` (`S05/selection_structural.rs:147`), called by `select_function` (`SG:7271`) before any block is selected, runs `accepts_graph` unconditionally, including for zero structural parameters. `validate_borrowed_argument` also compares the scalar parameters with the call plan (`:78-91`). The skipped check repeats one that already ran. | Intended |
| 7 | Record §2 "Fail-open risks", `:2229-2247`; record "What you are unsure about"; digest lines 68, 165 | `S05/selection_call_abi.rs` `integer_carrier_normalization` (`:2237`) and its callers | Narrow integers must be extended to the register carrier; other values need no extension. | Integers of 128 bits or of widths other than 8/16/32/64 have no register `ValueShape` (`calling-conventions/src/plans/vocabulary.rs:167-178` returns `None`), so they never reach a register carrier. `CopyI64` is correct for 64-bit integers and callables. Callers use `CopyI64` as "no extension needed" (`SG:200`, `SG:1193`, `SG:7358`). Floats go through `incoming_float_transfer` or `register_form_normalization` first (`SG:5453-5460`, `SG:6271-6276`, `SG:7351-7356`, `register_form_normalization` at `S05/selection.rs:1463`), and the other callers pass only integer types they have already matched. One float route is untraced: see [U1](#u1-indirect-float-result-of-a-normalized-foreign-call). | Intended, except U1 (still open) |
| 8 | Record §3 "Fail-open risks", `:448-470` | `S05/selection_edge_transfers.rs` `construction::prepare`, the unbridged fall-through path | Stage 12 `derive_order` (`12_selected-form-encoding-to-resolved-layout/src/ordinary.rs:68-100`) requires: no self fall-through, at most one fall-through predecessor per block, none into the entry block, and no fall-through cycle. It refuses any function that breaks these rules. | Stage 05 leaves a conditional's position-1 successor unbridged only if its target is not the entry block, is not already the target of another unbridged fall-through (`unbridged_fallthroughs`), and the new edge closes no cycle (walk through `fallthrough_next`, which starts with `current == source.id`). Every other case gets a `Jump` bridge block. Checking each edge as it is added keeps the fall-through graph acyclic. | Intended; the consumer refuses violations |
| 9 | Record §4 "Fail-open risks", `:3905-3944`; digest lines 162, 267 | `S05/selection_inputs.rs` `record_input::descriptor_family` (`:3910`) | Borrowed byte and element descriptors must keep the declared access. | Repaired in #5379 (`e0ffc8a3`): the function now calls `terminal_semantics::descriptor_reborrow_shapes_match` with the occurrence access when the expected carrier declares one. It is recorded in [07-13-psi-and-native.md](../07-13-psi-and-native.md) ("borrowed byte descriptor access, line 267"). | Duplicate (fixed in #5379) |
| 10 | Record §4 "Fail-open risks", `:5638`; record "What you are unsure about"; digest line 163 | `S05/selection_inputs.rs` `storage_origin_input::access_coordinates` (`:5561`), `_ => vec![whole(access.place)]` | `replay_memory_origins` must replace a read only with the returned views it actually reads. | Not traced to the consumer; see [U2](#u2-whole-place-coordinate-for-unlisted-read-operations). | Still open |
| 11 | Record §4 "Fail-open risks", `:6059-6068` | `S05/selection_inputs.rs` `collect_calls`, final `_` arm (`:6069`) | An unlisted producer may become an origin only if it carries no borrowed storage. | The arm returns a custody error when `contains_borrowed_storage` is true. `contains_borrowed_storage` returns `true` when a declaration is missing (`:6253`), so the lookup fails closed. A revisit of a recursive type returns `false` only after the first visit has already examined every field. | Intended (fails closed) |
| 12 | Record §5 "Fail-open risks", `:1758-1760`; digest line 167 | `S05/selection_interrupts.rs` `interrupt_installation_guards::root_interval` (`:1746`) and `stacks::validate_stacks` | [UEFI entry](../../../../../wiki/spec/build/uefi_entry.md): handed-off ranges must "exclude the live stack and retained state". | Not settled; see [U3](#u3-stack-overlap-check-when-no-entry-has-an-arrival-demand). | Still open |
| 13 | Record §2 "Fail-open risks", `:384-386` | `S05/selection_call_abi.rs` `validate_borrowed_argument`, `source.attachment.is_some_and(..)` | An attachment, when present, must name a declared structural type. | `None` means the function has no attachment, so there is nothing to look up. The record traced this; the current code is unchanged. | Intended |

### Record items with no claim

The record marks these as fail-closed, traced or "none seen". A reread of the
current code confirms each one:

- `S05/selection_access_lifecycles.rs`: `transfer_image` refuses an absent field (`:6532`); every `.find` goes through `.ok_or_else(failure(..))`.
- `S05/selection_call_abi.rs`: owned and borrowed arguments each go through exactly one validator (`:1978-1981`); aggregate result locations other than `Register` are `invalid` (`:2105`); `input_site::use_cut` and `invocation_key` return `None` on duplicates.
- `S05/selection_edge_transfers.rs`: a structural case with register bindings is refused (`:419-421`); leftover `edge_work` is an error (`:685`).
- `S05/selection_inputs.rs`: `view_access` returns `None` when its step budget runs out, and callers propagate it with `?`; `folded_literal`, `owned_store_destination` and `contains_borrowed_storage` fail closed; `replay_memory_origins` keeps non-read roles unchanged by design.
- `S05/selection_interrupts.rs`: the acknowledgement entry match (`:152-178`) is bounded by the roster size (`:76`) and the duplicate checks (`:143-147`); `validate_table` permits present gates only at admitted slots; `view_words` and `bounded_view` refuse stride 0; the static token check requires readability and exact size and alignment.

### Maintenance observations

The record's "Same job elsewhere", "Cases by shape" and "Unused or one-caller
code" sections, and digest lines 37, 50-51, 66, 68, 70 and 99-102, describe
duplicated helpers and fixed shape lists in this unit. None of them reports a
wrong result. The fixed shape lists refuse every other shape. They are left for
the [code repair queue](../../../../../OWNER_CODE_CONCERNS.md) if a repair is
taken:

- Comparison materialization repeated in `outcome::compare`, `Emission::compare/equal/at_most`, `Checks::compare` and `static_interrupts::require_equal`.
- One `Boundary` local slot plus `FrameAddress`, written twice: `ImageHome::scratch` and `interrupt_installation::control_home`. Digest line 254 notes that unit_013 named `selection_interrupts.rs:796` instead. The two readers name different twins; this packet does not settle which one is the intended pair.
- Counter loops: `Emission::repeat` and `Checks::iterate`.
- Seven place-declaration lookups in `S05/selection_inputs.rs` with different priority orders.
- Two instruction-kind lists for operand sources: `parameter_home_required` and `requires_descriptor`.
- `Emission::emit` in `S05/selection_edge_transfers.rs` next to `Builder::emit`, and the hand-written copies inside `structural_case::snapshot`.
- Stage 05 repeats the stage 04 call-plan checks (`selection_call_abi.rs:1895`); the stage 04 side was not read here.

## Unresolved claims

### U1. Indirect float result of a normalized foreign call

`SG:5453-5460`: when `scalar_result_buffer::is_indirect(call)` is true, the
code skips `incoming_float_transfer` and copies the loaded result with
`integer_carrier_normalization(result.scalar_type)` and the `copy_i64` key. For
a Binary32 or Binary64 result this emits an integer copy into a float-typed
register. I did not find whether a boundary entry plan can place a float scalar
result as `ValueLocation::Indirect`; `is_complete_scalar_result` accepts an
indirect location of any class. Next check: build a
`LegalizedNormalizedForeignCall` with an IEEE float result and an indirect
result placement in a stage 05 unit fixture. Then see whether stage 04 can
produce that plan, or whether selection or the register-constraint check
refuses it.

### U2. Whole-place coordinate for unlisted read operations

`access_coordinates` uses the whole source place for any instruction kind it
does not list. `replay_memory_origins` then replaces the original read row with
one `ReadReturnedView` row for each returned-view producer found under that
coordinate, and drops the row's byte offset and count. The reads that reach
this code have the roles `ReadByteSequence`, `ReadByteSpan`, `ReadElementView`
and `ReadIndexedPrimitive`. They come from `footprint_at_origin`
(`S05/selection_structural.rs:4771`), `copy`, `read_byte`, `replace`,
`byte_sequence_read` and `element_view_read` (`S05/selection_views_and_bytes.rs`).
I did not map every instruction kind behind those emitters to the listed
`access_coordinates` arms. Next check: for an indexed scalar read from a record
in which one field holds a call-returned view and another field holds owned
bytes, read the owned field. Then see whether replay replaces the owned read
with the unrelated producer's row, and whether a stage 06 memory-effect
consumer relies on the dropped offset.

### U3. Stack overlap check when no entry has an arrival demand

`root_interval` returns `Ok(None)` when no guard entry has
`requirement.arrival`. `validate_stacks` then skips the check that a dedicated
interrupt stack's backing lies outside
`[RetainedEntryStorageStart, ActivationBase]`. The function refuses a roster in
which only some entries have an arrival demand. The spec requires handed-off
ranges to exclude the live stack. I did not establish what keeps a dedicated
stack off the installer's live stack when the installation has no physical
arrival. It may be that the stack backing is an ordinary mutable loan, or that
provider admission (`verification/provider-admission/src/static_interrupts/scopes.rs`)
rules it out. Next check: read the scope admission for installations without
arrival entries, or build a selection fixture with a dedicated stack and no
arrival.

## Build break found during validation

At `a0152d26`, and still at `origin/main` `9117a5cc`, the focused stage 05 test
build stopped at E0004 in two crates:

| Crate and match | Fix | Fixture |
| --- | --- | --- |
| `optimization-unit`, `identity.rs` `proposition_encoding::encode_proposition` | Encode tag 23 and every field of the clause. Published Psi still refuses a declaration Identity (`psi/artifact/wire/src/foundation_validation.rs:6080`); the arm keeps the optimizer identity total and collision-free. | `declaration_identity_tests::declaration_identity_commits_every_clause_field` changes one clause field at a time and requires different bytes. |
| `optimization-unit-semantics`, `unit_validation.rs` `claim_transfers::proposition_structural_roots` | Report the root of each scalar read and input subject. Formal parameters and applications name no place. | `declaration_identity_roots_tests::declaration_identity_reports_scalar_and_input_roots` requires exactly the read and input roots. |

Before: `mbx test --locked -p target-operations-to-selected-instructions --lib -- ...`
failed with `error[E0004]: non-exhaustive patterns: &semantic_vocabulary::Proposition::DeclarationIdentity(_) not covered`,
first at `optimization-unit/src/identity.rs:267`, then, after that fix, at
`optimization-unit-semantics/src/unit_validation.rs:10668`. After: the command
passes. I did not build the whole workspace, so crates outside the stage 05
dependency tree may still have the same gap.

## Validation

- `mbx test --locked -p target-operations-to-selected-instructions --lib -- run_supply_checks runtime_profile_ zero_backing malformed_runtime resident_authored_value_codec resident_scalar_decode`: 8 passed. These cover claims 1-5: run relocation supply checks, runtime profile validation and compatibility against the Psi normalizer, malformed profiles, zero backing, and resident scalar codecs.
- `mbx check --locked -p target-operations-to-selected-instructions --all-targets`: compiles; one existing dead-code warning at `S05/selection.rs:1523`.
- `mbx test --locked -p optimization-unit --lib`: 15 passed.
- `mbx test --locked -p optimization-unit-semantics`: 25 passed.
- `mbx clippy --locked -p optimization-unit -p optimization-unit-semantics --all-targets -- -D warnings`: clean.
- `rustfmt --edition 2024 --check` on both edited files: the only diffs are hunks already present on `main`.
