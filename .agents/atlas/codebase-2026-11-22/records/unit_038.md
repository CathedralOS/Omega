# Record unit_038 (area 08-psi-program)

Checkout `4f3c255d05599c11b1ea8ac668635f15f2a6eb86`. Paths are relative to `source-rust/subsystems/compiler/psi/artifact/` (written `A/`). All 47 files were read in full. I did not read the crate map beyond its first section. Counts in "Same job elsewhere" and "Unused" come from the Grep tool in count or content mode, as stated.

Shared vocabulary used below: "wire" = canonical byte encoding of Psi; `Writer`/`Reader` = the byte cursors in `declaration-encoding/src/wire.rs`; `Field<T>` = the per-field encoder/decoder trait (`fields.rs:21`); "tag" = the leading byte that picks a variant.

---

## 1. callable-encoding crate

### A/callable-encoding/src/reach.rs (567 lines)
1. Purpose. Writes and reads the "closed reach application" of an applied machine: the template identity and commitments, parameter kinds, a telescope of type/const/proposition/machine arguments, an optional capture, the fixed service ceiling, dependencies and per-call applications (`reach.rs:1`).
2. Shape. `encode` (`reach.rs:15`) and `decode` (`reach.rs:146`) for `Option<ClosedReachApplication>`; `encode_telescope` (`:89`) / `decode_telescope` (`:211`); `encode_operational` (`:372`) / `decode_operational` (`:456`) for base commitment, suspension and blocking dependency terms and conditional-carry rows; `encode_argument`/`decode_argument` (`:525`, `:549`); `encode_service_ceiling` (`:562`, a one-line wrapper over `writer.ids`).
3. In/out. Reads `terminal_psi::ClosedReach*` records and `psi_effect_contracts::{ScopedPermission*, SuspensionValueOrigin}`; writes bytes via `Writer`; decode returns the records.
4. Decisions. Every decision is a tag byte: parameter kind `> 5` is refused (`:82`); telescope tag 0..4 (`:213`-`:265`); machine binding tag 3 vs 4 encodes whether `upper_bound` is present (`:109`, `:240`); carry subject tag 0..2 (`:467`); acquisition source tag 0..5 (`:350`); provenance tag 1/2 (`:493`). Const arguments are round-tripped through a string atom `CanonicalConstValue::atom()`/`from_atom` (`:102`, `:218`); decode refuses an atom with no canonical value (`:221`).
5. Cases by shape. The encoder refuses a conditional-carry claim whose identity is not `Established` ("conditional carry receiving claim is unknown", `:419`) and a provenance that is `Unknown` ("acquisition join is unknown", `:427`). Closed list of provenance forms: Established or Joined only.
6. Same job elsewhere. Carry/acquisition encoding is written once here; I saw no second copy in this unit. Suspected: the same `ClosedReach*` records are re-encoded in the module codec in `A/wire/src` (not read).
7. Unused. `encode_service_ceiling`: Grep for `fn encode_service_ceiling` over `source-rust/` finds only its definition (reach.rs:562); callers inside `reach.rs` are lines 51, 123, 133 (read). `decode_telescope` is `pub`; Grep not run for external callers (suspected used from the wire crate).
8. Stated limits. None in comments beyond the doc line.
9. Fail-open. `decode_kinds` accepts any kind 0..=5 without checking it against the telescope (`:82`); no cross-check here that `parameter_kinds.len() == telescope.len()` (open: may be checked by the foundation validator in `A/wire`, not read). `decode` of `fixed`/`dependencies` does not check sortedness here (open). Otherwise none seen.

### A/callable-encoding/src/results.rs (141 lines)
1. Purpose. Wire form of a callable's stored structural result, including the input paths a borrowed (reference) result may come from.
2. Shape. `encode_function_result` (`:29`), `decode_function_result` (`:70`), `validate_reference_source_order` (`:11`), `encode/decode_projected_qualifications` (`:126`, `:137`).
3. In/out. `StructuralResultDeclaration` <-> bytes. Delegates to `psi_declaration_encoding::{places, results, access::mode}`.
4. Decisions. Reference-source origin tag 1 = parameter, 2 = invocation carrier (`:47`-`:55`, `:84`-`:90`); extent tag 1 Projected / 2 WithinRegion (`:60`, `:93`). Decode runs `validate_reference_source_order`, which requires strictly increasing unique paths and non-empty strictly-increasing alternatives (`:14`-`:25`).
5. Cases by shape. Two origin kinds only; any other tag is `InvalidTag` (`:88`).
6. Same job elsewhere. `structural_wire.rs:588` in `A/wire` calls `validate_reference_source_order` (grepped, content mode); the wire crate has its own `structural_result_wire` (suspected same record, not read).
7. Unused. `validate_reference_source_order`: 3 hits across `source-rust` (definition, `results.rs:122`, `structural_wire.rs:588`) plus a `use` at `structural_wire.rs:634`.
8. Limits. none.
9. Fail-open. none seen; decode re-validates order.

### A/callable-encoding/src/signatures.rs (142 lines)
1. Purpose. Declares the wire records for call signature pieces: value declarations, entry claims, structural parameters, argument substitutions, static-requirement dispatch, declared result lifetime, and the machine result / parameter-kind enums.
2. Shape. Uses `wire_record!`/`wire_enum!` macros (`:14`, `:47`); `Commitment` field (`:56`); `encode_dependency`/`decode_dependency` for `BoundaryLifetimeDependency` (`:72`, `:100`); `DeclaredLifetimeDependency` (`:117`); `delegated_fields!` FunctionResult -> `results.rs` (`:139`).
3. In/out. Read via macro-generated field readers. No branching beyond tags: dependency tag 0 Referent, 1 Stored (`:82`, `:105`).
4. Decisions. Tag only. `MachineParameterKind` and `ParameterKind` are two identical enums over `BoundaryParameterKind` with different error labels (`:48`-`:49`).
5. Cases by shape. Machine result: Unit/Scalar/Structural only (`:50`-`:54`).
6. Same job elsewhere. `MachineParameterKind` and `ParameterKind` duplicate each other (`:48`, `:49`).
7. Unused. Not grepped.
8. Limits. none.
9. Fail-open. none seen.

---

## 2. codec crate

### A/codec/src/canonical_artifact.rs (921 lines)
1. Purpose. Builds, serializes and parses the `PSIART` artifact envelope around a closed Psi module: semantic bytes, optimization record, optional debug map, optional publication section. It also defines the execution-code views (closed module vs. invocation prefix) that borrow or retain a module.
2. Shape. `CanonicalTerminalArtifact` (`:51`; fields `optimization_bytes`, `optimization`, `debug_bytes`, `manifest`, `publication`, `semantic_module: Arc<ClosedTerminalModule>`). `from_parts` (`:84`), `from_closed_parts` (`:94`), `with_publication` (`:132`), `to_bytes` (`:195`), `from_bytes` (`:241`). Inner module `execution_code` (`:418`): `TerminalExecutionView` (`:447`), `RetainedTerminalCode` (`:528`), `InvocationTerminalCode` (`:543`, with `from_prefix` `:563`), `TerminalProgramProduct` (`:643`), `TerminalPublication` enum (`:428`). Constants `ARTIFACT_MAGIC = "PSIART\0\0"`, format markers 3 and 4 (`:18`-`:20`).
3. In/out. Reads `ClosedTerminalModule`, `PsiOptimizationExecutionRecord`, `TerminalDebugMap`, `PublishedEntryMetadata`, `EvidenceSeals`; calls `terminal_wire::sealing::*` for manifest and prefix encoding. Produces bytes/artifact.
4. Decisions. Marker 3 = no publication section, 4 = with one (`:212`, `:247`). `from_closed_parts` refuses a module whose identities have an `invocation_scope` ("invocation code cannot enter a portable Terminal artifact", `:100`-`:105`). `from_bytes` re-encodes everything and compares: if semantic/optimization/debug bytes or the publication section differ from the input it returns `NonCanonicalSections` (`:283`, `:302`-`:315`). `with_publication` is refused if one is already set, if `metadata.validate_declarations(module)` fails, or if evidence belongs to another module identity (`:137`-`:143`). `from_prefix` accepts only blocks ending in `PendingNormalCompletion` whose last operation is that call, has a suspension crossing and a declared invocation, and whose `(operation, crossing)` has a plan with the same target in `execution.suspension_call_plans` (`:572`-`:611`); at least one pending completion is required (`:612`).
5. Cases by shape. `InvocationPrefix` handles only the "pending normal completion at block end" shape; comment `:559`: "Each missing tail belongs to a particular checked suspending call at the end of its block". Identity equality ignores the publication section (`PartialEq`, `:60`-`:67`) while the manifest includes it (checked by equal `manifest`).
6. Same job elsewhere. Envelope length framing (`encode_section_len`, `:362`) vs. separate framing in `PublicationSection` bytes (`:153`); `ClosedTerminalModule::new` identity formation duplicates `from_prefix` identity formation (`:619`-`:622` vs `closed_module.rs:33`-`:36`).
7. Unused. `TerminalPublication` enum: Grep not run. `calling_declaration_owner` not grepped. The comment at `:2` says the immutable closed module "owns semantic identity"; `closed_module()` is a second accessor to the same field (`:341`).
8. Limits. Test fixture comment `:763` "pre_namespace" binary: keeps a legacy-format artifact round-tripping (`:762`-`:807`).
9. Fail-open. `from_bytes`: a publication section with no trailing evidence bytes (`fields.remaining() == 0`) is accepted with `evidence = None` (`:289`-`:291`); no stated reason (traced: `with_publication` still validates declarations, but a producer omitting evidence is accepted; whether consumers require evidence is not read here: open). `PartialEq` for the artifact ignores `publication` (`:60`): two artifacts with different publication sections but equal manifest compare equal only if manifests equal, so the manifest covers it (traced: `with_publication` updates the manifest, `:158`).

### A/codec/src/closed_module.rs (236 lines)
1. Purpose. The seal: a `ClosedTerminalModule` keeps the module, its closed integer/callable type catalog, canonical bytes and identity together.
2. Shape. `ClosedTerminalModule` (`:14`; `module: Arc`, `types`, `bytes: Arc<[u8]>`, `identity`), `new` (`:22`), `from_semantic_bytes` (`:27`), `evidence_seals` (`:63`).
3. In/out. Delegates to `terminal_wire::sealing::{encode_closed_execution_module, decode_closed_parts, encoded_fingerprint}`.
4. Decisions. None here beyond propagating encoder errors (e.g. symbolic width refused, test `:188`).
5. Cases by shape. none.
6. Same job elsewhere. Identity formation also at `canonical_artifact.rs:619`.
7. Unused. not grepped.
8. Limits. Doc `:1`: "Closure does not show that a checker accepted the program or that its producer can be trusted."
9. Fail-open. none seen. `evidence_seals` (`:63`) takes seals on trust and stamps this module's identity without checking they belong to it (traced: caller-supplied; `with_publication` later compares `evidence.module` to the view identity, which would always match since the identity was stamped here: open).

### A/codec/src/entry_receipt.rs (73 lines)
1. Purpose. Joins a declared program entry to one canonical artifact as a receipt, and re-checks the join.
2. Shape. `prepare_program_entry_receipt` (`:41`), `rejoin_program_entry_receipt` (`:65`), `ProgramEntryTerminalReceiptError` (`:10`).
3. In/out. Reads `PreparedNominalRoot` and artifact; makes `CheckedProgramEntryTerminalReceipt`.
4. Decisions. Entry result must be `TerminalMachineResult::Unit` (`:49`); the entry must have a source declaration (`:52`-`:55`). Rejoin compares `artifact.manifest().semantic()` to the receipt's identity only (`:69`).
5. Cases by shape. Only unit-result entries ("NonUnitEntry").
6. Same job elsewhere. none.
7. Unused. `rejoin_program_entry_receipt`: Grep over `source-rust` count mode: 2 files (definition in `entry_receipt.rs`, 2 hits in `source-to-psi/pipeline/07_checked-program-to-psi/src/custody.rs`).
8. Limits. none.
9. Fail-open. none seen; rejoin does not recheck the entry machine id (open: only identity compared).

### A/codec/src/entry_receipt/application.rs (153 lines)
1. Purpose. Attaches nominal application roots (entry, callback roots, physical adapter) to a current execution view and lets a portable artifact issue the entry receipt.
2. Shape. `TerminalApplicationCustody` (`:49`) `prepare` (`:56`) / `attach` (`:75`); `TerminalCompositionRoots` (`:21`); `AttachedApplicationRoots` (`:44`); `ProgramEntryReceiptRequest::check` (`:35`); `TerminalAdmissionRoot` (`:15`, a record); `project_root` (`:135`).
3. In/out. Reads `PreparedNominalRoot`, `TerminalExecutionView`, optional artifact `Result`, `TerminalMachineProvenance`. Produces root list and a lazy receipt request.
4. Decisions. `prepare` stores nothing when both `entry_identity` and `composition` are `None` (`:61`-`:67`), so `attach` returns empty roots (`:81`-`:86`). `attach` refuses if the artifact's `custody_identity` differs from the view's (`:88`-`:95`). Keyed on identity values, not names.
5. Cases by shape. Roots: entry, optional callback roots, optional physical adapter (`:100`-`:120`).
6. Same job elsewhere. none.
7. Unused. not grepped.
8. Limits. none.
9. Fail-open. If the artifact argument is `Err` the identity check is skipped (`:88`, `if let Ok`) and `attach` still succeeds; the error is held and only surfaces when the receipt is `check`ed (`:36`-`:38`). Stated by test `canonical_artifact.rs:737` ("retains portability errors on the receipt"). Roots for an invocation view are projected even without a portable artifact: stated.

### A/codec/src/lib.rs (53 lines)
Purpose and re-exports. `pub use terminal_wire::*` (`:34`) plus the sealed module, artifact, execution-code types; comments at `:3`-`:32` explain why the decoder re-encodes and enforces canonical order. No logic.

### A/codec/src/qualified_cleanup_tests.rs (106 lines)
Ignored test (`#[ignore]`, `:9`) that needs env var `OMEGA_QUALIFIED_CLEANUP_CANONICAL_ARTIFACT`: loads a CLI artifact, adds a `DiscardRoot` action beside a `DiscardResidual` on a qualified owned root, and expects decode error "affine region cleanup overlaps a whole-root discard" (`:100`-`:105`). Walks only `Jump`, `Conditional`, `StructuralCase` terminators (`:52`-`:71`).

---

## 3. declaration-encoding crate

### A/declaration-encoding/src/access/inputs.rs (42 lines)
Record `Input: TerminalAccessViewInput` (16 fields) and encode/decode of the catalog (`:28`, `:36`) using `decode_canonical` (decode then re-encode and compare). Count of `encode_access_view_input_catalog|decode_...|encode_access_view_catalog|decode_...` over `source-rust`: 23 hits in 7 files (Grep count), consumers in psi-to-native identity code.

### A/declaration-encoding/src/access/lifecycles.rs (210 lines)
Purpose. Wire form of an access lifecycle plan (initialize, move-out, view, relocate) with its target layout input, execution (selected provider vs compiler primitive), storage (extent or run), and placement. `encode` (`:28`), `decode` (`:114`), catalog wrappers (`:10`, `:18`).
Decisions. For `View`, vacant/resident must be absent else `NonCanonicalEncoding` (`:92`-`:95`); otherwise `owned_content()` must succeed (`:97`-`:100`). Decode sets vacant/resident `None` iff operation is View (`:182`-`:191`). Tag-keyed enums everywhere.
Fail-open. none seen. The identity test (`tests/identity.rs:41`) proves removing `vacant` is refused.

### A/declaration-encoding/src/access/mod.rs (6) and mode.rs (41)
`mod.rs` lists `inputs, lifecycles, mode, views`. `mode.rs`: `Access` field and `encode/decode_structural_access`: 1 Owned, 2 SharedBorrow, 3 MutableBorrow, 4 WriteOnlyBorrow (`:21`-`:26`); `decode_structural_access_tag` takes a raw byte (`:33`). Same byte values are used by `structural_signature_wire::encode_structural_access` in the wire crate (referenced at `block_wire.rs:695`; suspected duplicate, not read).

### A/declaration-encoding/src/access/views.rs (416 lines)
Purpose. Wire form of an access view plan: carrier, schema, identity indices, a validated placement (layout entries, per-field access, reach services), and field/accessor contracts.
Shape. `encode`/`decode` (`:35`, `:141`), `encode_placement` (`:50`), `decode_placement` (`:162`; builds `PlacementSnapshot{..}.validate()`, `:212`-`:224`), `encode_access`/`decode_access` (`:279`, `:359`).
Decisions. Placement entry tag 0 At / 1 IntegerAt / 2 Bits (`:70`-`:104`). Field access tag 0 Inaccessible, 5 Region, 1 Stable, 2 External, 3 Atomic (`:281`-`:356`). Validity of the whole placement is judged by `access_admission` after decode (`:223`).
Cases by shape. The encoder writes `ObservationModel::Atomic` as 2 for a `Region` (`:297`), but the decoder accepts only 0 and 1 and rejects 2 (`:366`-`:370`, message "placed repeated observation"). So a Region with Atomic observation would encode but not decode. Open: whether `access_admission` can ever produce such a region was not traced.
Also: decode builds `AccessPermissions { read, take, write, ..Default::default() }` (`:371`-`:376`), so the `atomic` field of permissions is not read back for a Region; encode writes only read/take/write (`:299`-`:301`). Consistent but lossy for the `atomic` sub-record (stated by symmetry only).
Fail-open. none seen beyond the above.

### A/declaration-encoding/src/content_projections.rs (144 lines)
Purpose. Encodes a content-projection capacity expression (`IntervalSet` of scalar pairs, or `CountedQuantity`) with each capacity scalar as one postorder term table. `CapacityTable: TermEncoding` (`:43`). Tags 1..3 leaves (SubjectField, RuntimeScalarEmbedding, Natural), 4 Successor, 5..7 Add/Subtract/Multiply (`:63`-`:81`, `:89`-`:94`). Expression tag 1 IntervalSet, 2 CountedQuantity (`:129`-`:141`). Naturals are strings (`:75`).

### A/declaration-encoding/src/crash_routes.rs (75 lines)
Purpose. Canonical crash causes and guards. `wire_record Bucket`, `wire_enum Cause {1 Trap, 2 Abort}`, `RouteGuard {0 Truth, 1 Predicate}` (`:9`-`:19`). `crash_routes_are_canonical` (`:31`): buckets strictly increasing by cause, alternatives non-empty, strictly increasing, and `Truth` only alone. `validate_crash_route_predicates` (`:41`). `encode_crash_route_buckets` refuses non-canonical (`:58`).
Unused/shared. `crash_routes_are_canonical`: 8 hits in `A/wire/src/semantic_module.rs` (lines 242, 249, 493, 699, 771, 884, 899 plus an import at 24) plus its definition and one internal call (Grep content over `source-rust/`). So it is the shared canonical check, used in 7 call sites.

### A/declaration-encoding/src/dispatch.rs (190 lines)
Purpose. Dynamic dispatch catalog wire records: interfaces with slots (family tuple, scalar and structural formals, claim paths, result, reference regions) and tables (rows `List<OptionalId>`). Uses `List<_, N>` with a minimum element size N to bound allocation (`:19`-`:33`). `FamilyScalar {1 Integer(i128), 2 Boolean}` (`:58`-`:61`). `encode/decode_dynamic_dispatch_catalog` (`:69`, `:77`, decode is canonical-checked). Tests prove prefixes are all refused, trailing bytes refused, symbolic width refused (`:96`-`:189`). Count of `encode_callable_invocations|decode_callable_invocations|encode_dynamic_dispatch_catalog|decode_quotient_catalog` over `source-rust`: 28 hits in 9 files.

### A/declaration-encoding/src/domains.rs (251 lines)
Purpose. Wire form of a structural domain declaration, including predicate, byte predicate, content projection, establishment routes and an access policy (schema image, issuance/reader/writer caller rules, observations, reach).
Shape. `Domain` record (`:17`), `Route` enum (tags 4, 5, 6; tags 1..3 unused, `:36`-`:41`), `DomainAccess` (`:139`), `DomainObservation` (`:102`), `OptionalBytePredicate` (`:198`), `encode_structural_domain_declaration` (`:245`).
Decisions. Byte predicate encoded as its mask bit: 1 ValidUtf8, 2 NoNul, 4 AsciiOnly, 8 NonEmpty, else `InvalidTag` (`:201`-`:211`). Observation model tags 0, 1, 2 (here Atomic=2 is accepted, `:127`; contrast `views.rs:366`). Schema is rebuilt from a runtime image (`schema_reflection::SemanticSchemaGraph::from_runtime_image`, `:164`), errors as `AccessPlan`.
Same job elsewhere. Reach-service encoding (`u64 normalized_identity`) repeated at `domains.rs:156`-`:159` and `views.rs:111`-`:114`; permission records appear both as `Permissions` here (with `atomic` sub-record, `:60`-`:68`) and inline in `views.rs` (`:299`-`:301`).
Limits. Doc `:242`: omitting a rule from the identity "would let a changed permission reuse an earlier checked optimization unit."
Fail-open. none seen.

### A/declaration-encoding/src/error.rs (74 lines)
`CodecError` enum (about 33 variants) and `malformed()` helper (`:72`). `Display` prints the `Debug` form (`:66`). Imported across the codec crates.

### A/declaration-encoding/src/fields.rs (432 lines)
Purpose. The `Field<T>` trait and the macro family that makes record/enum codecs from a list of field markers: `wire_record!` (`:222`), `wire_enum!` (`:261`, supports `; refuse pat => error` for variants that must never be written), `u32_newtypes!` (`:304`), `delegated_fields!` (`:378`). Markers: `Id`, `OptionalId`, `Ids`, `U8/U16/U32/U64/Flag`, `I128`, `Bytes`, `Index`, `Text`, `Strings`, `Digest`, `Nominal`, `Proposition`, `List<K, MIN>` (`:181`), `Optional<K>`, `Path<LIFETIME_FAMILY>` (`:409`), pair tuples (`:166`).
Decisions. `List::read` bounds the count by `remaining / MIN_ELEMENT_BYTES` before reserving (`:191`, via `bounded_count`) and uses `try_reserve_exact` (`:194`). `OptionalField` stores `StructuralFieldId` as `u64` with 0 meaning None (`:87`). `label::<T>()` takes the last path segment of the Rust type name as the error label (`:47`-`:50`): a string-derived diagnostic label, not a decision.
Used count. `ResultField`: definition here (`:26`) and 8 uses in `A/wire/src/module_wire.rs:504`-`:528` (Grep content). `encode_ieee_float_value`/`decode_ieee_float_value` (scalar_types.rs): 12 hits in 4 files (definitions plus `A/wire/src/term_wire.rs` x6, `wire_fields.rs` x3, `spec_tag_tables.rs` x1; Grep count). `require_increasing` (wire.rs:290) has many callers in `A/wire/src/foundation_validation.rs` and `semantic_module.rs`.
Fail-open. none seen.

### A/declaration-encoding/src/integer_types.rs (30 lines)
`EncodedIntegerTypes(BTreeMap<IntegerType, ClosedIntegerType>)` (`:7`). `record` closes each width once, failing on symbolic widths (`:10`-`:18`); `integer()` returns an error "integer type is not retained by this closed module" for any type not recorded (`:22`-`:29`). Uses a `BTreeMap` keyed by value (not a dense id table).

### A/declaration-encoding/src/invocations.rs (268 lines)
Purpose. Wire form of callable query results ("reachable callable invocations"): roots, reached machines, invocations with targets, formal-input possibilities. Includes `InvocationSite` field (`:19`; tags 0..3, tag 3 Cleanup with an extra `u32` ordinal), `Position` (`usize` as `u32`, `:64`), and `PathSegment` enum with `UnknownContents` refused on encode (`:137`-`:140`).
Decisions. Decode goes through `decode_canonical` (`:155`). Site decode for tag != 3 uses `Site::from_tagged_identity` (`:57`).
Doc `:3`-`:5`: "Encoding it does not prove the analysis correct; an independent source validator still compares it".
Fail-open. none seen.

### A/declaration-encoding/src/lib.rs (25 lines)
Module list and re-exports.

### A/declaration-encoding/src/places.rs (266 lines)
Purpose. Encodes structural paths and place kinds. Paths for operands/owned places refuse `RepresentationView` and `AllElements` segments (`require_operand_path`, `:170`-`:180`); reference-origin paths refuse `AllElements` and add tag 9 for `Element` (`:29`-`:47`, decode refuses tag 10 `:130`); qualification paths allow all (`:232`-`:243`); declared-lifetime paths must satisfy `declared_lifetime_path_is_static` (`:261`). `StructuralPlaceKind` tags 1..8 (`:61`-`:113`, `:182`-`:222`). Decisions keyed on segment kind.

### A/declaration-encoding/src/quotients.rs (69 lines)
Quotient catalog record: representations, operations, adaptations, with relations as four strings (`Relation`, `:12`-`:17`). `Positional {1 Quotient, 2 ExactEquality}`; `License {1 Mint, 2 Input, 3 Result}` (`:49`-`:58`). `encode_quotient_catalog` re-exported at lib.rs:25. Relations are identified by string text of declaration/type names (`:13`-`:16`): a display-name style key; they are committed to the bytes verbatim.

### A/declaration-encoding/src/results.rs (89 lines)
Multiplicity (1 Unrestricted, 2 Affine, 3 Linear), qualification path/set fields, `PathQualification`, `StructuralResult`, `QualifiedScalar`, `CallResult {0 Unit, 1 Scalar, 2 Structural}`. `QualificationSet` is a `u64` id with 0 meaning the empty set (`:21`-`:37`).

### A/declaration-encoding/src/structural_fields.rs (156 lines)
IEEE format (1 Binary32, 2 Binary64) and comparison kind (1..6) codecs, `IeeeFloatStructuralField` and `ByteSequenceStructuralField` codecs, canonical structural path tags 1 Field, 2 FixedIndex, 3 Case, 4 Referent (`:98`-`:114`). Decode builds the field through constructors returning `PropositionError` (`:60`, `:79`).

### A/declaration-encoding/src/terms/content.rs (237 lines)
Content (resource-ownership) records: entry claims, partition composition, conservation guarantees, reshuffles, derivations, body requirements and substitutions, address transport, structural place (`ContentPlace`). `Custody {0 Derived, 1 Boundary}`, `PlaceVersion {1 Entry, 2 Current}`, `PlaceSegment {1 Field, 2 FixedIndex, 3 Case}` (`:80`-`:91`). `ContentPlaceDeclaration` refuses `BlockParameter` places in both directions (`:96`, `:106`). Content terms nest at most once (separation of projections; doc `:167`-`:168`), decode refuses nested separation (`:217`). Place segments are named by text (`Field(name: Text)`, `:87`): string keys.

### A/declaration-encoding/src/terms/float_meaning.rs (213 lines)
Float-meaning projection records: sources (tags 1..10: input, exact literals, direct machine/block/call/structural leaves, semantic application), contracts with 32-byte commitments, `SemanticOperation` {0 Fixed(row), 1 FromInteger, 2 ToIntegerExact, 3 ToIntegerSaturating} with an integer carrier (`IntegerCarrier` refuses width 0, `:43`-`:45`). Tests (`:58`-`:121`) round-trip width-parameterized contracts and refuse bad carriers. Commitments are carried as bytes; they are checked later by `Proposition::validate` (see `proposition.rs` tests `:374`-`:388`, `:481`-`:491`).

### A/declaration-encoding/src/terms/integer.rs (114 lines)
`IntegerMathTerm` as a postorder table. Tags 1 literal (sign boolean + 16 magnitude bytes), 2 MathValue, 3..5 Add/Subtract/Multiply, 6 ShiftLeft, 7 Embed(scalar term as nested table) (`:43`-`:63`, `:78`-`:112`).

### A/declaration-encoding/src/terms/mod.rs (11)
Module list.

### A/declaration-encoding/src/terms/order.rs (138 lines)
`validate_canonical_proposition` (`:7`): iterative walk (explicit stack). Checks operand order for `Equal` by scalar-term byte key (`:30`), `IntegerMathEqual` by `>` on terms (`:37`), symmetric IEEE comparisons (`:49`, `:56`), byte-sequence and structural-identity equality (`:62`, `:70`), nested conjunction/disjunction refused (`:77`-`:106`), clause lists strictly increasing by encoded key (`canonical_propositions_strictly_increase`, `:126`). `LessThan`, `LessOrEqual`, math inequalities, `StructuralCaseMembership`, `ContentConservation` get no check (`:36`, `:44`, `:69`, `:114`): they have no order to check.
Same job. `contract_forms.rs` (wire crate) `canonical_clause` re-canonicalizes the same shapes on the producer side (`:130`-`:156`).

### A/declaration-encoding/src/terms/proposition.rs (494 lines)
`Proposition` as a postorder table (`PropositionTable`, `:80`). Tags: 1 Truth, 2 Falsehood, 3 Atom, 4..6 scalar relations, 7 Conjunction, 8 Implication, 9 ContentConservation, 10 Disjunction, 11 IeeeFloatComparison, 12 ByteSequenceEqual, 13 StructuralCaseMembership, 14..16 integer-math relations, 17 ScalarIeeeFloatComparison, 18 StructuralIdentity, 19 FloatMeaningEqual, 20 FloatIntegerConversion, 21, 22 (`:111`-`:229`, `:232`-`:324`). `canonical_proposition_order_key` encodes with a `checking_identity` writer so symbolic widths are allowed (`:70`-`:74`). Conjunction/Disjunction counts are bounded by `bounded_count(reader, 4)` (`:244`, `:264`). Tests (`:326`-`:494`) confirm commitment mutation is caught after decode by `.validate()`, not by the decoder.
Fail-open. The decoder itself accepts a mutated float-meaning commitment (`:313`, `:314`; test comment `:489`); rejection depends on a later `validate()` call (traced only through the tests; callers not checked: open). Contrast `order.rs:16`-`:27`, which does call `validate()` for these three variants.

### A/declaration-encoding/src/terms/scalar.rs (613 lines)
`ScalarTerm` as a postorder table. About 45 tags (1 Value, 2 Boolean, 3 Integer, 4..9 wrapping/saturating add/sub/mul, 10 BooleanNot, 11 BooleanEqual, 12..17 comparisons and bitwise, 18..24 shifts/widen/cast, 25..33 exact/wrapping/saturating divide etc., 34/42 BooleanField Current/Entry, 35/43 IntegerField, 36 ViewExtent, 37 IeeeFloatLiteral, 38/44 IeeeFloatField, 39 ByteSequencePredicate, 40 StructuralRead, 41 Application) (`:69`-`:278`, `:359`-`:520`). Every write calls `validate_after_operands()` first (`:70`); decoders rebuild via typed constructors that check the same rule (`binary_row` etc., `:327`-`:357`). `StructuralRead` encodes dynamic index operands as table children after the path (`:496`-`:508`); writer refuses a `Static(Referent)` projection (`:108`-`:112`).
Cases by shape. Dynamic-index positions are marked `None` and filled from children (comment `:478`).
Fail-open. Decoders for tags 34..36, 38/44 and 39 do not all call `validate_after_operands` (tag 38/44 does `:460`; 34, 35, 36, 39 construct directly `:416`-`:441`, `:472`-`:473`); open whether those terms have a local validity rule.

### A/declaration-encoding/src/terms/scalar_types.rs (127 lines)
Scalar type, integer type, integer value, IEEE float value codecs. `ScalarType` tags 1 Boolean, 2 Integer, 3 IeeeFloat, 4 Callable(u64 contract id; 0 refused as `ZeroIdentity`, `:81`-`:82`). Integer type tag 1 signed fixed, 2 unsigned fixed, 3 address, then 2 identity bytes for width (`:46`-`:58`); signed address is refused (`:50`-`:54`). Decode takes `u16` bits (`:98`), so parameterized widths have no wire form (writer's `require_integer_width` handles refusal, `wire.rs:51`).

### A/declaration-encoding/src/terms/table.rs (291 lines)
The postorder table machinery used by every recursive term. `TermEncoding` trait (`:28`), `TableChildren::take` (`:56`), `encode_rows` (`:101`, explicit stack), `decode_rows` (`:164`), `decode_term_table` (`:186`). A row with k children must name exactly the top k unconsumed entries in order; a handle `>= row` is `TermTableForwardReference`; other mismatch `NonCanonicalTermTable` (`:60`-`:68`). The decoder `pending` vec is unbounded by depth but bounded by row count, which is bounded by remaining bytes (`bounded_count`, `:170`). Test with 100,000 nested terms (`:226`-`:241`).

### A/declaration-encoding/src/wire.rs (331 lines)
`Writer` (`:15`) with an encoding role (`ClosedPublication`, `InvocationPrefix`, `CheckingIdentity`, `:7`-`:12`) and a shared `Rc<RefCell<EncodedIntegerTypes>>` that records every fixed integer type written (`:18`, `:51`-`:64`). `Reader` (`:154`). `string` caps length at `MAX_CONTENT_IDENTITY_BYTES = 1 << 20` on both write and read (`:4`, `:112`, `:217`). `bounded_count` (`:251`) rejects counts larger than remaining/min size. `decode_canonical` (`:317`) = decode, then trailing-bytes check, then re-encode and compare (`:324`-`:329`). `strictly_increasing` / `require_increasing` (`:286`, `:290`). Note: `1 << 20` string cap and the bounded counts are limits in the code; the project memory says not to invent size limits, this one is existing code.
Limits. none stated except by the constant name.
Fail-open. `Reader::boolean` rejects values other than 0/1 (`:207`-`:213`). none seen.

### A/declaration-encoding/tests/identity.rs (46 lines)
Three tests that read fixed byte fixtures (`tests/fixtures/identity/{domain,quotients,access}.bin`, written by "the Psi artifact writer at 296ef7ad0bf", `:8`) and require re-encoding to give identical bytes; the access test also requires that clearing `storage.vacant` makes encoding fail with `NonCanonicalEncoding` (`:41`-`:45`).

---

## 4. optimization-selections crate

### A/optimization-selections/src/lib.rs (270 lines)
1. Purpose. The closed vocabulary of Psi-level optimizations a build can select, their canonical encoding and a 32-byte identity.
2. Shape. `PsiOptimization` (9 variants, tags 1..9, `:38`-`:49`) generated by `psi_optimization_vocabulary!` (`:11`); `PsiOptimizationSelections` (sorted unique `Vec`, `:52`); `encode` (`:82`; magic `PSIOPT\0\0`, version 1, u32 count, one byte per tag); `decode` (`:93`); `identity` (`:138`, domain digest `psi.optimization-selections.v1\0`).
3. In/out. `decode` refuses short input, wrong magic, version != 1, truncation, trailing bytes, unknown tag, duplicate, out-of-order (`:94`-`:134`).
4. Decisions keyed on tag byte value.
5. Cases by shape. Fixed list of nine.
6. Same job elsewhere. A second selection type exists in `compiler/foundation/optimization-selections/src/lib.rs` with magic `OMGOPT\0\0` (Grep content, lines 36, 526, 537). Not read; suspected a separate native-side vocabulary, not a duplicate of this one (different magic).
7. Unused. `PsiOptimizationSelections`: 52 hits in 14 files across `source-rust` (Grep count, output paged with head_limit 0): this crate holds 15; the rest are `source-to-psi` (about 20), `psi-to-native` (about 7), the codec artifact (1) and `A/wire/src/sections.rs` (6).
8. Limits. none.
9. Fail-open. none seen.

---

## 5. publication crate

### A/publication/checked_program_entry.rs (50), publication.rs (10)
`CheckedProgramEntryTerminalReceipt`: record of source signature identity, source machine name, declaration identity, Psi identity, entry machine id with getters. `publication.rs` lists modules and re-exports three types.

### A/publication/machine_provenance.rs (162 lines)
1. Purpose. Names Psi machines for diagnostics: records a name and source span per machine and the first caller that reached it from a root.
2. Shape. `TerminalMachineProvenance { machines: BTreeMap<MachineId,(String,Option<SourceSpan>)>, callers: BTreeMap<MachineId,MachineId> }` (`:6`); `for_root` (`:12`) walks `execution.unoptimized_behavior` breadth-first from the entry recording callers; `label` (`:88`) formats a display string `machine \`name\` (Terminal machine N)` or "reached through"; `source_span` (`:100`).
3. Decisions. `for_root` follows `Invocation` and `NominalCleanup` callees, and for `BoundaryInvocation` every provider candidate of that boundary (`:43`-`:58`). Name lookup climbs the caller chain (`named_ancestor`, `:74`-`:86`), bounded by `callers.len()+1` iterations. `record_caller` and `name` keep the first value (`or_insert`, `:67`, `:71`).
4. Both maps keyed on machine id; the label is a display string for diagnostics only.
5. Same job. Name/caller chain also read by `entry_receipt/application.rs:110`.
6. Unused. `TerminalMachineProvenance`: 21 hits in 9 files (Grep count).
7. Fail-open. A machine whose body has no `unoptimized_behavior` entry is skipped by `continue` (`:28`-`:34`), so its callees are not attributed (labels fall back to "Terminal machine N"). This loses attribution, not a check.

### A/publication/published_entry.rs (104 lines)
1. Purpose. Declaration-owner custody rows issued with a portable program: which source declaration owns each program entry schema and each calling-policy request.
2. Shape. `PublishedEntryMetadata { provenance, has_bound_applications, entry_custody, calling_custody: Result<Vec<_>,String> }` (`:11`); `new` sorts rows by identity index (`:19`-`:35`); `validate_declarations` (`:37`); lookup by binary search (`owner`, `:95`).
3. Decisions. `validate_declarations` compares the sorted deduped declaration indices requested by `program_entry_contracts` and by `calling_policy_requests` (only those with `Ok` declaration) to the stored rows (`:38`-`:61`); `calling_custody` being `Err` is treated as matching (`map_or(true, ..)`, `:53`).
4. Fail-open. `:53` returns true when calling custody is `Err`. Classification: traced partly: `require_calling_custody` (`:64`) and `calling_schema_custody` (`:71`) return the stored error to later callers, so a program needing calling custody is refused at lookup, not at validation. Not stated by a comment at the site. Also `owner` says "has no authenticated source custody" if a declaration is missing (`:101`).

---

## 6. wire crate

### A/wire/src/block_wire.rs (2001 lines)
1. Purpose. Canonical encoding and decoding of machine control-flow blocks: block header, every `OperationKind`, and every `Terminator`.
2. Shape. Submodules `atomic_operations` (`:7`), `call_operations` (`:289`, register access), `operation_tags` (`:394`, 100+ tag constants), `scalar_operations` (`:556`, trapping integers, float result policy), `storage_operations` (`:668`, references, primitive reads/stores, element views, structural field store), `terminator_wire` (`:965`). Then `encode_block` (`:1380`), `decode_block` (`:1423`), `encode_operation`/`decode_operation` (`:1394`, `:1442`), and one macro `operation_wire!` (`:1476`) with a `rows` list (about 95 plain variants, `:1538`-`:1823`), a `paired` list for variants with an optional obligation using two tags each (`:1824`-`:1835`), and hand-written `encode`/`decode` arms for about 17 irregular variants (`:1836`-`:2000`).
3. In/out. `Block`/`Operation`/`OperationKind`/`Terminator` <-> bytes using `wire_fields` markers. Tests (`:1069`-`:1363`) round-trip, check legacy bytes and check that every tag not in the spec table (`spec_tag_tables::listed_tags("operation-tags")`) is refused as `InvalidTag("OperationKind", tag)` (`:1337`-`:1349`) and that tags 71 and 72 are unassigned and refused (`:1352`).
4. Decisions. Keyed on tag byte. Register access: encode refuses if `!operation.is_admitted()` or service presence disagrees with `requires_service_authority()` (`:302`-`:308`); decode refuses descriptor-table/segment/task operations through this path (`:340`-`:349`). Element-view read refuses non-`SharedBorrow` access on encode (`:817`) and decode (`:855`). Projected variants (`PROJECTED_*`) refuse an empty path (`:759`, `:786`, `:867`, `:917`). Terminator `PendingNormalCompletion` is refused by the portable encoder and given private tag 255 only when `writer.is_invocation_prefix()` (`:1037`, `:1047`-`:1060`); the portable decoder has no tag 255 (comment `:1054`).
5. Cases by shape. Tag numbers: whole-descriptor reads keep an older layout where place id 0 followed by an argument list marks a projected source (`:827`-`:833`, `:848`-`:861`): "Nonzero place IDs keep the existing whole-descriptor encodings". Terminator tags 2, 4, 13..20 (`:988`-`:1039`). `ExternalCompletionKind` only `ProcessExit` (`:1040`).
6. Same job elsewhere. Operation tag table duplicated in the spec tag table consumed by the test (`spec_tag_tables`, not read). Placement of `encode_structural_path` in two modules: block_wire imports `sections::semantic_module::structural_place_wire::encode_structural_path` (`:12`, `:675`) while declaration-encoding has `places::encode_structural_path` (`places.rs:8`); the wire crate has its own `structural_place_wire` (`structural_wire.rs:267`, grepped, not read), so suspected there are two path encoders (declaration-encoding and wire crate).
7. Unused. Not grepped for each tag.
8. Limits. Comment `:1054`: "This byte is local to the prefix commitment."
9. Fail-open. Decode of an unknown register-catalog tag is refused (`:332`). Decode of `ProcessExit` only. none seen in this file; semantic checks happen in `foundation_validation.rs` (not read).

### A/wire/src/closed_types.rs (185 lines)
`ClosedTypeCatalog { integers: EncodedIntegerTypes, callables: Vec<CallableContractId> }` (`:14`). `integer()` returns the recorded closed type or an error (`:29`). `record_callables` (`:33`): lists every contract id in the module's callable catalog that `callables.contract(id)` resolves, sorted and deduped. `scalar()` (`:45`): Boolean and IEEE float always close; integer must be recorded; callable must be in the retained list else "callable contract is not retained by this closed module" (`:51`-`:55`). Tests `:62`-`:185` cover width bytes, symbolic width refusal in runtime but not checking-key encodings, nested writers sharing the catalog.

### A/wire/src/contract_forms.rs (250 lines)
Purpose. Canonical forms shared by contract production and the receiving checks: equality operand order (`equal`, `:12`), clause sorting and de-duplication by encoded key (`clauses`, `:24`; `RepeatedClauses::{Refuse,Merge}`), conjunction flattening (`:47`), crash route bucketing (`routes`, `:63`), route substitution (`substituted_routes`, `:89`), and clause substitution with formal-to-actual binding (`substitute_parameters`, `:114`, `ScalarFormals`, `:158`).
Decisions keyed on byte-encoded order keys (`canonical_*_order_key`). Formals are 1-based positions: `formal.get() - 1` indexes `values` or `places` (`:185`-`:191`, `:168`-`:174`).
Cases by shape. `field()` refuses entry-version fields: "a loop header cannot restate a prior invocation's storage snapshot" (`:200`-`:202`); callable fields refused (`:223`); `case()` always errors (`:248`). `canonical_clause` re-sorts only `Equal`, `IntegerMathEqual`, `Conjunction`, `Disjunction` (`:130`-`:155`); other propositions pass through unchanged, so symmetric IEEE/byte/identity equality operands are not re-ordered here (`:154`), though `order.rs` requires them ordered. Open: whether substitution can break that order (it replaces values, not positions, so an ordered pair may become unordered).
Same job. Order checks in `declaration-encoding/src/terms/order.rs`.
Fail-open. `routes()` collapses a bucket to `[Truth]` whenever any alternative is `Truth` (`:80`-`:83`): this widens, by design, matching `crash_routes_are_canonical`; stated by that function's rule at `crash_routes.rs:36`.

### A/wire/src/debug_map_records.rs (92 lines)
Plain data types for the debug map: `DebugFileId` (non-zero u32), `DebugSourceDigest` ([u8;32]), `DebugSourceOrigin {User, Toolchain}`, `DebugSourceFile`, `DebugSourceSpan`, `DebugSubject` (Machine, Block, Operation, Edge, Value, Contract, Obligation, Place, Claim), `DebugSite`, `TerminalDebugMap { semantic, files, sites }` (`:88`). No logic.

---

## Coverage
Read in full (all lines of every range in `unit_038.tsv`): 47 files, 10,915 lines (reach 567, results 141, signatures 142; codec 921+236+73+153+53+106; declaration-encoding 5,176 + terms 2,569 + tests 46 = 7,791 incl. `access/` and top-level; optimization-selections 270; publication 326; wire 2,001+185+250+92). Skimmed: none. Not read: none.

## Five things a maintainer most needs to know
1. Every decoder here is paired with a re-encode comparison. `decode_canonical` (`declaration-encoding/src/wire.rs:317`) and `CanonicalTerminalArtifact::from_bytes` (`codec/src/canonical_artifact.rs:310`) reject any bytes that do not re-encode identically, so a codec bug in only one direction shows up as `NonCanonicalEncoding`.
2. Decoders do not all validate meaning. For float-meaning propositions the decoder accepts a mutated commitment and relies on a later `.validate()` call (`proposition.rs:313`, test `:489`); `terms/order.rs:16` is one place that calls it.
3. Region access has an asymmetric atomic tag: encoder writes `ObservationModel::Atomic` as 2 (`access/views.rs:297`), decoder rejects 2 (`:366`-`:370`), while `domains.rs:127` accepts 2.
4. All recursive terms (scalar, integer-math, proposition, capacity) share one postorder-table format (`terms/table.rs`); depth is not stack-bounded. Wire fields are described by the `Field<T>` trait and the `wire_record!`/`wire_enum!` macros (`fields.rs`), including a `; refuse` clause for variants that must never be written.
5. `PublishedEntryMetadata::validate_declarations` accepts an `Err` calling custody (`publication/published_entry.rs:53`); refusal happens later when a caller asks for that custody (`:64`, `:71`).

## Unsure about
- Whether `access_admission` can ever produce a `FieldAccess::Region` with `Atomic` observation (item 3 above).
- Whether `parameter_kinds` length and sortedness of `fixed`/`dependencies` in `reach.rs` are checked elsewhere (`A/wire/src/foundation_validation.rs`, not read).
- Whether the wire crate's `structural_place_wire`/`scalar_wire`/`structural_signature_wire` duplicate declaration-encoding paths and access codecs (only grepped for module names).
- Whether decoders for scalar tags 34, 35, 36, 39 need `validate_after_operands`.
