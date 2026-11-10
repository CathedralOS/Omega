# Unit 111 notes

Files covered by the unit (paths under `omega-rust/psi/interpreter/`):
- checked-interpreter/src/program_values.rs (1-2469, read in full)
- checked-interpreter/src/value.rs (1-543, read in full)
- terminal-interpreter/src/boundary_effects.rs (1-3787; read 1-2300 only)
- terminal-interpreter/src/boundary_inputs.rs (1-2814; NOT read)
- terminal-interpreter/src/boundary_storage_inputs.rs (1-2703; NOT read)
- terminal-interpreter/src/boundary_values.rs (1-2391; NOT read)
- terminal-interpreter/src/embedding.rs (1-3012; NOT read)
- terminal-interpreter/src/engine_calls.rs (1-3713; NOT read)

I ran out of room. Only 5,312 of 21,432 lines were read. Nothing below describes the unread ranges, except two grep hits noted at the end.

---

## 1. checked-interpreter/src/value.rs (543 lines, read in full)

**Purpose.** Runtime values for the checked-tree interpreter. Every storage place is an `Rc`-owned cell, so `&mut` borrows share the cell. Reference values can also carry an encoded "view" (backing cells, projection, physical codec). Cells and Text bytes are metered, optionally against a build-evaluation sponsor.

**Shape.**
- `Reference { cell, storage: Option<ReferenceStorage> }` (value.rs:21). `Reference::new` has no storage; `Reference::view` has storage (:27, :34).
- `ReferenceStorage` (:43) has four variants: `Direct{source,target}` (scalar recast), `ByteRegion{cells,offset,target}`, `AggregateByteRegion{cells,projection}`, `AggregateTyped{source,source_type,projection}`. `target()` returns `Some` only for the first two (:65).
- `ReferenceProjection{offset,type_reference,stored_integer}` (:74).
- `Cell(Rc<CellAllocation>)` (:81) derefs to `RefCell<Value>`. `Cell::ptr_eq` (:111).
- `CellMeter` (:123): `allocate` (:136) reserves a sponsor lease, bumps live and peak counters, and returns a `Cell` holding a `LiveCellLease` whose `Drop` decrements (:170).
- `TextBuffer` (:179): shared `Rc<TextAllocation>` plus an optional window lease (`subslice` :239). `write_byte`, `write_prefix`, and `replace` (:289) are the mutators.
- `TextByteMeter` (:330) with `allocate` (:343), and `LiveTextByteLease::grow/shrink` (:390, :413).
- `Value` enum (:428): `Unit, Int(i64), Bool, Float(f64), UndeterminedNaN(FloatFormat), Str(TextBuffer), Struct{type_symbol,type_name,fields: BTreeMap<ValueName,Cell>}, Enum{type_symbol,variant_name,payload: Vec<(name,Cell)>}, Array(Vec<Cell>), Ref(Reference)`.
- `Value::deep_clone_with` (:486).

**Inputs/outputs.** Reads `build_evaluation_sponsor` leases (:10). Counters are `Cell<u64>` inside `Rc` accounts and are mutated by allocate and drop.

**How it decides.**
- `Value::as_int` treats `Bool` as 0/1 (:450). `as_bool` treats any nonzero `Int` as true (:458). `as_float` converts `Int` to `f64` (:466). These are lenient coercions keyed on the runtime variant.
- `TextBuffer::replace` (:289): while any bounded window is live, a replacement must have the same length as the view, otherwise it errors (:292-299). With no live windows it may grow or shrink and adjusts the lease.
- `subslice` rejects `start>end` or `end>length` (:242).

**Cases by shape.** `deep_clone_with` clones `Struct`, `Enum`, and `Array` recursively and re-allocates cells. For `Str` and `Ref` it only does `self.clone()` (:530-536). A cloned `Str` therefore shares the same `Rc<TextAllocation>` (inferred from the `Clone` derive at :178), and a `Ref` shares its cell.

**Same job elsewhere.** `CellMeter` (:123) and `TextByteMeter` (:330) have parallel structure: account, peak, sponsor lease, and `Drop` lease. Same file, same job twice, one per resource.

**Unused/one-caller.** Not checked.

**Stated limits.** None.

**Fail-open.** `Drop` for `LiveCellLease` uses `saturating_sub` with only `debug_assert!(live > 0)` (:173-174), so an accounting underflow is hidden in release builds. This is accounting only, not a program-acceptance path. "none seen" otherwise.

---

## 2. checked-interpreter/src/program_values.rs (2469 lines, read in full)

**Purpose.** Evaluator helpers for the checked-tree interpreter: array windows, place resolution, reference views (recasts over byte regions or typed records), record-view layout and byte assembly, type metadata lookups, and read-cell projections. All are `impl Evaluator` blocks split into inline modules.

**Shape (modules and key functions).**
- `array_windows` (:9): `capture_array_window` (:15) and `write_array_window` (:63). Both are called only from `execution.rs:146` and `:211` (grepped: 2 call sites).
- `names_recasts_and_places` (:111):
  - `field_cell` (:127), `eval_name` (:166), `read_reference` (:185).
  - `capture_reference_view` (:217), `reference_recast_initializer` (:247), `reference_storage_path` (:358).
  - `read_mutable_record_recast_target` (:427), `project_record_reference` (:466), `prepare_mutable_record_recast_write` (:601), `write_mutable_record_recast_target` (:615).
  - `enum_value_from_path` (:672), `resolve_place` (:720), `resolve_name_place` (:751).
  - `coerce_scalar_with` (:839), `field_type_reference` (:893), `eval_subslice` (:966).
- `program_lookup` (:1010): `find_machine_by_name/symbol`, `find_state`, `find_state_by_symbol`, `find_data_by_name`.
- `record_views` (:1054): byte-region assembly and writing (`assemble_scalar_byte_region` :1064, `write_scalar_byte_region` :1095, `write_stored_integer_byte_region` :1135, `assemble_record_view_type` :1327, `record_view_type_layout` :1474, `record_view_fields` :1559, `record_view_type_projection` :1680, `write_record_view_type` :1880, `eval_recast` :1996).
- `type_metadata` (:2049): `eval_state_argument` (:2057), `expression_type_reference` (:2099), `collection_element_type` (:2168), `eval_recast_to_type` (:2183), `expression_scalar_type` (:2252).
- `value_projections` (:2359): `eval_read_cell` (:2367), `require_builtin_index` (:2448).

**Inputs/outputs.** Reads the program tables (`expression_table`, `type_reference_table`, `data_definition_by_symbol`, `plan_laid_layout_for_data`, `layout_storage_arguments`, `closed_data_definition`). Reads and mutates `Cell`s, and allocates new cells through `allocate_cell`. Writes go to byte cells for mutable recasts (`write_scalar_byte_region` :1130, `commit_typed_value_bytes` :1862).

**How it decides.**
- **Field lookup** is by authored string. `field_cell` finds struct fields by name in a `BTreeMap` (:134), enum payload by name (:143), and the array/text length field by `CollectionMeasure::from_authored_spelling(field)` (:151, :157). This is source spelling.
- **Boolean literal** is recognised in `eval_name` by spelling: `boolean_literal_spelling(name.as_str())` (:173, also :2392).
- **Machine body selection.** `find_machine_by_symbol` swaps in a build-time selected body by matching `selected.family == symbol` (:1022-1026).
- **State lookup by name.** `find_state` compares `state.name.as_str() == name` (:1034). `find_state_by_symbol` uses a symbol and returns `None` on an invalid symbol (:1042).
- **Field type lookup by name.** `data_field_type_reference` compares `field.name.as_str() == field_name` (:925); the machine-owned data loop at :911 does the same.
- **Record-view projection** is keyed on field name strings. `record_view_projection` finds a field by `name == field_name` (:1654) and traps if absent (:1655-1659).
- **Record layout identity** uses `normalized_type_identity` plus a `visiting` set to cut recursion (:1199-1202, :1508-1511). A revisit returns `None`/`Ok(None)`, not an error.
- **Layout source.** A laid-out layout plan (`plan_laid_layout_for_data`) is used if present. Otherwise it falls back to natural C-style alignment (:1264-1275, :1546-1553). If layout storage arguments exist but no plan is available it traps (:1233-1243) or returns `None` (:1531-1538, :1586-1593).
- **Scalar type of an expression** (`expression_scalar_type` :2252): for a binary op it picks the wider integer width, and on a tie prefers the left unless left is non-`Exact` (:2315-2325). Match arms with differing types give `None` (:2262). A literal gets its type from `literal.landing()`.
- **Builtin indexing check** (`require_builtin_index` :2448) calls `builtin_index_read_is_spelled(...)`. The name suggests a spelling test, but I did not read the callee, so what it keys on is unverified.

**Cases by shape (fixed lists that refuse the rest).**
- Recast sources: `reference_recast_initializer` handles an `Indexed` source over a byte array (:281-309), a record/typed source (:310-344), and a scalar source (:345-355); otherwise `Ok(None)`.
- `reference_storage_path` handles only `Borrow`, recast `Cast`, `Name`, `Member`, `Indexed`; `_ => Ok(None)` (:423).
- Record-view layouts handle only primitive scalars, `Constrained`/`Reference` (transparent), `FixedArray` with `FixedArrayLength::Literal`, `Slice`, and `Named`/`Generic` data; `_ => None`/`Ok(None)` (:1470, :1499). Non-literal fixed-array lengths are refused.
- Data layout requires every member to be a field (`member.as_field()`). A non-field member returns `None` (:1210-1213, :1516-1519). Variants/other members make a type unlayable.
- Stored-integer width must be 8..=64 bits and a multiple of 8, else trap (:1144-1148, :1350-1355).
- `resolve_place` supports `SavedSubject`, `Name`, `Member`, `Indexed`, `Borrow`; anything else must evaluate to a `Value::Ref`, otherwise `unsupported("place expression not supported")` (:744-747).
- `eval_recast` handles `F32`, `F64`, and integer targets (:2011-2043).

**Same job elsewhere (inside the unit).**
- Three near-identical layout walkers: `assemble_record_view_inner` (:1192), `record_view_data_layout` (:1503), `record_view_fields` (:1559). Each independently recomputes field specs, offsets, and the laid-layout plan check. The condition "layout_storage_arguments present but no plan" appears at :1233, :1531, :1586.
- Three `declared_type_is_*` unwrap loops over `Constrained`/`Reference` (:787, :1828, :2086).
- Four places that strip `Constrained`/`Reference` wrappers (:1394, :1484, :1700, :1912), plus `collection_element_type` (:2168) and `projected_member_type` (:2151).
- The sub-slicing logic exists twice: `capture_array_window` (:15-60) and `eval_subslice` (:966-1006). Both compute start/end/inclusive bound and the same bounds check, with different error text.
- Suspected outside the unit: `field_type_reference` (:893) overlaps other field-type lookups in `checked-interpreter`. I did not read them.

**Unused/one-caller code (grepped).**
- `capture_array_window` and `write_array_window`: 1 call site each, both in `execution.rs` (:211 and :146). Command: Grep `capture_array_window|write_array_window` over `omega-rust`.
- `read_mutable_record_recast_target`: callers at `execution_expressions.rs:87` and `program_values.rs:2388/2404/2420`.
- `prepare_mutable_record_recast_write` and `write_mutable_record_recast_target`: each one caller in `execution.rs` (:220, :148).
- `eval_recast_to_type`: one external caller at `execution_expressions.rs:401`, plus self-recursion.
- `expression_is_unsigned64`: callers at `execution_expressions.rs:333, 343, 344`.
- I did not grep the other `pub(crate)` helpers.

**Stated limits.** The only "unsupported" strings are the fixed-shape refusals above, e.g. "array window requires array or byte-carrier storage" (:39) and "array value projection has no exact builtin indexing meaning" (:2464).

**Fail-open risks.**
- `coerce_scalar_value` (:877-891): if `primitive_type_reference` is `None`, it returns the value unchanged with no coercion. A non-primitive destination skips range/domain wrapping. This is probably intentional because non-scalars pass through, but it is "open" (callers not traced).
- `coerce_scalar_with` (:845-875): `_ => Ok(value)` leaves `Bool`, `Str`, and non-F32 floats unchanged. Integer values go through `apply_arithmetic_domain` (:858). "open".
- `reference_recast_initializer` and `reference_storage_path` return `Ok(None)` for any unrecognised initializer shape (:252-256, :423). The caller `capture_reference_view` then returns `Ok(None)` (:233-235). I did not trace what the caller does with `None`, so whether a recast that cannot be viewed is silently treated as an ordinary reference is "open".
- `assemble_record_view_inner` returns `Ok(None)` when `visiting` already contains the type (:1200-1202), so a recursive type yields "no view" rather than an error. Callers turn `None` into traps in `read_record_view_projection` (:591-598) and `commit_typed_value_bytes` (:1868-1875). Those are "traced" fail-closed.
- `expression_scalar_type` for `Binary` returns `left.or(right)` when only one side is typed (:2326). A mixed typed/untyped operand therefore takes the one known type. "open".
- `record_view_projection` (:1650-1654) uses `.unwrap_or_default()` on `record_view_fields`, so an unlayable type produces an empty field list. That then traps with "has no field" (:1655-1659), which is fail-closed but gives a misleading message.
- `projected_member_type` returns `None` for an unknown member. `expression_type_reference` then returns `None`, and callers such as `assignment_target_coercion` (:815-837) return `None`, meaning no coercion. "open".
- Not fail-open, but notable: `write_array_window` on a `Str` target silently truncates nothing; it errors when a byte does not fit `u8` (:92-95).

---

## 3. terminal-interpreter/src/boundary_effects.rs (3787 lines; read 1-2300)

**Purpose (from the header, :1-8, and the read portion).** Effects at the host boundary of the terminal-Psi interpreter. It stages byte and scalar buffers before a call, validates host responses against declared contracts, and commits writebacks. Modules read: `boundary_predicates` (:10), `byte_buffers` (:483), `crash_outcome` (:1772), `normal_qualifications` (:1915), and the start of `original_storage` (:2153-2300). The remaining modules (2301-3787) I did not read.

**Shape.**
- `boundary_predicates` (:10-482):
  - `validate_input_requirements` (:43): checks `boundary.scalar_requires` against the actual scalar arguments. It builds a `PropositionContext`, substitutes argument values (`substitute_proposition_values` :105), then evaluates each requirement.
  - `InputObservations` (:123) turns structural reads into literals (`close_scalar` :201, `read` :361, `read_extent` :271).
  - `closed_guard` (:397): evaluates a closed proposition. It handles `Truth`, `Falsehood`, IEEE float comparison, boolean `Equal`, ordered integer comparison, integer-math comparison, `Conjunction`/`Disjunction`, and `Implication`. Everything else gives `None`.
- `byte_buffers::writeback` (:484): `TerminalBoundaryByteBuffer` (:499) with scope `View | Owner`. `BoundaryByteBufferBinding::capture` (:548), `validate` (:609), `commit` (:667).
- `byte_buffers::BoundaryArguments` (:729): per-argument parallel vectors (`values`, `aggregate_backings`, `bytes`, `buffers`, `storage_inputs`, `bindings`, `byte_sequences`, `primitive_loans`, `element_views`). `into_call_arguments` (:772) validates and converts them into `StructuralCallArguments`.
- `TerminalExecution::resolve_boundary_arguments` (:1039) and `prepare_boundary_arguments` (:1186), `prepare_byte_sequence_loan` (:1287), `prepare_owner_view_argument` (:1425), `resolve_mutable_boundary_binding` (:1472), `lend_declared_owner` (:1516), `resolve_shared_owner_binding` (:1619), `bounded_owner_snapshot` (:1643), `resolve_shared_boundary_binding` (:1699), `structural_field_binding_frame` (:1740).
- `crash_outcome` (:1772): `validate_boundary_crash_outcome` (:1789) and `validate_boundary_crash_routes` (:1862), with error enum `BoundaryCrashOutcomeError` (:1780).
- `normal_qualifications` (:1915): `CapturedNormalQualification` (:1923). `normal_qualifications_cover` (:1957), `capture_normal_qualifications` (:1976), `normal_qualification_is_covered` (:2018), `validate_normal_case_preservation` (:2041), `normal_qualification_subject` (:2103).
- `original_storage` (:2153-2300 read): `TerminalOriginalStorageRoot{artifact,namespace,allocation}` (:2165), `TerminalOriginalStorageRequest` (:2207), `TerminalOriginalStorageLoan` (:2272) and `scalar_path` (:2294).

**Inputs/outputs.** Reads `BoundaryMachineDeclaration` (scalar requires, structural parameters, crash routes), the live `TerminalExecution` (structural values, byte-sequence fields, call stack frames, claims), and the host-supplied structural arguments. Mutates execution state only at `commit` (:667-699): stored byte-array writes, `structural_byte_sequence_fields.insert`, and owner bytes.

**How it decides.**
- **Input requirements** (:101-119): each requirement must evaluate to `Some(true)`. `Some(false)` errors. `None` (no closed evaluation) also errors ("has no supported closed evaluation"). This is fail-closed.
- **Crash routes** (:1838-1859): selects the bucket whose `cause == cause`. An undeclared cause is an error (:1822-1826). The first guard that evaluates `Some(true)` wins; `None` marks "unsupported" and `Err(limit)` marks "exhausted". The result is `Ok` only on a true guard.
- **Crash route declaration order** is validated by comparing `cause` and guard order with `windows(2)` (:1865-1876). A `Truth` guard must be alone in its bucket (:1875-1876).
- **Writeback validity** (`BoundaryByteBufferBinding::validate` :609-665) compares argument index, capacity, scope, and the current capacity of the source against what was captured.
- **Byte binding access class** (`into_call_arguments` :890-924) is derived from the binding variant and `borrowed_storage_reference_ceiling`. A mismatch with the parameter's declared access is an error (:925-949).
- **Normal qualifications** are keyed on `(domain, carrier, captured subject)` equality (:2036-2038). The subject is either the active runtime place, or an inactive case guard plus path suffix (:1931-1938).
- **Place identities** are by `PlaceId`/`StructuralRuntimePlace`, not by names. Structural parameters are matched to predicate roots by `parameter.place == root` (:282, :371).

**Cases by shape.**
- `read_extent` (:271): only `ElementView` shapes and `ByteSequence` shapes (when not write-only) are observable. Otherwise "boundary extent requires an observable live view descriptor" (:338-342).
- `read` (:361) refuses write-only arguments ("cannot observe a write-only argument" :380).
- `closed_guard` (:397-480): fixed list of proposition kinds, `_ => None` (:479). Integer comparisons require equal types (:438).
- `BoundaryByteBufferBinding::capture` handles `MutableArray` (View scope only), `MutableField`/`MutableOwner`; `Immutable`/`Representation` are refused with `malformed()` (:589-591). Same refusal at :657-659 and :694-696.
- `prepare_byte_sequence_loan` (:1287-1421): branches on parameter access and byte-sequence shape: bounded-owned exclusive borrow, shared borrow of bounded-owned, shared borrow of borrowed view with a non-empty path, mutable borrow of borrowed view. Anything else returns `Ok(None)` (:1422).
- `normal_qualification_subject` (:2108-2150): a path segment `AllElements` is refused ("needs an exact captured element" :2111-2113).
- `lend_declared_owner` (:1516-1617): looks for the owner's storage home in the storage table or in call-stack frames. The `Immutable` case counts only if the frame owns storage (parameter with `Owned` access, or an operation-result place) (:1575-1589).

**Same job elsewhere.**
- `closed_guard` is shared by input requirements (:159) and crash routes (:1841). It is also called from `terminal-installation` (`omega-rust/psi/compilation/validation/terminal-installation/src/evaluation_and_synchronous_calls.rs`, grepped `closed_guard\(` : 6 hits there, 6 in this file). It is shared with the installer, which is consistent with its role.
- `InputObservations::evaluate` (:131) duplicates the `Conjunction`/`Disjunction`/`Implication` walk of `closed_guard` (:456-478), with short-circuit evaluation in one and strict evaluation in the other.
- Three-valued handling and the same structural-qualification checks appear in `prepare_byte_sequence_loan` branches (:1347-1350, :1360-1363, :1374-1377, :1395-1399), each repeating the same "access equal, multiplicity unrestricted, projected qualifications empty" guard. Same job repeated four times in a row.

**Unused/one-caller code (grepped).**
- `validate_input_requirements`: 1 external call site, `engine_calls.rs:3383` (Grep `validate_input_requirements\(` over `omega-rust`: 2 hits including the definition).
- `validate_boundary_crash_outcome`/`BoundaryCrashOutcomeError`: 16 hits in this file, 1 in `boundary_values.rs` (Grep count over `omega-rust`). So there is one consumer outside the file; I did not read it.

**Stated limits (quotes).**
- "opaque argument transport cannot discard storage or loan companions" (:758).
- "boundary write-only descriptor extent cannot cross a reference" (:309).
- "case replacement cannot establish an uncaptured normal membership" (:2075).
- "normal host membership preservation needs an exact captured element" (:2112).

**Fail-open risks.**
- `validate_input_requirements` early-returns `Ok(())` when `boundary.scalar_requires.is_empty()` (:51-53). Structural requirements that are not in `scalar_requires` are not checked here. Whether they are carried elsewhere I did not trace: "open".
- `validate_boundary_crash_routes` returns `Ok(())` for an empty `crash_routes` (:1885-1887), so a boundary with no declared routes passes validation. That follows from "no routes declared"; the cause lookup at :1822-1826 still rejects any cause, so a crash is not accepted. "traced" (read the code).
- `close_predicate` (:168-185) leaves unmatched propositions untouched (`_ => {}`), and `closed_guard` then yields `None`, which the callers treat as an error. Fail-closed for both users.
- `normal_qualification_is_covered` returns `Ok(true)` if the qualification is "intrinsic" (:2025-2027), meaning entailed by the scalar projection type. Stated by the helper name `scalar_projection_entails_membership`; callee not read: "open".
- `capture_normal_qualifications` skips rows that are intrinsic (`continue` at :1996-1998). Same "open" note.

---

## 4. Files not read

`boundary_inputs.rs`, `boundary_storage_inputs.rs`, `boundary_values.rs`, `embedding.rs`, `engine_calls.rs`, and `boundary_effects.rs` 2301-3787 were not read. No claims are made about them. Two grepped facts only: `validate_input_requirements` is called at `engine_calls.rs:3383`; `validate_boundary_crash_outcome` has one hit in `boundary_values.rs`.

---

## Coverage

- Read in full: `program_values.rs` 1-2469 (2469 lines); `value.rs` 1-543 (543 lines); `boundary_effects.rs` 1-2300 (2300 lines). Total 5,312 lines.
- Skimmed: none.
- Not read: `boundary_effects.rs` 2301-3787 (1487); `boundary_inputs.rs` 1-2814 (2814); `boundary_storage_inputs.rs` 1-2703 (2703); `boundary_values.rs` 1-2391 (2391); `embedding.rs` 1-3012 (3012); `engine_calls.rs` 1-3713 (3713). Total 16,120 lines.

## Five things a maintainer most needs to know about the read part of this unit

1. The checked interpreter's reference model has four storage kinds (`value.rs:43`), and every recast (`program_values.rs:247`) must pick one. `reference_recast_initializer` and `reference_storage_path` return `Ok(None)` on any shape they do not list (:252-256, :423), so the caller sees "no view" and I did not trace what it then does.
2. Record-view layout is computed three separate times with the same checks (`:1192`, `:1503`, `:1559`). A change to the "layout storage arguments but no plan" rule must be made at :1233, :1531, and :1586.
3. Field, state, and data lookups in the checked interpreter are by authored name string (`:134`, `:1034`, `:925`, `:1654`). The length pseudo-field is matched by `CollectionMeasure::from_authored_spelling` (:151).
4. Boundary input requirements and crash routes both fail closed on anything `closed_guard` cannot evaluate (`boundary_effects.rs:113-118`, `:1853-1859`). `closed_guard` is also called from the terminal-installation crate (6 hits in `evaluation_and_synchronous_calls.rs`).
5. Boundary byte buffers are staged as copies and committed only on success (`boundary_effects.rs:593-606`, `:667-699`). Only `MutableArray`, `MutableField`, and `MutableOwner` bindings can be written back; `Immutable` and `Representation` are `malformed()` at :589-591.

## What I am unsure about

- What `capture_reference_view` callers do with `Ok(None)`. I did not read `execution.rs`.
- Whether `builtin_index_read_is_spelled` (`typed_trees::operator`) keys on source spelling or on types. I only saw the call at `program_values.rs:2458`.
- Whether structural (non-scalar) boundary requirements are validated somewhere else when `scalar_requires` is empty (`boundary_effects.rs:51`).
- Everything in the six unread ranges.
