Second pass for unit 112

Ranges covered (one file, whole file, starts at line 1 so it begins at the file header, not inside a function):
- `omega-rust/psi/interpreter/terminal-interpreter/src/storage_bytes_and_views.rs` 1-5598 (5598 lines)

Contradiction with earlier notes: none. The earlier notes (`unit_112.md:9`, `:117`) say this file was NOT READ and only cite one grep hit (`resolve_structural_path_shape` called from here). That is confirmed: it is called at `storage_bytes_and_views.rs:4057` (read).

All paths below are relative to `omega-rust/psi/interpreter/terminal-interpreter/src/`. Everything in this file is `impl TerminalExecution<'_>` methods or helper types inside two top-level modules: `byte_sequences` (`:10-3122`) and `element_views` (`:3124-5598`).

---

## 0. File layout (read)

| Lines | Module | What |
|---|---|---|
| 1-8 | header comment | "Accounted byte-sequence storage, bounded byte-field bindings, and typed or representation element views" |
| 10-3122 | `pub(crate) mod byte_sequences` | all byte-sequence code |
| 11-1142 | `binding` | `ByteSequenceBinding` enum, `ByteOwnerHome`, descriptor resolution, reborrow, owner byte access |
| 1143-1186 | `concatenation` | `execute_byte_sequence_concatenation` |
| 1187-1206 | `equality` | `execute_byte_sequence_equal` |
| 1207-1333 | `field_index_store` | byte-field length / byte store / byte read ops |
| 1334-1644 | `field_store` | whole-field store, view-field store, byte-field carrier resolution |
| 1645-1743 | `operands` | `fixed_byte_array_observation`, `byte_sequence_operand_view` |
| 1744-1762 | `predicate` | `execute_byte_sequence_predicate` |
| 1763-1984 | `read` | `TerminalByteRead` (borrowed read of contiguous, cell, or representation bytes) + 2 tests |
| 1985-2220 | `storage` | `ByteSequenceStorage` (accounted `Vec<u8>`) + 4 tests |
| 2221-2292 | `store` | `execute_byte_sequence_store` |
| 2293-2540 | `subslice` | `execute_byte_sequence_subslice`, `narrow_byte_sequence_binding` |
| 2541-2909 | `write` | `MutableByteSequenceStorage`, `mutable_byte_sequence_storage`, `owned_byte_sequence_storage`, length/read/write ops |
| 2911-3121 | (module body) | `ByteSequenceView`, `OriginalByteArrayWindow`, `ByteSequenceBacking` |
| 3124-5598 | `pub(crate) mod element_views` | element views |
| 3125-3186 | `immutable_sequence` | `ImmutableStructuralSequence` |
| 3188-3830 | (module body) | `ElementViewHome`, `CollectionStorage`, `ElementView`, `ElementViewBacking` |
| 3832-4917 | (module body) | `count` and one big `impl TerminalExecution` (establish / write / length / read / subslice) |
| 4919-5293 | `representation` | `RepresentationView` (bytes viewed as typed elements) |
| 5294-5536 | `typed` | `TypedElementView` (one element type viewed as another) |
| 5538-5597 | `tests` | 2 tests on descriptor-home matching |

---

## 1. `byte_sequences::binding` (11-1142)

1. Purpose. Defines what a byte-sequence "descriptor" (a borrowed or owned byte value held at a place) is bound to at run time, and resolves a descriptor argument to its binding, checks it, reborrows it shared or mutable, and exposes the owner bytes behind it.
2. Shape.
   - `enum ByteSequenceBinding` (`:28-52`): `Immutable(ByteSequenceView)`, `Representation(Arc<ElementView>)`, `MutableArray{array: ArrayStorage, array_type, offset, length, referent, descriptor_home}`, `MutableField{field, capacity, parent_type, referent, descriptor_home}`, `MutableOwner{home: ByteOwnerHome, capacity, referent, descriptor_home}`.
   - `enum ByteDescriptorSource {Stored, CapturedReferent}` (`:54-57`, private).
   - `enum ByteOwnerHome {Frame{depth, machine, place}, Storage(StructuralRuntimePlace)}` (`:60-67`).
   - Methods on the binding: `access` (`:70`), `descriptor_home` (`:79`), `immutable_mut` (`:100`), `with_descriptor_home` (`:109`), `relocated_descriptor` (`:129`), `immutable` (`:147`), `snapshot` (`:157`), `read_view` (`:174`), `matches_shared_home` (`:190`), `owner_loan_referent` (`:198`), `validate_presented_referent` (`:209`), `validate_mutable_referent` (`:219-356`).
   - On `TerminalExecution`: `byte_binding_qualifications_hold` (`:360`), `byte_field_read` (`:378`), `byte_field_length` (`:419`), `byte_field_snapshot` (`:435`), `declared_byte_owner` (`:451`), `byte_descriptor_binding` (`:474`), `retained_byte_descriptor_binding` (`:512`), `byte_descriptor_source` (private, `:525-674`), `byte_descriptor_length` (`:676`), `shared_byte_descriptor_binding` (`:718`), `prepare_owned_byte_descriptor_argument` (`:753`), `prepare_byte_descriptor_argument` (`:857`), `reborrow_byte_sequence_binding` (`:967`), `exclusive_owner_bytes` (`:1031`), `byte_owner_bytes` (`:1059`), `byte_owner_bytes_mut` (`:1091`).
3. Inputs/outputs. Reads `TerminalExecution` tables: `structural_byte_sequence_fields`, `structural_byte_storage`, `structural_byte_descriptors`, `activation_storage.claim_descriptors`, `byte_sequence_values`, `structural_values`, `call_stack[*].byte_sequence_values`, `live_claims`, `structural_cases`. Produces a `ByteSequenceBinding` or an error; `byte_owner_bytes_mut` hands out `&mut ByteSequenceView` after calling `invalidate_stored_provenance` (`:1105`, `:1133`).
4. How it decides.
   - Mutable bindings are validated against the value presented: `validate_mutable_referent` compares `value == descriptor_home.unwrap_or(referent)` (`:246`, `:294`, `:326`), then identity (`opaque_identity`) and path of the backing array/field (`:259-261`, `:327-328`), type shape (`:296-304`) and field declaration by `id`, `identity`, relevance and carrier (`:344-352`). Keyed on identity + path + type id.
   - `byte_descriptor_source` (`:525`) finds a binding by trying, in order: claim descriptor at the selected place (`:567`), `structural_byte_descriptors` (`:575`), `byte_sequence_values[argument.place]` when path empty (`:589`), `captured_byte_owner_binding` (`:601`), field-by-identity (`:616-642`), then returns `None` if the place has stored bytes or an initialised empty array (`:643-648`), else scans the current frame and every call-stack frame for any place whose bound structural value matches (`:649-673`). The scan matches by `opaque_identity` + `path` + `structural_type` (closure `matches`, `:552-566`).
   - Permission match in `prepare_owned_byte_descriptor_argument` (`:814-846`) keys on binding variant and parameter multiplicity/access: shared views need Unrestricted + `None|SharedBorrow`; mutable views need Affine (or activation claim) + `None|MutableBorrow` + owned source.
   - `prepare_byte_descriptor_argument` (`:857`) returns `Ok(None)` (not an error) for several non-matching shapes: borrowed-storage param with no ceiling (`:865-873`), Owned param (`:874`), place not in `structural_values` (`:877`), projection failure (`:880-884`), non-borrowed-byte carrier (`:885-891`).
5. Cases by shape.
   - `byte_descriptor_source` field branch (`:616`): "only when the last path segment is `Field`", with an optional preceding `Case` segment (`:619-622`).
   - `validate_mutable_referent` has three hard-coded arms (MutableArray, MutableOwner, MutableField), anything else `malformed()` (`:311-320`).
   - `snapshot`/`reborrow` shared of a mutable binding: arm per variant, `unreachable!()` for Immutable/Representation (`:1022-1024`).
   - `prepare_owned_byte_descriptor_argument` accepts only `BorrowedView` and `BoundedView` carriers (`:772-778`).
6. Same job elsewhere.
   - "Does this view's recorded home match this value": three copies: `ByteSequenceView::matches_descriptor_home` (`:3009`), `ElementView::matches_descriptor_home` (`:3571`), and `ByteSequenceBinding::matches_shared_home` (`:190`) dispatching to both. Also the `matches_bound_descriptor_home` pair (`:3018`, `:3587`) and an inline copy of the same three-field comparison in the closure at `:561-563`.
   - `with_descriptor_home` exists three times: `:109`, `:2966`, `:3482`. `detached_descriptor` twice: `:3003`, `:3561`.
   - Unsigned-u64 operand fetch: `byte_sequence_unsigned_operand` (`:2898`), `count` (`:3832`), `self.unsigned_integer_value` (used at `:1261`, `:2312`). Three spellings of "read a ValueId as u64/usize".
7. Unused / one-caller (Grep content mode on `omega-rust`):
   - `relocated_descriptor(`: 1 call site, `storage_records_and_arrays.rs:1789`.
   - `retained_byte_descriptor_binding(`: 1 call site, `storage_references.rs:4148`.
   - `declared_byte_owner(`: 3 call sites, all `boundary_effects.rs` (`:1477`, `:1625`, `:1651`).
   - `shared_byte_descriptor_binding(`: 1 call site, `storage_records_and_arrays.rs:1623`.
   - `byte_binding_qualifications_hold(`: 3 call sites (`:819`, `:953` here, `boundary_effects.rs:1383`).
   - `byte_field_snapshot(`: 1 call site (`:1723`, this file).
   - `byte_field_length_home(`: 1 call site (`:1230`, this file).
   - `byte_field_read(`/`byte_field_length(`: `byte_field_length(` 1 call (`:1231`); `byte_field_read(` 2 calls (`:1317` here, `embedding.rs:901`).
   - "single/shared/canonical" comments: none in this file's ranges; nothing to count.
8. Stated limits. `UnsupportedSemanticVariant("bounded byte owner requires its byte-sequence storage representation")` (`:1074`) and `("...mutable byte-sequence storage representation")` (`:1107`).
9. Fail-open risks.
   - `ByteSequenceView::matches_descriptor_home` (`:3009-3016`): when the view has no recorded `descriptor_home`, it returns `value.path.is_empty()` and never compares identity or type. So a bare view (for example the one inserted by concatenation at `:1178-1182`, `ByteSequenceView::new` with `descriptor_home: None`, `:2961`) matches any value with an empty path in `matches_shared_home`, `byte_descriptor_length` (`:689`), `reborrow_byte_sequence_binding` (`:980-982`). Which wrong program: a different empty-path value paired with a home-less view. Status: open for the callers; I did not trace whether every use pairs the view with its own `structural_values` row. The same fallback is in `ElementView::matches_descriptor_home` (`:3571-3576`).
   - `ByteSequenceView::replace_byte` copy-on-write branch (`:3090-3097`) does `*self = Self::new(storage)`, which sets `descriptor_home: None` and `original_array: None`, while the unique-owner branch (`:3084-3089`) keeps `descriptor_home` and only clears `original_array` (`:3098`). After a shared-backing write the view therefore falls back to the home-less match above. Status: read, effect on callers not traced (open).
   - `prepare_byte_descriptor_argument` `Ok(None)` returns (`:865-891`): the caller decides what `None` means; I did not read the caller (`engine_calls.rs`/`engine_terminators.rs`). Open.
   - `byte_descriptor_source` last fallback (`:649-673`) accepts the first call-stack place whose structural value has same identity/path/type. Stated by structure only (no comment). Open.

---

## 2. `concatenation`, `equality`, `predicate`, `store` (1143-1206, 1744-1762, 2221-2292)

1. Purpose. Four byte-sequence operations: concatenate two operands into a new bounded-owned value, compare two operands for equality, evaluate a byte predicate, store a source sequence into a bounded-owned destination.
2. Shape. `execute_byte_sequence_concatenation` (`:1151`), `execute_byte_sequence_equal` (`:1194`), `execute_byte_sequence_predicate` (`:1751`), `execute_byte_sequence_store` (`:2231`). All four are dispatched from `engine_execution.rs` through a macro table (`:2827`, `:2845`; concatenation as `flow`, store as `advance`; grepped). Equal/predicate use `byte_sequence_operand_view` (`:1686`) and `store_scalar`.
3. Inputs/outputs. Concatenation mints a new `TerminalStructuralValue` via `local_structural_identities.allocate()` (`:1173`) and inserts an `Immutable` binding at `result.place` (`:1178-1182`). Store replaces a field cell (`:2280-2283`) or owner bytes (`:2285`) with `bytes.owned_snapshot()`.
4. How it decides. Capacity: result type must be `ByteSequence(BoundedOwned{capacity})` and `left.len()+right.len() <= capacity` (`:1157-1168`). Store: `source != destination`, `bytes.len() == length` operand and `length <= capacity` (`:2275`). Keyed on declared capacity in the type and the length value operand.
5. Cases by shape. Store accepts destination binding `MutableField`, `MutableOwner`, or `Immutable` (goes through `mutable_byte_sequence_storage`, `:2253`); `Representation` and `MutableArray` destinations are refused (`:2262-2265`) and a `MutableByteSequenceStorage::Array` result is refused (`:2287`).
6. Same job elsewhere. The Immutable arm of the store (`:2252-2261`) and `owned_byte_sequence_storage` (`:2693`) both derive capacity from `BoundedOwned` in the type.
7. Unused. `execute_*` are dispatched by macro (counts above); no separate grep needed.
8. Stated limits. None.
9. Fail-open. none seen. (Concatenation does not check `result.multiplicity` or claims; `owned_byte_sequence_storage` later checks `result.claims` empty and `qualification_establishments` empty at `:2717-2724`. Not traced for concatenation itself.)

---

## 3. `field_index_store` and `field_store` (1207-1644)

1. Purpose. Operations on byte-sequence fields of records: read the live length of a field, store one byte, read one byte, store a whole sequence into a bounded-owned field, store a view into a shared-view field, and the shared resolver that finds the field cell and its declared carrier.
2. Shape. `execute_structural_byte_sequence_field_length` (`:1216`), `..._field_byte_store` (`:1245`), `..._field_read` (`:1291`), `byte_field_length_home` (`:1352`), `execute_structural_byte_sequence_field_store` (`:1369`), `resolve_structural_byte_sequence_field` (`:1419`, requires `BoundedOwned`), `readable_byte_field` (`:1439`), `execute_structural_byte_view_field_store` (`:1464`), `resolve_structural_byte_field_carrier` (`:1528`), private `resolve_byte_field_carrier` (`:1541-1642`).
3. Inputs/outputs. Reads `program.machine(current_machine)`, `live_claims`, `structural_values`, `element_view_values`, `values`; returns `(StructuralByteSequenceRuntimeField{parent, namespace, field}, ByteSequenceCarrier)`; the store ops write `structural_byte_sequence_fields` (`:1411`, `:1516`, `:1278-1286`) and call `invalidate_stored_provenance` (`:1287`, `:1415`).
4. How it decides. `resolve_byte_field_carrier` refuses if the root place is claimed (`:1553`), then picks access from the machine's parameter declaration or an operation-result place (`:1556-1576`), asks `access_covers(access, requested)` (`:1619`), resolves the projection, finds the field by `StructuralFieldId` (`live_field_by_id`, `:1629`) and takes its declared carrier (`:1630-1633`). Field identity is by `StructuralFieldId`, not names. Byte store checks `scalar_type == u8`, `live_length <= capacity`, `byte_index < live_length` and stored length equals `live_length` (`:1271`, `:1283`).
5. Cases by shape.
   - `readable_byte_field` (`:1448-1460`): `BoundedOwned` ok; `BorrowedView{access: None | Shared | Mutable}` ok with no capacity; `BoundedView` ok unless `WriteOnlyBorrow`; everything else `malformed()`.
   - `execute_structural_byte_view_field_store` (`:1474-1489`): field carrier must equal exactly `BorrowedView{access: Some(SharedBorrow)}`; a `None` field means store into the whole shared-readable view place (`:1491-1501`), path empty only.
   - `resolve_byte_field_carrier` element-view root branch (`:1587-1610`): requires the element type to equal the backing array element.
6. Same job elsewhere. `execute_structural_byte_sequence_field_byte_store` (`:1245`) and `execute_byte_sequence_write` (`:2839`) both write one byte of a bounded-owned sequence with identical length/index checks; one goes through a field cell, the other through `MutableByteSequenceStorage` (including `Field`, `:2872-2880`). `execute_structural_byte_sequence_field_read` (`:1291`) and `byte_sequence_read` (`:2794`) likewise.
7. Unused. See section 1 counts (`byte_field_length_home` 1 call, `byte_field_snapshot` 1 call).
8. Stated limits. None in comments.
9. Fail-open.
   - `resolve_byte_field_carrier` `length_metadata` guard (`:1577-1582`) only refuses a `WriteOnlyBorrow` root path containing `Referent`; other WriteOnly combinations rely on `access_covers` (`:1619`). Traced within the function: requested access is WriteOnlyBorrow for `requires_mutation` or write-only length metadata, so `access_covers(access, requested)` decides. Not a fail-open in my reading.
   - Element-view root branch (`:1587-1594`): the `requires_mutation && access != MutableBorrow` refusal applies only if `parameter_root` is true; for a non-parameter (operation result) element-view root, mutation is not checked there. Status: open (the result place would be a mutable view only if its producer made it so; not traced).

---

## 4. `operands` (1645-1743)

1. Purpose. Turn a `ByteSequenceOperand` (either `Whole{source, length}` or `Field{source, field, ..}`) into a concrete `ByteSequenceView` snapshot, confirming the operand's declared length value equals the real length.
2. Shape. `fixed_byte_array_observation` (`:1657`), `byte_sequence_operand_view` (`:1686`).
3. Inputs/outputs. Reads the structural type tree, `scalar_array_snapshot`, `values`; returns a view. For a fixed byte array it builds a new `ByteSequenceStorage` by reading each scalar element (`:1675-1683`).
4. How it decides. `Whole` requires `SharedBorrow` access (`:1692`); a fixed-size byte array type (`FixedArray` with `fixed_byte_extent`) takes the array path, otherwise the descriptor path (`:1695-1718`). After either path the length operand must be a u64 equal to `bytes.len()` (`:1732-1739`). Field path checks capacity if known (`:1724-1728`).
5. Cases by shape. `FixedArray` with a fixed byte extent versus everything else (`:1669-1673`, returns `Ok(None)` when not).
6. Same job elsewhere. The array-path length check at `:1698-1702` duplicates the final check at `:1734-1739`.
7. Unused. `fixed_byte_array_observation(`: 2 call sites (`:1695` here; `engine_structural.rs:2294`). `byte_sequence_operand_view(`: used by concatenation, equal, predicate (read, `:1163`, `:1200`, `:1757`).
8. Stated limits. None. 9. Fail-open: none seen.

---

## 5. `read` (1763-1984) and `storage` (1985-2220)

1. Purpose. `TerminalByteRead` is a borrowed, length-aware byte reader over three kinds of backing: contiguous `&[u8]`, a slice of `ScalarCell`s, or an element-view representation read through a trait object. `ByteSequenceStorage` is the accounted `Vec<u8>` that backs every owned byte sequence; it reserves bytes from an `EvaluationInvocationAccount` lease before allocating.
2. Shape. `enum ByteStorage<'a> {Contiguous, Cells, Representation{reader: &dyn RepresentationByteReader, view}}` (`:1768`), `trait RepresentationByteReader` (`:1777`, implemented only for `TerminalExecution` at `:1785`, reads via `read_element_view_scalar` as u8), `struct TerminalByteRead` (`:1808`, constructors `representation` `:1842`, `contiguous` `:1851`, `cells` `:1857`; `len`, `is_empty`, `read`, `copy_to`). `struct ByteSequenceStorage {bytes, lease, account}` (`:1990`) with `zeroed` (`:2003`), `copy` (`:2027`), `bytes`, `bytes_mut`, `account`, `replace` (`:2046`), `from_values` (`:2083`).
3. Inputs/outputs. `zeroed` reserves `account.reserve_storage(0, length)` then `try_reserve_exact`; errors map to `Storage(..)` or `StorageAllocationFailed` (`:2008-2018`). `replace` grows the lease before reserving and shrinks it back on reserve failure (`:2046-2066`), shrinks after a shorter replacement (`:2070-2079`).
4. How it decides. `TerminalByteRead::cells` checks every cell is a u8 scalar before exposing the window (`:1860-1864`). `PartialEq` (`:1821`) first compares equal `Cells` slices with `==`, else length and byte-by-byte; any read error makes the comparison false (`:1831-1835`).
5. Cases by shape. Three storage kinds only (`:1768-1775`).
6. Same job elsewhere. `ByteSequenceBinding::snapshot` (`:157`) re-copies through `TerminalByteRead` into storage; `ByteSequenceStorage::from_values` (`:2083`) and `ByteSequenceStorage::copy` (`:2027`) are two ways to build storage (iterator versus slice). Element-view byte reading exists both as `TerminalByteRead::Representation` (`:1787`) and `RepresentationView` storage-byte reading (`:5128`).
7. Unused. `ByteSequenceStorage::zeroed|copy|from_values` and `.replace(b..)`: Grep count (`\bByteSequenceStorage::(zeroed|copy|from_values)\(|\.replace\(b` over `psi/interpreter`) = 42 hits in 10 files, 16 in this file (mostly tests). `ByteSequenceStorage::replace(` itself: only seen in the test at `:2191-2197` in this file; I did not grep the method name outside this file, so "no non-test caller" is suspected, not established.
8. Stated limits. Header says it reserves "live byte capacity before allocation or copy-on-write detachment" (`:5-7`). Read: `replace_byte`'s COW path calls `ByteSequenceStorage::copy` -> `zeroed` -> `reserve_storage` (`:3091`, `:2027`, `:2008`), consistent. Test `copy_on_write_reserves_the_overlapping_window_before_mutation` (`:2146`) pins it.
9. Fail-open. none seen.

---

## 6. `subslice` (2293-2540)

1. Purpose. Build a narrowed borrowed byte view (shared or mutable) over a window of a source byte descriptor.
2. Shape. `execute_byte_sequence_subslice` (`:2303-2407`), `narrow_byte_sequence_binding` (`:2408-2538`, also called from `execute_establish_element_view`, `:4232`, `:4371`).
3. Inputs/outputs. Reads `start/end/length` operands, source descriptor and binding; mints a staged identity, inserts the new binding and value at `result.place`, calls `track_affine_result` (`:2388-2406`).
4. How it decides. A long single boolean (`:2331-2362`): result place must be declared as this operation's result in the verified machine (`:2331-2335`), result type must be a borrowed byte view, `length == source_length`, `start <= end <= length`, `descriptor_reborrow_matches`, mutable needs no source qualifications, multiplicity must be non-linear, no result qualifications/claims, neither place claimed, no affine discard on the result place. A pre-existing value at `result.place` is allowed only if it is the exact previously minted value with the same home (`:2365-2387`), i.e. the operation may be re-run on the same place.
5. Cases by shape. `narrow_byte_sequence_binding` arms (`:2445-2530`): shared Representation, shared Immutable, MutableArray (shared or mutable), MutableOwner only when the window is the whole sequence (`:2516-2523`); MutableField and every other pair refused.
6. Same job elsewhere. `ByteSequenceView::subslice` (`:3102`) and `ElementView::subslice` (`:3811`) are the two underlying window operations; `execute_element_view_subslice` (`:4827`) is the element-view counterpart to this function with a near-identical precondition list.
7. Unused. `narrow_byte_sequence_binding(`: 3 call sites (`:2390`, `:4232`, `:4371`; read).
8. Stated limits. None. 9. Fail-open. none seen. (The "already holds the same value" arm at `:2370-2386` accepts a repeat; the condition requires identity-minted equality and matching home, so it is traced within the function.)

---

## 7. `write` (2541-2909)

1. Purpose. Resolve where a mutable byte sequence's bytes live (owner frame, record field, or backing array window), and perform length, read and write on it.
2. Shape. `enum MutableByteSequenceStorage {Owned(ByteOwnerHome), Field(..), Array{backing, offset}}` (`:2556`), `mutable_byte_sequence_storage` (`:2566-2691`), private `owned_byte_sequence_storage` (`:2693-2770`), `byte_sequence_length` (`:2772`), `byte_sequence_read` (`:2794`), `execute_byte_sequence_write` (`:2839`), `byte_sequence_unsigned_operand` (`:2898`).
3. Inputs/outputs. Reads machine declarations, `live_affine_frontier`, `byte_sequence_values`, `structural_values`; returns `(storage handle, current length)`. Writes go through `byte_owner_bytes_mut`, the field cell's `immutable_mut`, or `stored_byte_array_write`.
4. How it decides. Whether a place is owner or borrow is decided from the declared parameter or result place: Owned parameter or operation-result place of a bounded-owned type goes to `owned_byte_sequence_storage` (`:2574-2587`). Otherwise a producer operation must have `result_access == MutableBorrow`, Affine multiplicity, no claims or establishments, and the result place's whole-path discard in the live frontier (`:2592-2602`); a parameter must be `MutableBorrow` with Unrestricted multiplicity or a view-descriptor parameter (`:2613-2619`). Write requires `claimed_length == current_length` and `index < current_length` (`:2860`).
5. Cases by shape. `owned_byte_sequence_storage` for a non-parameter place requires the producer be `EstablishByteSequenceConcatenation` (`:2717-2720`): "only concatenation results are owned storage" (inferred from the match, no comment). `MutableByteSequenceStorage::Array` is rejected by `execute_byte_sequence_store` (`:2287`).
6. Same job elsewhere. Section 3 (field byte store/read) and section 1 `exclusive_owner_bytes` (`:1031`) which also returns bytes with a capacity bound.
7. Unused. `byte_sequence_length(`: 2 non-definition call sites in the interpreter (`:5053` here, `engine_structural.rs:2267`); `byte_sequence_read(`: 1 interpreter call (`engine_structural.rs:2271`). Other hits are unrelated functions of the same name in `omega/.../selection_views_and_bytes.rs`.
8. Stated limits. None. 9. Fail-open. none seen.

---

## 8. `ByteSequenceView` and friends (2911-3121)

1. Purpose. A reference-counted window over `ByteSequenceStorage`, with an optional recorded descriptor home and an optional record of the original array it was copied from.
2. Shape. `struct ByteSequenceView {backing: Arc<ByteSequenceBacking>, window: Range<usize>, descriptor_home: Option<TerminalStructuralValue>, original_array: Option<OriginalByteArrayWindow>}` (`:2915`); `struct ByteSequenceBacking {storage, exposure_namespace: Option<u64>}` (`:2930`). `PartialEq` is `Arc::ptr_eq` plus equal window (`:2935-2938`), not byte equality.
3. Inputs/outputs. `new` takes a storage and calls `new_exposure_namespace()` (`:2957`). `subslice` shares the backing and clears `descriptor_home` (`:3102-3120`); `replace_byte` writes in place only if the `Arc` is unique, else copies the window (`:3071-3100`).
4. How it decides. Home match by recorded home or empty path (see section 1 risk). `into_account` keeps the view if the storage's account is the same invocation, otherwise copies (`:3049-3065`).
5. Cases by shape. None. 6. Same job elsewhere. See section 1.
7. Unused / one-caller (Grep content mode over `psi`):
   - `shares_backing(`: 3 external calls (`boundary_storage_inputs.rs:722`, `:908`; `boundary_values.rs:953`).
   - `retained_payload_bytes(`: 1 (`boundary_values.rs:958`).
   - `original_byte_allocation(`: 1 (`boundary_storage_inputs.rs:699`).
   - `storage_address_coordinate(`: 1 (`storage_primitives.rs:5978`).
   - `into_account(`: 3 (`boundary_effects.rs:2695`, `:2822`, `:2829`; the last two are in test-looking code, inferred).
   - `owned_snapshot(`, `original_array(`, `with_original_array(`: combined Grep count over `omega-rust` of the eight method names `storage_address_coordinate|original_byte_allocation|retained_payload_bytes|shares_backing|into_account|owned_snapshot|original_array|with_original_array` = 22 hits in 9 files, 2 of them in this file; not split per name for the last three.
8. Stated limits. None. 9. Fail-open: see section 1 (home-less view match; COW dropping `descriptor_home`).

---

## 9. `element_views` core (3124-3830)

1. Purpose. `ElementView` is a borrowed window over a collection of elements, with four possible backings: a structural collection (an array in structural storage), an immutable sequence of structural values, a typed reinterpretation of another element type, or a byte-representation reinterpretation.
2. Shape. `ImmutableStructuralSequence {elements: Vec<TerminalStructuralValue>, _lease}` (`:3130`, `from_values` `:3136` reserves lease for `len * size_of::<TerminalStructuralValue>()` and rejects any element with a non-empty `path`, `qualifications` or `projected_qualifications`, `:3161-3166`). `ElementViewHome {storage: PrimitiveStorageHome, path}` (`:3214`). `CollectionStorage {collection, home, view}` (`:3219`). `ElementView {diagnostic_source, backing, window, home, descriptor_home, access}` (`:3242`). `enum ElementViewBacking {Structural, ImmutableSequence, Typed, Representation}` (`:3252`). Methods: `retain_storage`/`retain_projection_storage` (`:3260-3321`), `retained_bytes` (`:3323`), `captured_storage_home` (`:3334`), `structural` (`:3377`), `immutable_sequence` (`:3394`), `structural_projection` (`:3410`), `project_array` (`:3487`), `lends_same_array_storage` (`:3537`), `matches_*` (`:3571`, `:3587`), `complete_scalar_array_frame` (`:3601`), `owned_multiplicity` (`:3636`), `lends_structural_storage` (`:3644`), `captured_structural_index` (`:3660`), `initialized_empty_scalar_array` (`:3690`), `scalar_storage` (`:3732`), `subslice` (`:3811`).
3. Inputs/outputs. Reads structural types, `PrimitiveStorage`, `TerminalExecution`; produces projections `(collection value, path)`, storage handles or retention sets (`RetainedStorage`).
4. How it decides. Access is checked with `StructuralAccess::admits_referent` (`:3451`, `:3696`, `:3750`, `:3817`); positions are bounds-checked against `self.len()` and window offsets with `checked_add`. Identity: `lends_same_array_storage` compares `opaque_identity`, `path`, `structural_type`, window and home (`:3547-3558`). `captured_structural_index` matches by `opaque_identity` and path prefix (`:3664-3687`).
5. Cases by shape.
   - `retain_projection_storage` projection handling (`:3291-3319`): only empty projection, `Element`, or `Exact(FixedIndex)`; `_ => return` for every other projection, so retention records nothing for other segment kinds.
   - `structural_projection` for `ImmutableSequence`: read-only and shared (`:3421-3423`), `path` must start with a `FixedIndex` (`:3424-3428`). For `Structural`: empty path requires the whole literal array (`:3454-3463`).
   - `project_array` requires a `FixedArray` with `ArrayExtent::Literal` (`:3509-3515`).
6. Same job elsewhere. `lends_structural_storage` (`:3644`) and `lends_same_array_storage` (`:3537`) both ask "does this view lend that storage" with different keys. `ElementView::subslice` (`:3811`) versus `ByteSequenceView::subslice` (`:3102`).
7. Unused / one-caller (Grep content over `omega-rust`; the Rust-psi `captured_storage_home` hits in `06_checked-trees-to-lowered-psi` are a different function with the same name and are not counted):
   - `lends_same_array_storage(`: 1 (`storage_references.rs:4426`).
   - `captured_structural_index(`: 2 (`storage_references.rs:602`, `:883`).
   - `lends_structural_storage(`: 1 (`engine_structural.rs:1580`).
   - `complete_scalar_array_frame(`: 1 (`boundary_inputs.rs:1265`).
   - `owned_multiplicity(`: 2 (`engine_calls.rs:393`, `boundary_effects.rs:850`).
   - `initialized_empty_scalar_array(` (method on the view): 4 calls on `loan.` (`storage_records_and_arrays.rs:3651`, `:4740`; `storage_primitives.rs:3198`; plus the `execution.` variant at `:3725` here, which is a different method on `TerminalExecution`).
   - `ElementView::captured_storage_home(`: interpreter calls `storage_references.rs:857`, `:3729`, `:3731`, `:3957`, `:3965`; `storage_records_and_arrays.rs:3108`; here `:4456` (7).
   - `retire_unrestricted_views(`: 1 (`storage_primitives.rs:4533`).
8. Stated limits. None in comments. 9. Fail-open: `ImmutableSequence` retention (`:3278-3285`) silently skips positions where `elements.element(position)` is `None` (`if let Some`); window is bounds-limited by construction so not reachable from a malformed program (inferred, open).

---

## 10. `element_views` operations (3832-4917)

1. Purpose. Execute the element-view operations: establish (shared or mutable) a view over an array/byte array/representation, write one element, read the length, read one element, take a sub-window.
2. Shape. `count` (`:3832`), `element_view_argument` (`:3854`), `element_view_at_resolved_home` (`:3875`), `retire_unrestricted_views` (`:3905`), `execute_establish_element_view` (`:3931-4424`, ~490 lines), `element_collection_storage` (`:4426`), `resident_element_storage` (`:4468`), `element_scalar_field` (`:4521`), `scalar_field_in_projection` (`:4536`), `read_structural_element` (`:4568`), `write_structural_element` (`:4619`), `execute_element_view_write` (`:4669`), `execute_element_view_length` (`:4729`), `read_element_view_scalar` (`:4775`), `execute_element_view_read` (`:4807`), `execute_element_view_subslice` (`:4827`).
3. Inputs/outputs. Establish mints a new `TerminalStructuralValue` (`:4130`, `:4206`, `:4293`, `:4350`), inserts into `byte_sequence_values` or `element_view_values` plus `structural_values`, calls `track_affine_result` (`:4268`, `:4303`, `:4422`) except in the `WholeRepresentation` branch (`:4157-4158`, which returns without `track_affine_result`). Reads and writes dispatch on `ElementViewBacking` (`:4708-4725`, `:4789-4800`).
4. How it decides.
   - Producer/declaration cross-check by operation id and place kind (`:3981-3986`, `:4044-4052`); the establishing op's declared source place must equal the runtime source place.
   - Establish descriptor must be `ElementView{element == view_element}` or a borrowed byte carrier whose `element_scalar` is `u8` (`:3996-4008`).
   - Region geometry is decided by `terminal_semantics::element_view_shape_region_geometry` and `collection_view_descriptor` (not read, in `terminal_semantics`), with branches `WholeRepresentation` (`:4063`), `RepresentationBytes` (`:4203`), source FixedArray of a different element type (`:4271`, typed view), else a collection by extent `Fixed|StoredLength|LiveLength` (`:4306-4348`).
   - Subslice (`:4827`) precondition list (`:4855-4889`) mirrors byte subslice.
5. Cases by shape.
   - `execute_establish_element_view` accepts the `FixedRange` suffix only with `region == WholeCollection` (`:4023-4026`) and only for a `FixedArray` of `u8` with literal length (`:4185-4202`).
   - Typed view: only `FixedArray` with `ArrayExtent::Literal` source of a differing element type (`:4271-4275`); the descriptor must be `ElementView` (`:4277`).
   - `LiveLength{capacity}` extent is accepted only for a `ByteSequence` descriptor, and then the collection length used is the declared `capacity` (`:4342-4346`).
   - `execute_element_view_subslice`: `Linear` multiplicity refused (`:4839`).
   - `execute_element_view_write`: `ImmutableSequence` backing refused (`:4709`).
6. Same job elsewhere. `execute_element_view_subslice` (`:4827`) and `execute_byte_sequence_subslice` (`:2303`): same precondition list shape (place declared as op result, no claims, no affine discard at result, repeat-allowed arm `:4890-4902` vs `:2365-2387`). `write_structural_element` (`:4619`) / `write_typed_element` (`:5473`) / `write_representation_element` (`:5200`): three per-backing writers. `read_structural_element` (`:4568`) / `read_typed_element` (`:5407`) / `read_representation_element` (`:5108`): three per-backing readers.
7. Unused. `element_view_argument(`/`element_view_at_resolved_home(`: used within this file at `:4088`, `:4321`, `:4697`, `:4764`, `:4843` (read); not grepped outside. `execute_*` are dispatched via the macro table (`engine_execution.rs:2867` for subslice, grepped; the other four dispatch rows I did not grep).
8. Stated limits. `.in_reference_formation("...")` labels name the failing stage only. None else.
9. Fail-open.
   - `execute_element_view_subslice` `(view.len() != 0 && result.projected_qualifications.iter().any(|row| !source_value.projected_qualifications.contains(row)))` (`:4868-4872`): the "result's projected qualifications must be among the source's" check is skipped when the source view is empty. What gets through: an empty-view subslice may claim projected membership qualifications that the source did not carry (they must still be `is_element_membership`, `:4874-4877`). Status: open (no comment).
   - `execute_establish_element_view`, `WholeRepresentation` branch skips `track_affine_result` (`:4158`); other branches call it. Status: open; not traced whether a `WholeRepresentation` view result is always Unrestricted (the function pins `multiplicity` to Unrestricted for `EstablishElementView` and Affine for `EstablishMutableElementView`, `:3935-3962`, and `whole_representation_view` refuses non-shared `source.access()`, `:5179`, so Affine appears not to reach it for the representation case in practice; inferred).
   - `retire_unrestricted_views` (`:3905-3929`) removes every byte/element view place that has no affine discard and no claim, including from `structural_values`, and records `retired_structural_homes`. It is a drop, not an accept; I do not call it fail-open.

---

## 11. `representation` (4919-5293)

1. Purpose. Present a run of bytes (a byte array, an immutable byte sequence, or another element view) as an array of structured elements with a different layout, reading and writing scalar leaves by byte offset.
2. Shape. `RepresentationView {storage: ByteStorage, start, length, layout: RepresentationElementLayout}` (`:4938`), private `enum ByteStorage {Array{storage, offset}, Immutable(ByteSequenceView), Typed{source: ElementView, layout}}` (`:4945`; separate type from `read::ByteStorage`), `scalar_position` (`:4977`), `representation_view` (`:5015`), `read_representation_element` (`:5108`), `representation_storage_byte` (`:5128`), `whole_representation_view` (`:5173`), `write_representation_element` (`:5200`), helpers `scalar_width` (`:5236`), `bit_carrier` (`:5248`), `decode_scalar` (`:5255`), `encode_scalar` (`:5276`).
3. Inputs/outputs. Reads stored byte arrays (`stored_byte_array_read`) or the typed source view; writes `stored_byte_array_write` byte by byte (`:5225-5231`), little-endian (`bits |= byte << (byte_index*8)`, `:5123`, `:5229`).
4. How it decides. Layout leaf found by `leaf.path == path && leaf.scalar_type == expected` (`:4991`) - path equality and scalar type, keyed on structure not name. Window checks use stride (`layout.size`), `is_multiple_of(stride)` (`:5038`), `checked_*`.
5. Cases by shape.
   - `scalar_width` (`:5236-5247`): integers that are not address types and have 8, 16, 32 or 64 bits; binary32; binary64; everything else (bool, address ints, other floats) `malformed()`.
   - `representation_view` source: `ByteSequence` collection with `MutableArray` binding or `Immutable` + `SharedBorrow` (`:5057-5071`); otherwise an array root path (`:5072-5099`).
   - `write_representation_element` only for `ByteStorage::Array` (`:5215`); `Immutable` and `Typed` storage are not writable.
   - `whole_representation_view` requires the source be shared and `source_layout.size` a multiple of `target_layout.size` (`:5179-5183`).
6. Same job elsewhere. `decode_scalar`/`encode_scalar` (`:5255`, `:5276`) and `typed::reinterpret` (`:5359`) both convert a scalar between views using `terminal_semantics::scalar_reference_view_value`; `reinterpret` additionally falls back to `ScalarRepresentationViewPair::classify` (`:5366`) and accepts Boolean; the representation pair does not.
7. Unused. `representation_view(`: 1 call (`:4252`); `whole_representation_view(`: 1 call (`:4128`). Both read, not grepped outside this file.
8. Stated limits. None. 9. Fail-open: none seen. (`scalar_position` takes `.find` of the first matching leaf, `:4987-4992`; duplicates are not rejected here, unlike `typed::selected`, which rejects more than one match, `:5347`.)

---

## 12. `typed` (5294-5536)

1. Purpose. View an array of one element type as an array of a different element type, converting each scalar leaf on read and write.
2. Shape. `TypedElementView {collection, home, relation: ElementRepresentationRelation}` (`:5306`), `selected` (`:5322`), `checked_value` (`:5354`), `reinterpret` (`:5359`), `typed_element_view` (`:5384`), `read_typed_element` (`:5407`), `write_typed_element` (`:5473`).
3. Inputs/outputs. Relation comes from `terminal_semantics::element_representation_relation`; reads/writes go to a structural scalar field cell, a primitive home, or resident element storage.
4. How it decides. `selected` pairs source and target leaves by zip position and requires exactly one pair whose target `path` and `scalar_type` match (`:5340-5349`). `checked_value` requires scalar type equality, `terminal_scalar_belongs_to_type`, and integer bounds when the leaf has `bounds` (`:5354-5357`). Both read and write check value before and after reinterpretation (`:5463-5468`, `:5488-5494`).
5. Cases by shape. Writes require `access == MutableBorrow` (`:5480`).
6. Same job elsewhere. `read_typed_element` (`:5407`) repeats the three-way storage choice of `read_structural_element` (`:4568`): scalar field cell, primitive home, resident element storage.
7. Unused. `typed_element_view(`: 1 call (`:4280`), `read_typed_element(` 1 (`:4791`), `write_typed_element(` 1 (`:4711`), all in this file (read, not grepped outside).
8. Stated limits. None. 9. Fail-open: none seen. (`write_typed_element` checks the existing stored value passes `checked_value` against the source leaf before overwrite, `:5509-5511`, only on the field-cell branch; the home and resident branches do not.)

---

## 13. Tests (read)

- `read::tests` (`:1916-1983`): 2 tests (borrow/copy semantics, malformed cells).
- `storage::tests` (`:2101-2219`): 4 tests using `BuildEvaluationSponsor` (backing retained once until last owner drops; COW reservation at limits 5 and 6; staged growth shares budget; failed scalar copy retires reservation).
- `element_views::tests` (`:5539-5597`): 2 tests on `matches_descriptor_home` / `matches_bound_descriptor_home` / `same_descriptor_home`. They pin that qualification changes do not change the home but a different `opaque_identity`, path or type does.
- No tests in this file for the op executors (`execute_*`), only for reader, storage and home matching.

---

## Coverage

- Read in full: `storage_bytes_and_views.rs` 1-5598 (5,598 lines).
- Skimmed: none.
- Not read: none.
- Total 5,598 = lines_read 5,598 + lines_not_read 0.

Greps run (Grep tool, content or count mode, over `omega-rust` unless noted): `\.relocated_descriptor\(`; `retained_byte_descriptor_binding\(`; `declared_byte_owner\(`; the eight-name view-helper pattern (count 22 in 9 files); the lends/captured/initialized/retire pattern; `(shares_backing|retained_payload_bytes|storage_address_coordinate|original_byte_allocation|into_account)` over `psi`; `fixed_byte_array_observation|byte_field_snapshot|byte_field_length_home|shared_byte_descriptor_binding|byte_binding_qualifications_hold|execute_byte_sequence_store|execute_structural_byte_view_field_store`; `byte_sequence_length|byte_sequence_read|byte_field_read|byte_field_length`; `ByteSequenceStorage::(zeroed|copy|from_values)|.replace(b` over `psi/interpreter` (count 42); dispatch names in `engine_execution.rs`.

## Five things a maintainer most needs to know about this unit

1. Every mutable byte descriptor is a `ByteSequenceBinding` variant (`MutableArray`/`MutableField`/`MutableOwner`) that is re-validated against the presented value each time it is used (`validate_mutable_referent`, `:219-356`); the bytes are not stored in the binding, they are looked up again from the array, field cell or frame each time (`:1031-1089`).
2. Whether a view "belongs" to a value is decided by a recorded `descriptor_home`; a view with no home matches any value with an empty path (`:3009-3016`, `:3571-3576`). `subslice` drops the home (`:3117`, `:3826`) and the copy-on-write branch of `replace_byte` also drops it (`:3096`), so the empty-path fallback is reached in ordinary flows.
3. `execute_establish_element_view` (`:3931-4424`) is one ~490-line function covering four backings (structural collection, representation bytes, whole representation, typed) and byte versus element descriptors; the `WholeRepresentation` early return skips `track_affine_result` (`:4157-4158`).
4. Byte storage is accounted: `ByteSequenceStorage` reserves from the invocation lease before allocating or growing, and refunds on failure (`:2003-2081`); `ByteSequenceView` shares one `Arc` backing and charges it once until the last view drops (test `:2122`).
5. Subslice, write and read each exist in two parallel versions, one for byte sequences and one for element views, with near-identical precondition lists and per-backing duplicates (section 10 item 6); there is no shared helper between them in this file.

## What I am unsure about

- Whether every use of `matches_descriptor_home` / `matches_shared_home` pairs the view with its own `structural_values` row, which decides whether the empty-path fallback can accept a mismatched value (not traced past this file).
- What `prepare_byte_descriptor_argument`'s `Ok(None)` returns (`:865-891`) cause in the caller (`engine_calls.rs`/`engine_terminators.rs` not read in this pass).
- Whether `ByteSequenceStorage::replace` has any non-test caller (I did not grep that method name outside this file).
- Behaviour of `terminal_semantics` functions that decide region geometry (`element_view_shape_region_geometry`, `collection_view_descriptor`, `descriptor_reborrow_matches`, `byte_qualification_predicates_in`): called here, not read.
- Dispatch rows for `execute_establish_element_view`, `execute_element_view_write`, `_length`, `_read` were not grepped; only the subslice, store, concatenation and view-field-store rows were.
