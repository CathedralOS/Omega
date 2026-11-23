# Record: unit_083 (area 15-package-manager)

Checkout `C:/Development/Omega/.agents/worktree/worker-2/` at `4f3c255d0559`. All 177 files of `units/unit_083.tsv` were read in full.

Path shorthand used below:
- `E/` = `source-rust/subsystems/package-manager/review/evidence/src/` (crate `review-evidence`)
- `R/` = `source-rust/subsystems/package-manager/review/` (package-manager crate, review module)
- `A/` = `source-rust/subsystems/package-manager/source-acquisition/` (crates `source-acquisition`, `source-capture`, `source-selection`, `resolver-execution`)

Method note. "read" = I read the lines. "grepped" = a `grep -rnE --include=*.rs` over `source-rust/` with `_recycling` filtered out; counts include the definition line and tests unless I say otherwise.

Cross-file facts used in many entries:
- The recovery readers in `E/encoding/recovery/policy/` all call the `tag!` macro (`E/encoding/recovery/policy.rs:97`, read): a tag byte not in the listed arms returns `Error::InvalidTag`. `grep 'tag!\(reader'` finds 125 uses.
- `Reader::sequence(minimum_encoded_bytes, decode)` (`E/encoding/recovery/policy/reader.rs:128`) refuses a count larger than `remaining_bytes / minimum_encoded_bytes`. Each call site passes its own literal minimum (for example 62, 112, 58, 41, 83 in the files below). Those literals are written by hand at each site.
- Record validators (`E/record/**/validation*.rs`, `*_validation.rs`) return `Result<(), &'static str>`. Their judgments are on identity equality (`PackageReviewNominalIdentity` = `owner` + `path` string), positions (indices into telescopes), and canonical order (`ordered`, `ordered_by`).

---------------------------------------------------------------------------------------------------

## Part 1. `review-evidence`: binary recovery readers (`E/encoding/recovery/policy/`)

### `physical_calling_policy.rs` (34), `physical_calling_policy/placement.rs` (134), `physical_calling_policy/state.rs` (72)
1. Purpose: read the physical calling contract (registers, stack slots, entry control, saved machine state) back from canonical bytes.
2. Shape: `physical` (`physical_calling_policy.rs:9`) builds `PackagePolicyPhysicalCallingContract`; `policy` (`:24`) reads the six-way calling policy tag; `value_placement` (`placement.rs:11`), `eightbyte` (`:38`), `location` (`:47`), `register` (`:75`); `entry_control` (`state.rs:10`), `state_plan` (`:23`), `state_set` (`:55`).
3. In/out: takes `&mut Reader`; returns record values from `E/record/calling/physical.rs` and `E/record/representation/movement.rs`.
4. Decides: by tag byte only. `state_set` rejects a non-strictly-ascending list with `NonCanonicalEncoding` via `PackagePolicyMachineStateSet::from_canonical` (`state.rs:71`, `E/record/calling/physical.rs:89`).
5. Cases by shape: fixed vocabularies. Calling policy tags 0..5 (`MicrosoftX64` ... `DarwinAapcs64`, `physical_calling_policy.rs:27-33`); register tags 0..18 (x86 general, `Xmm(n)`, `Aarch64X(n)`, `Aarch64V(n)`, `placement.rs:76-96`). Sequence minima: parameters `13`, locations `6`, clobbers `1`.
6. Same job elsewhere: encoder counterparts in `E/encoding/encode/` (`encode_value_placement` is used by the test at `placement.rs:104`; other encoder files are outside this unit).
7. Unused code: none seen.
8. Stated limits: none.
9. Fail-open: none seen.

### `policy_recovery_accounting.rs` (127)
1. Purpose: resource counters, ceilings and fixed error text for recovery of untrusted policy bytes.
2. Shape: `PackagePolicyRecoveryUsage` (`:18`, `owned_bytes`, `sequence_elements`), `PackagePolicyRecoveryLimits` (`:34`; `maximum_bytes`, `maximum_field_bytes`, `maximum_sequence_elements`, `maximum_owned_bytes`, `maximum_depth`), `PackagePolicyRecoveryError` (`:85`, 16 variants incl. `RetiredVocabulary` `:94`).
3. In/out: `bounded()` (`:59`) clamps a caller's limits down to `Default` (64 MiB / 64 MiB / 65_536 elements / 64 MiB / depth 128, `:72-82`).
4. Decides: numeric comparison only.
5. Cases by shape: none.
6. Same job elsewhere: `PackagePolicyRowLimits` in `E/record/policy_rows_limits.rs:4` is the same pattern for rows (different defaults: 65_536 rows, 128 MiB, 1M elements, `:17-23`).
7. Usage: `PackagePolicyRecoveryUsage` appears on 18 lines across `source-rust` (grepped).
8. Stated limits: comment names the 64 MiB defaults as sized so large `semantic_dependencies` fit "within the candidate pipeline's retained policy budget" (`:9-11`).
9. Fail-open: none seen. A caller can ask for higher limits but `bounded()` caps them.

### `reader.rs` (201)
1. Purpose: the one bounded binary cursor every recovery function uses.
2. Shape: `Reader` (`:6`; fields `applied_services`, `remaining`, `limits`, `remaining_elements`, `remaining_owned_bytes`, `depth`); `new` (`:16`), `finish` (`:34`, trailing bytes error), `take` (`:56`), `literal` (`:65`, a mismatch gives `UnsupportedVersion`), fixed-width little-endian readers (`:79-99`), `boolean` (`:101`, only 0/1), `bytes`/`string` (`:109,124`), `sequence` (`:128`), `option` (`:153`), `nested` (`:164`), `boxed` (`:178`), `elements` (`:186`), `allocate` (`:194`), `canonical_scratch` (`:49`).
3. In/out: charges `remaining_elements` and `remaining_owned_bytes`; `usage()` (`:42`) reports spend.
4. Decides: bytes remaining vs. count; depth vs. `maximum_depth` (`:168`); field length vs. `maximum_field_bytes` (`:111`).
5. Cases: `sequence` with `minimum_encoded_bytes == 0` always errors (`:135`).
6. Same job elsewhere: text-side framing reader `framing.rs` repeats the byte-ceiling logic (`reserve`, `:40-66`) because it builds bytes from text.
7. Usage: `applied_services` is set from the schema version at `E/encoding/recovery/policy/baseline.rs:47` and read at `E/encoding/recovery/policy/identity.rs:32` (grepped).
8. Stated limits: comment line 2: `canonical_scratch` charges verification output to the same budget.
9. Fail-open: none seen. `allocate` for `sequence` uses `size_of::<T>() * count`, not the heap bytes owned by nested values (inferred from `:138-142`); nested allocations charge themselves when they run.

### `public_api.rs` (40), `public_api/data.rs` (38), `public_api/domains.rs` (41), `public_api/signatures.rs` (80), `public_api/traits.rs` (114)
1. Purpose: read the public API section: traits, conformances, domains, propositions, constants, operators, data, type aliases.
2. Shape: `public_api` (`public_api.rs:13`, takes `schema_version`); `data_shape` (`data.rs:14`); `domain_shape` (`domains.rs:15`); `type_parameter` (`signatures.rs:19`), `machine_contract` (`:46`, wrapped in `reader.nested`), `machine_signature` (`:62`); `trait_shape` (`traits.rs:21`), `trait_requirement` (`:36`), `conformance_shape` (`:89`), `operator_shape` (`:100`).
3. In/out: reads, produces `E/record/public_policy/*` records.
4. Decides: schema version. `type_aliases` exists only if `schema_version >= BASELINE_WITH_TYPE_ALIASES` else `None` (`public_api.rs:25-38`). `structural_parameter_positions` is `None` when `schema_version == BASELINE_WITHOUT_STRUCTURAL_PARAMETERS` (`traits.rs:49-55`).
5. Cases: closed vocabularies: data kind 0/1, supply 0/1, domain classification 0/1 (only `ProgressProfile`), semantic roles 0/1, borrow origin tags 1/2 (tag 0 is not accepted, `traits.rs:73-76`), type-parameter tags 0..5.
6. Same job elsewhere: `trait_requirement` and `machine_signature` read near-identical signature field lists (`traits.rs:40-66`, `signatures.rs:65-79`); the record side also has two near-twins, `PackagePolicyTraitRequirement` (`E/record/public_policy/declarations.rs:27`) and `PackagePolicyMachineParameterSignature` (`E/record/public_policy/signatures.rs:40`).
7. Usage: `conformance_shape` is re-exported and used from `representation_availability.rs:12`.
8. Stated limits: none.
9. Fail-open: `schema_version` above the current one is not rejected here (the caller in `baseline.rs`, not in this unit, decides) (inferred).

### `representation.rs` (92), `representation_availability.rs` (15)
1. Purpose: read the representation policy (opaque types, producer availability, selections, demands).
2. Shape: `impl PackagePolicyRepresentation { recover_canonical_component!(...) }` (`representation.rs:19`), `policy` (`:28`), `opaque_representation_fields` (`:46`), `selection` (`:78`), `availability` (`representation_availability.rs:7`).
3. In/out: reads target via `calling_application::shapes::target` (outside this unit), demands via `calling_application::application`.
4. Decides: tags. Origin tag only `0 => NamedConformance`; lifecycle only `0 => Inert`; copy disposition `0/1` (`:65-74`).
5. Cases by shape: single-variant vocabularies (`NamedConformance`, `Inert`).
6. Same job elsewhere: `opaque_representation_fields` is called from `selection` (this file); a grep of the name finds it also used from the calling-application reader (not read; suspected).
7. Usage: none to report.
8. Stated limits: none.
9. Fail-open: none seen.

### `selected_providers.rs` (142) and `selected_providers/{authority,bindings,service,signature}.rs` (38, 63, 123, 25)
1. Purpose: read selected-provider plans, rows, families, service methods, and provider bindings.
2. Shape: `PackagePolicySelectedProviders::recover_canonical` (`selected_providers.rs:25`), `policy` (`:52`), `target` (`:63`), `plan` (`:73`), `row` (`:98`), `family` (`:120`); `authority` (`authority.rs:12`); `binding` (`bindings.rs:20`); `method` (`service.rs:18`), `carry` (`:104`); `signature` (`signature.rs:10`).
3. In/out: accepts versions `PROVIDER_POLICY_WITHOUT_APPLICATION_GRANTS`, `..._WITH_APPLICATION_GRANTS` and the current one (`:34-40`); validates with `validate_canonical_structure`, mapping any error to `Error::InvalidValue` (`:42-46`).
4. Decides: `target` (`:63-71`) finds the target by comparing the string read from the bytes with `TargetProfile::identity().as_str()` for each profile: a string compare of the target name. `ArithmeticDomain`-style string matching is in `scalars.rs` (below).
5. Cases: binding tags 1..7; tag `0 => return Err(RetiredVocabulary)` (`bindings.rs:25`, comment `:2` says it was a retired string-backed import). Progress subject only tag `1` (`authority.rs:22`, `service.rs:85`). Completion tags 0/1 where 1 is `External(ProcessExit)` only (`service.rs:45-50`). Grant selector tags 0..2 (`selected_providers.rs:89-93`). Authority tags 0/1, coverage only tag 0 (`:127-133`).
6. Same job elsewhere: `service.rs` reads `ServiceProgressPremise` using strings plus an interned-identity table (`:81-98`); `authority.rs` reads the same premise shape with nominal identities (`:18-36`). Two readers for two forms of one idea.
7. Usage: `service_method` is re-exported (`selected_providers.rs:14`) and used in `terminal_permissions.rs:8`.
8. Stated limits: `service.rs:2` comment: interned route identities rebuild from the same texts; "mint order is record-local".
9. Fail-open: none seen. Unknown target string gives `InvalidValue` (`:70`).

### `signatures.rs` (55)
1. Purpose: read conformance bounds and data properties.
2. Shape: `conformance_bound` (`:8`), `data_properties` (`:19`, multiplicity tags 0..2 then a mandatory `tag 0 => ()` byte at `:28`), an in-file `#[test]` (`:32`).
3. Decides: tag. The trailing byte must be exactly 0 (the test at `:53-54` shows `[0,1,0,...]` gives `InvalidTag`).
4. Fail-open: none seen. Note: the trailing `0` byte is a placeholder for an absent policy (test name says "absent policy tag").

### `structural_expressions.rs` (133), `structural_expressions/scalars.rs` (93), `structural_expressions/tags.rs` (208)
1. Purpose: read structural runtime-requirement expressions (boolean and scalar expression trees) and primitive-type / integer-operator vocabulary.
2. Shape: `boolean_expression` (`structural_expressions.rs:15`), `structural_field` (`:94`), `structural_path` (`:103`), `runtime_index` (`:117`); `scalar_expression` (`scalars.rs:16`); `primitive_type` (`tags.rs:9`), `integer_binary` (`:26`, 31 tags).
3. In/out: each recursive call is inside `reader.nested` (`:18`, `scalars.rs:19`) and child expressions go through `reader.boxed`, so depth and allocation are charged.
4. Decides, SPELLING: the integer-literal landing arithmetic domain is read as a string and matched against `"Exact"`, `"Wrapping"`, `"Saturating"`, `"Trapping"` (`scalars.rs:40-45`); any other string gives `InvalidTag`. The encoder writes those same strings (`E/encoding/encode/values/crashes/vocabulary.rs:80-81`, grepped). It is a string compare at a point where every other enum uses a tag byte.
5. Cases: boolean tags 0..8, 10..14 (tag 9 is not accepted, `:52-76`); scalar tags 0..10, 14, 15, 16 (tags 11-13 not accepted, `scalars.rs:73-90`); primitive type tags 0,1,2,3,11 (`tags.rs:13-23`). Tag 3 builds `FixedInteger::new(sign, width)` and rejects an invalid width with `InvalidValue` (`tags.rs:19`).
6. Same job elsewhere: the three-way validation of the same expression shapes is in `E/record/package/callable_policy/validation/structural.rs` (below).
7. Usage: `tags.rs` contains in-file tests (`:65-207`) pinning round trips and old-version rejection.
8. Stated limits: none.
9. Fail-open: none seen.

### `terminal_permissions.rs` (69)
1. Purpose: read terminal permission services and per-method dispositions.
2. Shape: `recover_canonical_component!` for `PackagePolicyTerminalPermissions` (`:19-26`, accepts the extra version `TERMINAL_POLICY_WITH_NOMINAL_SERVICES`), `policy` (`:28`), `disposition` (`:51`).
3. Decides: `disposition` finds a class by `TerminalAuthorityClass::ALL` and `canonical_tag()`; classes must be strictly ascending (`:61`) else `NonCanonicalEncoding`; then `TerminalAuthorityDisposition::try_from_canonical_classes`.
4. Fail-open: none seen.

### `text.rs` (58), `text/framing.rs` (183), `text/tokens.rs` (112)
1. Purpose: recover a baseline from its named-text form by turning the markup back into canonical binary, then re-rendering to prove the text was exactly canonical.
2. Shape: `PackagePolicyTextRecoveryLimits` (`text.rs:14`), `PackagePolicyBaseline::recover_text_with_usage` (`:35`); `framing::binary` (`framing.rs:9`), private `Reader::value` (`:87`), `reserve` (`:40`); `Tokens` (`tokens.rs:6`) with `atom` (`:26`), `quoted` (`:54`), `decoded` (`:86`), `nibble` (`:106`).
3. In/out: text -> bytes -> `recover_canonical_with_usage` -> `render(&policy, Writer::new(maximum, Some(text)))` compared back to the input (`text.rs:49-51`); a mismatch is `NonCanonicalEncoding`.
4. Decides, SPELLING: `framing.rs` matches the keywords `"field" | "record"`, `"item"`, `"}"`, `"sequence"`, `"option"`, `"tag"`, `"bool"`, `"u8"`..`"i128"`, `"string"|"bytes"|"fixed"`, `"digest"` (`:88-181`). Names after `field`/`record`/`tag` only have to be a valid label (`label(...)`, `:90,117`). The comment at `framing.rs:2` says renamed fields are caught by the re-render, not by this reader.
5. Cases: digest must be exactly 64 lowercase hex characters (`:171`, `nibble` accepts only `0-9a-f`, `tokens.rs:108-110`). Tokens longer than 64 bytes fail (`tokens.rs:33`). Quoted strings accept bytes 0x20..0x7e plus `\\`, `\"`, `\xHH` escapes (`:62-81`).
6. Same job elsewhere: `framing.rs` `reserve` (`:40-66`) vs `Reader::allocate` + `sequence` (`reader.rs`): two byte-budget implementations.
7. Usage: `recover_text_with_usage` has 2 callers outside `evidence/src` in `source-rust` (`dependencies/graph/subject/catalog.rs:364`, grep matched also a different type's method at `dependencies/graph/subject/text/record.rs:109`; so the baseline variant's external callers were not separated).
8. Stated limits: none beyond comments naming the budget model.
9. Fail-open: `framing::binary` alone accepts any renamed field names; safety depends on the caller rendering again (stated, `framing.rs:1-2`; the one caller in `text.rs:50` does so). `tokens::decoded` indexes `value[index]` after a backslash and uses `.expect("validated escape")` (`tokens.rs:94,99`); it is only safe on slices that `quoted` already validated (traced: sole caller `framing.rs:158,166` passes `quoted` output).

---------------------------------------------------------------------------------------------------

## Part 2. `review-evidence`: crate root, acceptance comparison

### `lib.rs` (9), `projection_error.rs` (25)
Purpose: crate root with `#![forbid(unsafe_code)]` (`lib.rs:3`); two error enums `SemanticBindingProjectionError` (`projection_error.rs:7`) and `PackageReviewProjectionError` (`:18`). No decisions.

### `permission_acceptance.rs` (309)
1. Purpose: compare the permissions and assumptions a project's lock accepted with those a fresh compile produced. It does not admit anything (`:1-2`).
2. Shape: `TerminalPermissionAcceptance` (`:9`; `schema_version`, `package`, `target`, `rows: Vec<([u8;32], String)>`), `RuntimeServicePermission` (`:17`), `compare_runtime_permissions` (`:25`), `compare_policy` (`:39`), `check_subject` (`:51`), `compare_rows` (`:69`).
3. In/out: reads proposed rows from `encoding::runtime_permission_rows` or `PackagePolicyBaseline::acceptance_rows_with_limits`; returns `Ok(())` or diagnostics.
4. Decides: schema version must equal `PACKAGE_POLICY_ROW_VERSION` (`:56`); package and target equal (`:61`). Only rows with `kind().requires_runtime_consent()` take part (`:76`). A `ComponentAssumption` row is accepted only if `(key digest, canonical text)` is present in `self.rows` (`:81-88`); the remaining rows are compared as sorted lists for exact equality (`:106-113`). The key is `wire::digest_encoder::content_identity(row.key_bytes())` and the meaning is the canonical text string.
5. Cases: three consent kinds only: `TerminalPermission`, `AssumedBindingContract`, `ComponentAssumption` (`E/record/policy_rows.rs:55`).
6. Same job elsewhere: `lockfile/acceptance.rs:123` builds the `TerminalPermissionAcceptance` from the lock (not read).
7. Unused code: `compare_policy` has 1 grepped hit (its own definition) across `source-rust`. `compare_runtime_permissions` has one non-test caller at `source-rust/subsystems/compiler/runtime_consent.rs:97`.
8. Stated limits: comment `:92-94` points at `component_publication.md`.
9. Fail-open: none seen. An extra non-consent row (a "regular callable") passes by design (test `:292-294`).

---------------------------------------------------------------------------------------------------

## Part 3. `review-evidence`: record vocabulary and validators (`E/record/`)

### `record.rs` (195) and `record/accessors.rs` (126)
- `record.rs`: module list and re-exports; also defines `PackageReviewSourceFacts` (`:41`) with `consumed_units: Option<Vec<_>>` ("An absent inventory cannot support cache reuse", `:39`). No logic.
- `accessors.rs`: four declarative macros: `opaque_representation_selection_accessors!` (`:8`), `package_policy_callable_signature_accessors!` (`:49`), `signature_parameter_accessors!` (`:88`), `field_projection_accessors!` (`:109`, kinds `valued|copied|borrowed|sliced|optional`). `field_projection_accessors!` has 297 grepped uses.

### `authority.rs` (93), `authority_expressions.rs` (188), `behavior_policy.rs` (105)
Purpose: dangerous-authority classes and crash/termination/progress vocabulary. `PackageReviewDangerousAuthorityClass` (`authority.rs:23`, eight classes), `PackageReviewDangerousAuthority` (`:35`), `...Slack` (`:41`), `PackageReviewInstallationReach` (`:59`). `authority_expressions.rs` defines the scalar/boolean expression enums (`:79`, `:135`); `PackageReviewPrimitiveType = symbols::PrimitiveType` (`:16`). `behavior_policy.rs`: `PackagePolicyCrash*`, `PackagePolicyTermination` (`:100`), `PackagePolicyCapabilityFlow::compare_canonical` (`:24`) which orders flows through a hand-written `kind_tag()` mapping (`:29-37`): a second tag table for `CapabilityFlowKind` besides any encoder table.
Empty impls: `impl PackageReviewProgressSubject {}` (`authority.rs:93`), `impl PackageReviewStructuralParameterField {}` (`authority_expressions.rs:41`), and similar (see "Dead stubs" at the end).
Fail-open: none seen.

### `baseline.rs` (126) with `baseline/{boundary_owners,component_assumptions,external,getters,native_bindings,validation}.rs` (70, 29, 88, 63, 92, 262)
1. Purpose: the `PackagePolicyBaseline` record (one reviewed package in one target) and its structural validator.
2. Shape: `PackagePolicyBaseline` (`baseline.rs:35`, 18 fields); `apply_service_audit_policy` (`:59`), `apply_service_classes` (`:79`); `validate_canonical_structure` (`validation.rs:24`), `validated_semantic_dependencies` (`:147`), `symbolic_demands` (`:215`), `exposes` (`:255`); `validate_boundary_application_owners` (`boundary_owners.rs:15`), `validate_demand` (`:41`); `validate_external_associations` (`external.rs:14`); `PackagePolicyNativeBinding::validate` (`native_bindings.rs:54`); `PackagePolicyComponentAssumption::validate` (`component_assumptions.rs:20`).
3. In/out: validator reads the baseline's own components. Cross-package check takes a `lookup` closure (`boundary_owners.rs:17`).
4. Decides: package/target equality across components (`validation.rs:25-35`); ordering and uniqueness with `ordered` (`:64-70`); `native_bindings` order by `key()` which uses `producer.callable_identity` string (`native_bindings.rs:41`); each native binding's `application` string must satisfy `matches_closed_calling_application` (`native_bindings.rs:70-85`); target tags in external supplies resolved with `TargetProfile::from_canonical_target_name` and compared to `identity().as_str()` (`validation.rs:82-95`, a string compare of target names).
5. Cases by shape: `apply_service_classes` classifies a service only if some callable `exposes` it (`:91-96`); `native_bindings.validate` requires exactly one calling application: `let [calling] = ... else Err` (`native_bindings.rs:67`) ("native inventory has no unique pinned calling application"); native producers are limited to `Import` and `Syscall{evaluated: Some}` (`:44-52`).
6. Same job elsewhere: external validation checks the same signature against the callable twice, once here (`external.rs:41-56`) and once in `E/record/signatures/external_policy/validation.rs:83-96`.
7. Usage (grepped): `apply_service_audit_policy` 3 hits (callers `compiler/build/planning/package_review/policy.rs:120`, `R/candidate/passes_occurrences.rs:643`); `apply_service_classes` 3 (caller `policy.rs:119`); `validate_boundary_application_owners` 2 (caller `lockfile/validation.rs:55`); `PackagePolicyMachineStateSet::new` 1 (caller `compiler/build/planning/package_review/calling/physical.rs:61`).
8. Stated limits: `boundary_owners.rs:3-5` "Toolchain declarations have no managed-package baseline to look up"; `external.rs:5-7`.
9. Fail-open:
   - `baseline.rs:86-98`, `apply_service_classes`: `continue`s (drops the classification silently) when a dangerous capability with the same `service` already exists (the `class` is not compared) or when no callable exposes the service. A second class for the same service is lost. Open (no comment). Reached by any `(service, class)` list passed by the two callers above.
   - `boundary_owners.rs:30`: `ToolchainSource(_) => continue`: no check for toolchain-owned operators. Stated (comment at `:5`).
   - `validation.rs:117-122`: a slack relation with an absent callable is an error; fine (fail-closed).

### `calling.rs` (29), `calling/application.rs` (94), `calling/application/validation.rs` (226), `.../validation/callbacks.rs` (225), `.../validation/shapes.rs` (192), `.../validation/target.rs` (7), `calling/callbacks.rs` (89), `calling/opaque.rs` (25), `calling/physical.rs` (120)
1. Purpose: the calling-contract records of a boundary call (ABI, shape graph, native parameters, callback relations, opaque uses, physical placement) and their structural validators.
2. Shape: `PackagePolicyCallingPlan` (`application.rs:20`, 21 fields); `validate_canonical_structure` (`validation.rs:16`), `validate_for_service` (`:23`), `validate_structure` (`:98`); `callbacks::validate` (`callbacks.rs:16`), `validate_disjoint_layouts` (`:186`); `shapes::validate` (`shapes.rs:19`), `visit` (`:39`, colour-marking DFS), `validate_opaque` (`:92`); `PackagePolicyPhysicalCallingContract` (`physical.rs:16`).
3. In/out: reads the plan only; leans on `layout_plans::validate_private_callback_placements` and `compose_private_callback_placements` (`callbacks.rs:166,174`), `application_identity::type_identity::encoding` helpers (`validation.rs:11-13`).
4. Decides: ABI requirement path string equals `requirement.path` (`validation.rs:17`, and the reverse test `:28`); `static_parameters` kinds vs `abi.operands` kinds mapped by a `match` (`:42-50`); target geometry; lengths of native vs physical vs semantic telescopes (`:129-135`); name uniqueness of native parameters (`:137-144`); shape DAG acyclicity with colours (`shapes.rs:48-52`), array extent = `element.byte_size * length` with equal alignment (`:62-66`), record field extent `byte_offset + child.byte_size <= parent.byte_size` (`:79`); opaque occurrence path walk (`:170-185`); callback binder ordinals vs machine-kind static parameters (`callbacks.rs:31-51`).
5. Cases by shape (fixed list, then refuse): "normalized capacity" caps of 256 on native parameters and semantic parameters (`validation.rs:113`), shapes and fields (`shapes.rs:21`), opaque uses and occurrences (`:99`); callback catalog limit `BOUNDARY_CALLBACK_CAPACITY` (`callbacks.rs:18-24`). Comment `shapes.rs:4-6`: caps chosen "so cycle checking uses fixed stack scratch (`[u8; 256]`)". Callback layout may only be of `Parameter` or `Field` destination (`:74-96`). Opaque roles only `Parameter`/`Result`; path elements only `FixedArrayElement` and `RecordField` (`shapes.rs:133-184`).
6. Same job elsewhere: `callbacks.rs:208-212` `contained` + `callback_ranges` (disjointness of byte ranges) is separate from the shape DAG extents check at `shapes.rs:62-84`.
7. Usage: `target.rs` is a 7-line forwarder to `PackageReviewRepresentationTarget::validate_canonical_structure` (`target.rs:6`). `impl PackagePolicyCallbacks {}` etc. empty (`callbacks.rs:84-89`).
8. Stated limits: `validation.rs:27` "Earlier policy records have only the declaration path in the key."
9. Fail-open:
   - `validation.rs:28-30`: for an old-style key (`abi.requirement_identity == requirement.path`) `validate_for_service` runs only `validate_canonical_structure`, skipping the closed-application checks (`:31-95`). Stated (comment `:27`). Reached by old-format records.
   - `shapes.rs`: record field offsets are checked against the parent extent but not for overlap or alignment between sibling fields (read `:68-85`). The check cannot see an overlapping-field record. Open. Whether another stage rejects it: not traced.

### `conformances.rs` (9), `conformances_policy.rs` (49), `contracts.rs` (37), `contracts/callable_contracts.rs` (81), `contracts/declarations.rs` (29), `contracts/expressions.rs` (318), `contracts/propositions.rs` (122), `data.rs` (44), `domains.rs` (56)
Purpose: pure data vocabulary (closed conformance application, contract expressions with 28 variants `expressions.rs:209-298`, static arguments, propositions, data fields, domain routes). Logic: `PackageReviewDomainEstablishmentRoute::kind` (`domains.rs:45`), `PackageReviewResultCaseIdentity` getters, `PackageReviewSynchronousInvocation::parameter/service`. Decisions: none. `PackageReviewArithmeticDomain` (`expressions.rs:86`) is a four-variant enum that recovery reads back by string (see Part 1).

### `contracts/expressions_operator_policy.rs` (328)
1. Purpose: compare an operator coordinate or nominal identity to a framed identity string without allocating.
2. Shape: `matches_policy_requirement` (`:29`), `write_policy_requirement` (`:43`), `PackageReviewNominalIdentity::matches_named_callable` (`:57`), `trait_application_operands` (`:80`), `matches_unapplied_trait_declaration` (`:94`), `matches_applied_trait_declaration` (`:98`), `matches_closed_calling_application` (`:104`), `write_qualified_path` (`:130`), `MatchingWriter` (`:175`), `EscapingWriter` (`:161`).
3. In/out: renders the expected string into a writer that strips the expected text as a prefix; true when the whole string is consumed.
4. Decides, SPELLING: the judgment is a text match against strings built with literal labels `"boundary-operator-policy"` (`:46`), `named-callable(`, `path`, `parameters`, `result-dispatch` (`:67-75`), `nominal(`, `package-owner(32:<hex>)`, `toolchain-owner`, `unresolved-owner` (`:131-143`), `trait-application(` with `",arguments(),lifetimes())"` as the unapplied marker (`:85,95`). Escaping of `\ ( ) ,` is done by `EscapingWriter` (`:166`).
5. Cases: unapplied vs applied trait is told apart by comparing the tail text to `",arguments(),lifetimes())"` (`:95,100`).
6. Same job elsewhere: the same canonical-string families are produced in `application_identity::type_identity` (`:101,115-126` call into it); the validator `validate_trait_application` is imported from there. `write_framed_identity` (`E/record/identity.rs:14`) is also used in `terminal_permissions_generics.rs:75`; grepped 9 lines across `source-rust`.
7. Usage: `policy_requirement_identity` is `#[cfg(test)]` (`:19`); grepped 2 hits. `matches_policy_requirement` 18 grepped hits.
8. Stated limits: comment `:9-11` says the owner namespace is written as `toolchain-owner` "without source digest".
9. Fail-open: for a `ToolchainSource` owner, `write_qualified_path` writes `toolchain-owner` and ignores the toolchain digest (`:141`). Test `:236-243` pins that a different toolchain digest still matches. Stated (comment `:9-11`). A requirement string from toolchain A therefore matches a coordinate owned by toolchain B with the same path and dispatch.

### `identity.rs` (109)
`write_framed_identity` (`:14`), `PackageReviewNominalOwner` (`:36`: `Package`, `ToolchainSource`, `Unresolved`), `PackageReviewNominalIdentity::validate_resolved` (`:55`: refuses empty path or `Unresolved`), `PackageReviewServiceIdentity::validate_resolved` (`:88`: if `application` is set it must satisfy `matches_applied_trait_declaration`, string match). `validate_resolved` is the shared "resolved identity" gate; it is called from 20+ validator files in this unit (read; no count run).

### `package.rs` (45), `package/boundary_policy.rs` (73), `package/boundary_policy_validation.rs` (212)
`PackagePolicyBoundaryApplications` (`boundary_policy.rs:18`), validation `PackagePolicyBoundaryApplications::validate_canonical_structure` (`boundary_policy_validation.rs:37`), `checked_body` (`:154`), `validate_application` (`:172`). Decides: realization shape against the provider plan row at `selected_plan_index` found by binary search on `(requirement owner, requirement path string)` (`:92-104`); for a realization of `NongenericCheckedBody` the application must be `Empty`, `SpecializedCheckedBody` must be `Exact(_)`, `ExactCompilerIntrinsic` must be `Empty` plus a `CompilerIntrinsic` binding whose execution equals (`:109-148`). Requirement identity string must match the coordinate via `matches_policy_requirement` (`:79`). Cases: three realization variants (closed). Fail-open: none seen.

### `package/callable_policy.rs` (86) plus `callable_policy/{conformances,getters,reach_dependency}.rs` (28, 108, 16)
`PackagePolicyCallables` (`:33`), `PackagePolicyCallable` (`:55`, ~30 fields), `PackagePolicyCallableRole` (`:46`: Boundary, Public, Build, PrivateAssumption, PrivateExternal). `exposed_services` (`getters.rs:48`) chains six sources: published reach, checked realized reach, checked concrete reach, unresolved installation upper bounds, declared and realized synchronous invocations (read). `published_service_reach` (`:82`) falls back from declared to checked realized reach. Used by `baseline.rs` `exposes`.

### `callable_policy/validation.rs` (125), `validation/behavior.rs` (190), `validation/signature.rs` (204), `validation/signature_contracts.rs` (150), `validation/signature_expressions.rs` (242), `validation/signature_parameters.rs` (3), `validation/structural.rs` (189)
1. Purpose: structural validation of each callable row: role/supply consistency, authority ceilings, behavior declarations, signature scope.
2. Shape: `PackagePolicyCallables::validate_canonical_structure` (`validation.rs:16`), `service_set` (`:117`); `behavior::validate` (`behavior.rs:20`), `invocations` (`:142`), `progress` (`:161`); `Scope` (`signature.rs:28`) with `static_kind` (`:39`), `validate` (`:88`), helpers `text`, `value_type`, `owned_pair`, `operator`, `depth`, `lifetimes` (`:162-204`); `contract`, `fact`, `proposition`, `evidence` (`signature_contracts.rs`); `expression`, `static_argument` (`signature_expressions.rs:12,180`); `structural::validate` (`structural.rs:8`).
3. In/out: read-only over one `PackagePolicyCallable`; scope is built from `scope(&type_parameters, lifetime_count)` (`E/record/public_policy/validation.rs:119`).
4. Decides: role vs supply pairs (`validation.rs:33-53`): `PrivateAssumption` needs `AdmissionClaim` supply; `PrivateExternal` needs `ExternalRealization`; `Boundary` needs a boundary-class supply; `Public` must not have one. Published roles must have a published reach, declared invocations, and declared suspend/block (`:54-66`). Callable identity owner must equal `Package(self.package)` (`:30`). Checked reach `NoCheckedBody` is refused for `CheckedBody` supply (`:86-90`). Crash route: `alternative_guards` non-empty and `Truth` alone (`behavior.rs:117-127`). `Local` and `AssignmentIndex` runtime indices are rejected in published requirements (`structural.rs:18,79,170-174`).
5. Cases by shape: nesting ceiling `depth >= 128` (`signature.rs:188`, `structural.rs:184`); roles: at most one `Build` callable (`validation.rs:18-26`).
6. Same job elsewhere: crash-route guard validation is written twice: `behavior.rs:117-133` and `E/record/public_policy/validation/behavior.rs:62-85`. Progress premise validation twice: `behavior.rs:161-189` and `public_policy/validation/behavior.rs:35-53`. Synchronous invocation validation twice: `behavior.rs:142-159` and `public_policy/validation/behavior.rs:26-34`. The lifetime-partition normalization appears as a library call (`signature.rs:131`, `provider_policy/validation.rs:127`) and as hand-written code (`E/record/signatures/external_policy/validation.rs:31-45`).
7. Usage: `signature_parameters.rs` is a 3-line re-export of `public_policy::validation::signatures::parameters`.
8. Stated limits: `behavior.rs:1` crash summaries only on checked-body or `Requirement` supply.
9. Fail-open:
   - `behavior.rs:20-140`: `checked_may_suspend`/`checked_may_block` are compared to declared ceilings only when the declared value is `Some(false)` (`validation.rs:81-84`). A declared `None` accepts any checked value; that is the "open ceiling" reading (inferred).
   - `signature_contracts.rs:113-123`: `EvidenceProjection` ignores `source_kind` and `source_lane_position` (only `declaring_trait`/`requirement` are checked). Open.
   - `structural.rs:106-128`: integer binary kinds, widths and primitive types are not checked against each other; only non-emptiness and parameter bounds are checked. Open.

### `package/provider_policy.rs` (34) with `provider_policy/{authority,binding_validation,families,methods,plans,rows,service_validation,signature,validation}.rs` (110, 183, 45, 69, 73, 92, 96, 114, 211)
1. Purpose: records and validators for provider plans, rows, family coverage, and service methods.
2. Shape: `PackagePolicySelectedProviders::validate_canonical_structure` (`validation.rs:26`), `validate_families` (`:149`), `PackagePolicyProviderPlan::compare_canonical` (`:11`); `PackagePolicyProviderBinding::validate_native` (`binding_validation.rs:10`), `validate_canonical_structure` (`:32`), `validate_locator` (`:126`), `validate_producer` (`:164`), `matches_owner` (`:174`); `validate_service_methods` (`service_validation.rs:77`), `validate_service_structure` (`:9`); `validate_signature` (`signature.rs:42`); `validate_authority` (`authority.rs:50`); `validate_native_service_method` (`provider_policy.rs:27`).
3. In/out: reads plans and rows; row search by `requirement` identity.
4. Decides: native bindings only for `TargetProfile::LinuxArm64 | LinuxX64` syscalls (`binding_validation.rs:23-24,60`), imports by locator kind and target: `PeByName`/`PeByOrdinal` only on `WindowsX64`; `ElfVersioned` on Linux; `MachODylibSymbol` on `MacosArm64 | MacosX64` (`:131-155`). Syscall numbers must fit `u32` (`:25,55`). Plan `target` string must equal `self.target.target_name()` (`validation.rs:43`). Providers: `matches_owner` accepts a `Package(a)` owner only with `Some(a)` and `ToolchainSource(_)` only with `None` (`:178-181`).
5. Cases by shape: `validate_locator` and the syscall arm are per-target lists (see 4). `compare_canonical` orders by six fields (`validation.rs:12-21`).
6. Same job elsewhere: the "owner vs package" match appears again in `baseline/validation.rs:79-81` and `native_bindings.rs:62-64` using direct comparisons rather than `matches_owner`.
7. Unused: `PackagePolicyProviderEvaluatedSyscall` has an empty impl (`rows.rs:57`).
8. Stated limits: none.
9. Fail-open:
   - `validation.rs:43`: `!plan.target.is_empty() && plan.target != self.target.target_name()` accepts an empty plan target string. Open (no comment).
   - `validation.rs:37-43`: the provider-type / declaration / realizing-package condition combines five terms in one boolean; it admits `provider_type.is_empty() != provider_declaration.is_none()` when the declaration is `None` and (`realizing_package.is_none()` or all rows have `native_build`). Open.
   - `signature.rs:81-111`: signature vs calling consistency is run only inside `for calling in &self.calling`; a method with no calling plans is not cross-checked (read).

### `package/providers.rs` (111), `package/callables.rs` (28)
Vocabulary only: `PackageReviewCompilerIntrinsicExecution` (`providers.rs:19`, 19 variants incl. embedded `terminal_psi`, `native_intrinsics` types), grant selector kinds, `PackageReviewBoundaryApplication` (`Empty`/`Exact`), `PackageReviewCheckedServiceReach` (`callables.rs:6`, `NoCheckedBody`/`CheckedBody`).

### `package/terminal_permissions.rs` (82), `package/terminal_permissions_generics.rs` (107)
`PackagePolicyTerminalPermissions::validate_canonical_structure` (`terminal_permissions.rs:28`): services ordered, every permission names a method of its service, each service must have at least one permission (`:34-36`). `generics::validate` (`:7`) checks that each method's schema arguments are exactly `service-parameter:<ordinal>` identities. SPELLING: `write_service_parameter_identity` (`:67`) writes the string `named(name(service-parameter:{ordinal}))` inside `signature-type` framing and compares by prefix-stripping (`:43-47`); a 96-byte stack buffer (`StackIdentity`, `:78-81`) bounds the text; an ordinal needing more than 96 bytes errors (`.get_mut(...).ok_or(fmt::Error)`, `:89-91`). `PackageReviewTypeIdentity::service_parameter` (`:60`) is the only public constructor (2 grepped hits: definition and one call).
Fail-open: none seen.

### `policy_rows.rs` (168), `policy_rows_limits.rs` (57)
`PackagePolicyRowKind` (`policy_rows.rs:19`, 28 kinds) with `tag_and_name` (`:59`, one table for byte tag and spelling, comment `:7-9` says so "wire and text representations cannot drift"), `requires_runtime_consent` (`:52`), `audit_recommended_on_change` (`:100`), `row_change_audit` (`:133`). `PackagePolicyRow` (`:139`). `update_requires_decision` (`:158`) returns the same field as `initial_requires_decision` (`:157`): two accessors, one field (read). `PACKAGE_POLICY_ROW_VERSION = 11` (`:16`).
Usage: `update_requires_decision` 7 grepped hits, 2 of them on a different type (`lockfile/acceptance.rs:175`). `PackagePolicyRowKind::canonical_tag` is used in `R/compare/fingerprints.rs:174`.
Limits: `policy_rows_limits.rs:17-23`.

### `public_policy.rs` (63), `public_policy/{declarations,getters,signatures}.rs` (108, 86, 67)
Records for trait/requirement/conformance/domain/operator/alias/data shapes and type parameters. `type_aliases_for_version` (`public_policy.rs:38`) is a version-range match: versions in `BASELINE_WITHOUT_STRUCTURAL_PARAMETERS..BASELINE_WITH_TYPE_ALIASES` require `None`; `BASELINE_WITH_TYPE_ALIASES..=PACKAGE_POLICY_BASELINE_VERSION` require `Some` (`:47-52`); anything else is refused. 4 grepped hits.

### `public_policy/validation.rs` (198), `validation/{behavior,data,declarations,signatures}.rs` (86, 80, 267, 171)
`PackagePolicyPublicApi::validate_canonical_structure` (`validation.rs:27`), `roots` (`:81`), `root` (`:93`: owner must be `Package(package)`), `ordered_by` (`:101`, strict increase, "repeats or changes canonical order"), `ordered` (`:115`), `scope` (`:119`). Declaration validators `trait_shape` (`declarations.rs:24`), `operator` (`:107`), `conformance` (`:120`), `domain` (`:136`), `proposition` (`:181`), `application` (`:206`), `evidence` (`:227`). Signature telescope helpers `parameters`, `machine`, `child`, `formals`, `result`, `bounds`, `evidence_parameters` (`signatures.rs`). `data::validate` (`data.rs:14`).
Decides: trait application arity is checked only when the trait is declared in the same API (`declarations.rs:218-223`, `:254-263`); structural parameter positions strictly ordered and below parameter count (`:74-83`); returned borrow and qualification indices below `positions.len()` (`:86-99`); data members unique by name and identity, retired identities may not be reused (`data.rs:29-66`); an `Implementation` slot count must equal the number of conformance bounds (`signatures.rs:157-168`).
Same job elsewhere: `ordered`/`ordered_by` are the canonical-order checks used by every validator. Grepped: `ordered_by(` 22 lines, `ordered(&` 59 lines in `source-rust`.
Fail-open: `declarations.rs:218-223` and `:254-263`: when the referenced trait is not in this API (`find` returns `None`), no arity or requirement check runs. Open. A trait from another package is not checked here; whether it is checked elsewhere I did not read.

### `public_policy/validation/tests.rs` (143), `tests/structural_parameters.rs` (413)
In-file regression tests: contract binders start at declared statics (`tests.rs:127-141`), version compatibility of saved structural positions, alias rows, carried nominals, service application identity, package membership (`structural_parameters.rs:114-413`). They round-trip through `recover_canonical`, `canonical_text`, `recover_text_with_usage`.

### `representation/{mod,movement,policy,policy_validation,shape,target,target_validation}.rs` (48, 105, 98, 136, 41, 61, 45)
`PackagePolicyRepresentation` and validators `validate_canonical_structure` (`policy_validation.rs:8`), `validate_availability` (`:35`), `validate_selections` (`:59`), `validate_demands` (`:95`, groups demands by calling application with `compare_calling_application`); `PackageReviewRepresentationTarget::validate_canonical_structure` (`target_validation.rs:11`).
Decides: target profile enum carries 8 profiles including `UefiX64`, `CrossPlatformCli`, `LocalUnchecked`, `MacosX64` (`target.rs:8-17`); `CrossPlatformCli`/`LocalUnchecked` are validated only by pointer geometry (power-of-two alignment, size multiple of alignment, `target_validation.rs:13-25`); other profiles must equal `native_target()` architecture, object format, pointer size/alignment (`:26-43`). A selection must differ from a public producer surface only if the surface is found (`policy_validation.rs:77-90`). The producer interface must have exactly two trait arguments, and a selection `application.trait_arguments.len() == 2` (`:47,70`).
Cases by shape: "two trait arguments" hard-coded (`:47,70`).
Fail-open: `policy_validation.rs:77`: `if let Ok(index) = binary_search_by(...)`: a selection with no matching producer availability is not cross-checked. Open.

### `restricted_build.rs` (139), `semantic_bindings.rs` (34), `semantic_policy.rs` (45), `target_context.rs` (23)
`PackagePolicyRestrictedBuildRequest` and friends mirror `build_consent::Restricted*` types (`From` impls `:71,92,112`); comment `:5-9` says captured counts are deliberately recorded as `captured: bool` only. `SemanticBindingReviewCandidate::new` has a `debug_assert_eq!` on a schema digest (`semantic_bindings.rs:22-25`): a release build does not check it. 29 grepped hits across `source-rust`. `PackagePolicySemanticDependency` has `Declaration` and `CarriedType` variants. `PackagePolicyTargetContext::validate_canonical_structure` refuses an empty target tag only (`target_context.rs:16-20`); comment `:4-6` says "Uncatalogued tags remain valid inert disclosure".
Fail-open: `semantic_bindings.rs:22`: the digest agreement is `debug_assert` only. Open: a mismatching binding/schema pair reaches `Self { .. }` in a release build.

### `signatures.rs` (33), `signatures/{callables,external_policy,external_supply,signature_vocabulary,traits}.rs` (11, 70, 37, 59, 64), `signatures/external_policy/{signatures,validation}.rs` (49, 96)
Vocabulary: `PackageReviewTypeIdentity { canonical: String }` (`signature_vocabulary.rs:8`), `PackageReviewForeignLocator` (4 variants, `external_supply.rs:7`), `PackagePolicyExternalBinding` (7 variants, `external_policy.rs:29`), `PackagePolicyExternalExecutableSupply`. `PackagePolicyExternalExecutableSupply::validate_canonical_structure` (`validation.rs:17`): for a trait requirement it recomputes the lifetime partition by hand (`:31-45`) and compares to `requirement_lifetime_partition`.
Same job elsewhere: lines `:31-45` duplicate `application_identity::type_identity::normalize_requirement_lifetime_partition` called in `callable_policy/validation/signature.rs:131` and `provider_policy/validation.rs:127`.
Fail-open: none seen.

### Dead stubs (record-wide)
- Empty inherent impl blocks of the form `impl X {}` in `E/record`: 31 grepped lines (`grep -rE '^impl [A-Za-z<>_, ]+ \{\}$'` under `E/`). Examples: `authority.rs:93`, `authority_expressions.rs:41,60,68,76`, `callbacks.rs:84-89`, `conformances_policy.rs:49`, `signatures/traits.rs:21`.

---------------------------------------------------------------------------------------------------

## Part 4. package-manager review module (`R/`)

### `inspect_packages.rs` (403)
1. Purpose: the read-only `omega packages` inspection transaction.
2. Shape: `PackageInspectionOptions` (`:38`), `PackageInspectionOutcome` (`:47`; `report`, `complete`, `requires_decision`), `PackageInspectionError` (`:56`, wraps `PackageCommandError`), `PackageInspectionTransaction` (`:103`) `open` (`:109`) `prepare` (`:122`), `PreparedPackageInspection` (`:169`) with `acquire_root` (`:190`), `source_acquisitions` (`:217`), `complete_source_resolution` (`:264`), `review_target` (`:281`), `finish` (`:339`), `select_targets` (`:355`).
3. In/out: reads `build.omg`/`omega.lock` once (`read_pair`) and again at `finish`; consumes per-target fresh reviews; writes only `outcome.report` (cap 64 MiB, `:183`).
4. Decides: `select_targets` (`:355`): empty request means every target in the accepted lock, else the host if supported; a requested list is sorted by `identity().as_str()` (string order), duplicates by equality refused (`:375-378`); a requested target absent from the lock is refused (`:379-387`). `review_target` requires targets in order (`:288`). `finish` fails if the file pair read differs from `before` (`:346`). Source context identity compared via `matches!(... source_context == &context)` (`:228`).
5. Cases: `source_acquisitions` takes pins from `accepted.targets().first()` only (`:221-225`): the first target's source graph serves all targets.
6. Same job elsewhere: `transaction/mod.rs` (not read) runs the same acquire -> review pipeline for install/update (crate map says so); the grep did not compare the two.
7. Usage: all pub items reached through `R/mod.rs:46-49`; not individually grepped.
8. Stated limits: comment `:4-9`; `:53-54` "retain its existing typed failure owner".
9. Fail-open:
   - `review_target` (`:310-319`): when fresh analysis fails, `complete` is set false but `requires_decision` is untouched. `application/src/commands/packages.rs:109,112` reads both flags (grepped); a caller that read only `requires_decision` would see "no decision needed" for an unanalyzed target. Traced to one caller that reads `complete` first (line order observed, not the full branch).
   - `:221-225`: pins from the first accepted target only; a second target with different pins is checked later by `mutable_local_graph_matches` (`sources.rs:40-51`), not here (traced).

### `inspect_packages/report.rs` (467), `report/policy.rs` (365), `report/policy_callables.rs` (209), `inspect_packages/sources.rs` (76)
1. Purpose: bounded text report of accepted lock vs fresh findings; per-target review that compares fresh policy to the accepted baseline.
2. Shape: `render` (`report.rs:18`), `contents` (`:82`), `checked_context`, `build_snapshot` (`:213`), `graph` (`:278`), `package_key` (`:308`), `resolution` (`:349`), `path` (`:371`), `Output` (`:393`, byte-capped `Write`), `Name` (`:430`); `policy::render` (`policy.rs:14`), `Names` (`:213`) with `providers`, `representation`; `Names::callable`, `reach`, `invocations`, `method` (`policy_callables.rs`); `sources::review` (`sources.rs:24`), `PackageInspectionSources` (`:18`).
3. In/out: `render` first cross-checks `fresh` against `accepted` (`:30-70`): target match, source fingerprint equal, count of reviews = sum of occurrence contexts, each package's review present with equal context/resolution/package/target; otherwise returns an "analysis context mismatch" error. `sources::review` refuses if the executed source graph differs from the accepted one via `mutable_local_graph_matches` (`:40-51`), then calls `review_resolved_packages` and `compare_package_policy_changes`.
4. Decides: identity of package keys (`change.key() == package.key()`); context equality; no source spelling beyond display. Name display: `Name::fmt` looks a package up by `owner` digest and prints its name, adding `[hex]` only when two packages share a name (`:446-455`) (a display decision, not a judgment).
5. Cases: `build_snapshot` prints one of captured / unbound live root / none (`:218-229`); staged output entry kinds `Directory`, `File`, `Symlink` (`:247-266`).
6. Same job elsewhere: `Names::reach` (`policy_callables.rs:123`) and `Names::invocations` (`:146`) are two near-identical list printers.
7. Usage: all private to the `inspect_packages` module.
8. Stated limits: `report.rs:98` "--details shows complete canonical policy"; `policy.rs:46-49` "public type aliases unavailable in this review format".
9. Fail-open: none seen. The report prints `contract {contract:?}` Debug text (`policy.rs:362`), a display only.

### `inspect_source/{execution,mod,report,request}.rs` (73, 9, 38, 93)
1. Purpose: `omega packages` source inspection for a locator: parse, resolve, report.
2. Shape: `inspect_package_source_locator` (`execution.rs:63`), `inspect_package_source` (`:52`), `SourceAdapter::parse` (`request.rs:13`, strings `"local"` and `"git"`), `PackageSourceRequest::parse` (`:40`), `file_url_path` (`:68`), `reject_local_rev` (`:80`), `PackageSourceInspection::to_text` (`report.rs:17`).
3. In/out: `inspect_package_source` brackets the work with `storage.verify_path_identity()` before and after (`execution.rs:57,59`).
4. Decides, SPELLING: adapter by string `"local"`/`"git"` (`request.rs:15-16`); `file://` prefix and leading `/` for local locators (`:69-72`); empty after `trim` refused (`:46`).
5. Cases: only two adapters; local path with revision refused.
6. Usage (grepped): `inspect_package_source` is `pub` but referenced only in `execution.rs` (3 lines, all in this file); `inspect_package_source_locator` is called from `application/src/commands/packages.rs:139`; `to_text` called once, `packages.rs:146`.
7. Fail-open: none seen. `execution.rs:17`: for a local path it calls `resolve_local_source(&path, limits)` without the resolver; the checks around it are the identity checks at `:57,59` (read).

### `mod.rs` (52), `timings.rs` (91), `package_change.rs` (155), `package_change_error.rs` (74)
- `mod.rs`: module wiring and re-exports only.
- `timings.rs`: env/flag-gated stage timer printing to stderr. Gate `OMEGA_REVIEW_TIMINGS` (`:14,57`) or a thread-local flag (`:17`). Usage (grepped): `timings::stage` 8 hits, `subject_stage` 4, `timings::note` 4, `timings::requested` 1 (`candidate/passes.rs:58`).
- `package_change.rs`: `review_package_change_reusing` (`:101`) checks a closure for one target and compares to the accepted section; `PackageChangeReview::propose_lock_target` (`:60`) requires `resolution.comparison() == self.changes.fingerprint()` (`:65`), `all_required_changes_accepted` (`:70`), then re-verifies transitive source custody (`:73-78`) and compiler identity (`:64,145-154`) before building the lock target. `verify_compiler_identity` compares a 32-byte digest.
- `package_change_error.rs`: `PackageChangeError` enum and `Display`.
Fail-open: none seen. `compiler_identity` is read twice in `review_package_change_reusing` (`:115,128`) and once again in `propose_lock_target` (`:64`).

### `restricted_build_grants.rs` (163)
1. Purpose: at execution time, admit a restricted Build request only if its canonical meaning text equals a text the lock retained for that package and context.
2. Shape: `UngrantedRestrictedBuildRequest` (`:32`, `project` `:40`), `RestrictedBuildCheckpoint` (`:91`; `derive` `:96`, `grants` `:120`), `CheckpointRestrictedBuildGrants` (`:142`) implementing `RestrictedBuildGrants::admit` (`:150`).
3. In/out: `derive` collects `canonical_text()` of every `RestrictedBuildRequest` acceptance row per `(package, context)` into a `BTreeSet<String>` (`:109-112`). `admit` renders the request's acceptance text and tests set membership (`:158`).
4. Decides, SPELLING: membership of the canonical meaning string in the retained set. `grants` finds the entry by `(package identity, context)` equality (`:133-135`).
5. Cases: none.
6. Same job elsewhere: `permission_acceptance.rs:69-116` compares consent rows by `(digest, text)`.
7. Unused code: `RestrictedBuildCheckpoint` and `CheckpointRestrictedBuildGrants` have no references outside this file in `source-rust` (grepped `RestrictedBuildCheckpoint|CheckpointRestrictedBuildGrants`: all hits are inside `restricted_build_grants.rs`). The other pub type, `UngrantedRestrictedBuildRequest`, is used at `candidate/error.rs:17,93,193`. This is consistent with `OWNER_CODE_CONCERNS.md` naming this file (crate map).
8. Stated limits: comment `:1-16`.
9. Fail-open: none seen. A package/context absent from the checkpoint gets an empty set (`unwrap_or_default`, `:137`), so every request is refused.

---------------------------------------------------------------------------------------------------

## Part 5. source acquisition: capture crates (`A/capture/`)

### `capture/source_capture.rs` (12), `capture/tree.rs` (264), `capture/filesystem.rs` (271), `capture/read_accounting.rs` (39), `capture/tests.rs` (197)
1. Purpose: in-memory immutable source trees and a filesystem view that routes paths into them.
2. Shape: `CapturedSourceNode` (`tree.rs:10`: `Directory`, `File{bytes, executable}`, `Symlink{target}`), `CapturedSourceTree` (`:17`, `BTreeMap<PathBuf, Node>`), `new` (`:22`), `canonicalize` (`:62`), `exists`/`is_file`/`is_dir`/`read_to_string` (`:142-183`); `SourceFileSystem` (`filesystem.rs:10`) with `add_capture` (`:48`), `join_captures` (`:23`), `canonicalize` (`:78`), `exists`/`is_file`/`is_dir`/`read_to_string` (`:96-139`), `captured_files` (`:145`), `route` (`:175`), `route_capture` (`:227`); counters `source_file_read_count` and `record_source_file_read` (`read_accounting.rs:7,11`).
3. In/out: `tree.canonicalize` resolves `..` and symlinks inside the tree with a 64-link ceiling (`:105`); a link target must be a contained relative path (`:119-125`). `SourceFileSystem::route` maps an absolute path to a mounted capture root or to the live disk.
4. Decides: entry paths must be non-empty, without `..`, and their spelling must equal the rebuilt component path (`validate_entry_path`, `:186-214`; on Windows `/` is replaced with `\` before comparing, `:201-205`). Every parent must be an explicit `Directory` entry (`:36-50`). A capture root must be absolute, free of `.`/`..`, canonical and NUL-free (`filesystem.rs:49-67`).
5. Cases: `route` follows host symlinks up to 64 times to find a capture root (`:185-217`); a `..` at a capture root errors with `PermissionDenied` "path escapes captured root" (`:233-238`).
6. Same job elsewhere: `read_to_string` UTF-8 check and error text exist in both `tree.rs:169-173` and `filesystem.rs:130-135`.
7. Usage (grepped): `has_captured_namespace` 4 external callers in `compiler/build/inputs/source/...` (`checkpoint.rs:147`, `files.rs:233`, `frontier.rs:550`); `captured_files` 2 external callers (`entry_contract_seed.rs:192`, `logical.rs:201,604`); `source_file_read_count` has no caller outside `read_accounting.rs` (its own test); `record_source_file_read` called from `compiler/build/inputs/source/source_snapshot_file_read.rs:104`, `A/tree/filesystem.rs:114` and `filesystem.rs:125`.
8. Stated limits: `filesystem.rs:141-144` "Logical module discovery must not reopen a live directory".
9. Fail-open: `route` (`filesystem.rs:176-178, 213-215`) returns `Physical(path)` (the live disk) when there are no mounted captures, or when the path is under no mounted capture and no host symlink on it redirects into one. So a read of a path outside every capture reads the real file. The tests state it as intended (`capture/tests.rs:102`: a path outside the project reads "outside" from disk). Traced: `exists`/`is_file`/`is_dir` return `false` on any routing error (`:100,108,116`). What a wrong program gets: a source import that names a path outside the captured roots succeeds from the live disk (inferred from `:102`); whether the compiler rejects such imports earlier was not traced.

### `capture/selection/selection.rs` (635)
1. Purpose: choose which members of a source tree to capture, without opening files.
2. Shape: `SourceCaptureRequest` (`:38`), `entire_tree` (`:45`), `new` (`:51`), `SourceCaptureObligation` (`:32`), `SourceSelectionError` (`:102`), `select_source_members` (`:157`), `charge_scoped_member` (`:264`), `canonical_filesystem_metadata_path_is_canonical` (`:13`).
3. In/out: takes callbacks `lookup`, `is_directory`, `children`, `validate_path`, `selection_error`; returns `BTreeMap<path bytes, Member>`.
4. Decides: path canonical form on bytes (no NUL, no backslash, no empty/`.`/`..` component, no leading `/`, `:13-29`); a user entry that has a component of the form letter+`:` is refused as drive-qualified (`:56-59`); entries may not repeat or nest inside another (`:68-86`); row limit `count >= row_limit` (`:276`); path-byte budget (`:285-290`). Required absent entry is an error, optional is skipped (`:227-233`). A non-directory ancestor makes the member unreachable (`:191-197`).
5. Cases: `entire_tree` is the only way to select the empty path (`:43-49`, `:177-183`).
6. Usage: `canonical_filesystem_metadata_path_is_canonical` has 30 grepped lines across `source-rust`.
7. Stated limits: `:11` `FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT = 16 MiB`.
8. Fail-open: none seen. An empty `SourceCaptureRequest::new([])` selects only the root directory entry (test `:405-407`).

---------------------------------------------------------------------------------------------------

## Part 6. source acquisition: custody (`A/custody/`)

### `custody/mod.rs` (31), `lock.rs` (453), `publication.rs` (427), `platform.rs` (440), `tree.rs` (498)
1. Purpose: capability-rooted checks that the per-user cache is owned, private and unreplaced; per-entry locks; private staging and atomic publication.
2. Shape: `CacheEntryLock` (`lock.rs:35`) with `acquire_from_parent` (`:126`), `acquire_local_from_parent` (`:210`), `verify_path_identity` (`:265`), `verify_cache_lock_path_identity` (`:315`), `verify_retained_cache_parent_path` (`:349`); `CacheLockBudget` trait (`:30`); `PendingCacheEntry` (`publication.rs:185`) with `create` (`:194`), `publish` (`:284`), `ProvisionalCacheDirectory` (`:319`, drop removes if armed), `publish_cache_directory_from_open_parent` (`:104`), `create_private_cache_directory` (`:355`), `retain_private_cache_directory` (`:373`); `platform.rs`: `same_capability_file_identity` (`:43`), `verify_capability_cache_node_owner_and_mode` (`:53,79`), `verify_cache_node_owner_and_mode` (`:88,114`), Windows identity checks (`:122-244`), macOS ACL checks (`:248-440`); `tree.rs`: `CacheCustodyKind` (`:42`), `verify_cache_custody_root` (`:132`), `verify_retained_cache_custody_root` (`:150`), `verify_cache_ancestry` (unix `:230`, windows `:271`, other `:294`), `verify_cache_custody_from_open_root` (`:334`), `open_cache_custody_directory` (`:456`), `read_bounded_cache_record*` (`:54,72`).
3. In/out: reads filesystem metadata through retained directory handles; returns `SourceResolveError` (`GitCacheInvalid` or `LocalSnapshotInvalid` chosen by `cache_custody_invalid`, `tree.rs:489-498`).
4. Decides: identity is device+inode (Unix) or volume serial + file index (Windows) (`platform.rs:43-50`, `lock.rs:381-406`); owner must be the effective uid and mode must have no `0o022` bits for non-symlinks (`platform.rs:61-74`); ancestors of the cache root may be owned by the effective user or by uid 0, and writable-by-others ancestors are tolerated only with the sticky bit (`tree.rs:245,253`); every cache tree entry must be a directory, regular file or symlink, otherwise "unsupported filesystem entry kind" (`:431-437`); logical byte total vs ceiling, entry count vs `CACHE_CUSTODY_ENTRY_LIMIT`, depth vs `CACHE_CUSTODY_DEPTH_LIMIT` (`:347-441`). Stage names are `.git-stage-<pid>-<seq>` (`publication.rs:207-208`). `private cache directory` must have mode `0o700` exactly on Unix (`:413`).
5. Cases by shape: `verify_cache_ancestry` has three cfg arms (unix, windows, other). The macOS ACL checks are a fixed set of eight functions each with a non-macOS stub (`platform.rs:248-440`). The Windows identity checks are a fixed set of four functions with non-Windows stubs, one by `non_windows_verify_stub!` macro (`:28-41,216,246`). `PendingCacheEntry::create` retries up to 128 sequence numbers (`:205`).
6. Same job elsewhere:
   - `verify_capability_cache_node_owner_and_mode` (`platform.rs:53-76`) and `verify_cache_node_owner_and_mode` (`:88-111`) have the same body on `CapabilityMetadata` and `std::fs::Metadata` respectively.
   - The wait-for-lock loop is repeated in `acquire_within` (`lock.rs:99-111`, test-only) and `acquire_from_parent` (`:139-151`), and the timeout loop in `acquire_local` (`:173-189`, test-only) and `acquire_local_from_parent` (`:224-240`).
   - "classify with `symlink_metadata`, open with no-follow, compare identity" appears in `publication.rs:373-390` (`retain_private_cache_directory`), `tree.rs:456-487` (`open_cache_custody_directory`), `git/cache/custody.rs:28-49` (`open_retained_git_directory`), `platform.rs:122-177,325-360`.
   - `synchronize_cache_parent` is defined in `publication.rs:170-183` (private) and `git/cache/invalidation.rs:79-93` (`pub(super)`); grepped `synchronize_cache_parent` 8 lines.
7. Usage (grepped): `verify_cache_node_owner_and_mode` 5 lines; `verify_capability_cache_node_owner_and_mode` 16. `PendingCacheEntry::create` has one non-test caller (`git/cache/creation.rs:58`) and takes a `_cache_identity` argument it never uses (`publication.rs:197`).
8. Stated limits: `platform.rs:5-8` "Non-Unix targets enforce portable kind, handle identity, and bounded topology checks without promoting host ACLs into resolver isolation evidence"; `mod.rs:10-16` threat model.
9. Fail-open:
   - `platform.rs:78-85,113-120`: on non-Unix hosts (Windows), the owner/mode check returns `Ok(())`. Stated (header `:5-8`).
   - `platform.rs` macOS-ACL functions on non-macOS return `Ok(())` (`:274-280,315-322,385-401,431-440`). Stated (header).
   - `tree.rs:293-296`: on a target that is neither Unix nor Windows, `verify_cache_ancestry` returns `Ok(())`. Open (no comment). Not one of the four hosted targets (inferred).
   - `tree.rs:245`: uid 0 ancestors are accepted for any non-root effective user. Open (no comment).
   - `tree.rs:253`: a group/other-writable ancestor with the sticky bit passes. Open (no comment; the check message says "without sticky-entry protection", `:257`).
   - `publication.rs:177-183`: on non-Unix `synchronize_cache_parent` is a no-op (no fsync of the parent directory after rename). Open.

### `custody/tests/{mod,limits,locks,publication,snapshots,repository_integrity,traversal,macos_acl,windows_ancestry,windows_siblings}.rs` (40, 47, 209, 67, 176, 199, 116, 244, 131, 84)
Tests, grouped by invariant (`mod.rs:1-3`). They pin: byte ceiling (`limits.rs:11`), no-follow bounded reads (`:26`), replaced lock/parent/stage detection (`locks.rs:16,40`; `publication.rs:44`), cold-key lock creation races (`locks.rs:76`: 8 contenders x 32 keys), whole-resolution lock budget (`:150`), error precedence namespace/invalidation over operation failure (`:175`), Git cache config substitution, symlink and hard link rejection (`repository_integrity.rs`), macOS ACL allow entries (`macos_acl.rs`, cfg macOS), Windows ancestry and sibling reconciliation (cfg windows). Several tests are `#[ignore]`d on Windows with the reason "Windows prevents replacement while the retained ... handle is open" (`locks.rs:38`, `publication.rs:42`, `snapshots.rs:16,58,116`, `repository_integrity.rs:75,96`). The Windows ancestry test checks that a junction parent is rejected (`windows_ancestry.rs:108-131`). I did not run any test.

### `error.rs` (361)
`SourceResolveError` (`:10`), 36 variants, one `Display` match (`:133-318`), `GitSubmoduleEdge` (`:128`), and helper constructors `cache_invalid`, `local_snapshot_invalid`, `git_tree_invalid`, `too_many_files`, `too_many_bytes`, `local_source_changed` (`:322-361`). Decides nothing. `GitSubmodulesUnsupported` carries `edges` (`:82`).

---------------------------------------------------------------------------------------------------

## Part 7. source acquisition: Git (`A/git/`)

### `git/cache/{mod,configuration,creation,custody,identity,invalidation,repository,retry,snapshots}.rs` (31, 77, 241, 239, 33, 93, 298, 75, 55)
1. Purpose: build, verify, invalidate and discard bare Git repository cache entries inside the per-user cache.
2. Shape: `create_git_cache_entry` (`creation.rs:44`), `discover_git_object_format` (`:180`), `parse_git_remote_object_format` (`:205`); `replace_canonical_git_control_file` (`configuration.rs:17`); `verify_git_repository_tree_from_open_root` (`custody.rs:72`), `reject_retained_git_path` (`:195`), `open_retained_git_directory` (`:28`), `verify_retained_git_directory_identity` (`:51`); `git_cache_identity` (`identity.rs:9`), `git_cache_metadata` (`:22`); `invalidate_git_cache_entry_from_open_parent` (`invalidation.rs:46`); `VerifiedGitRepository` (`repository.rs:48`) with `open` (`:81`), `verify_identity` (`:131`), `verify_current` (`:162`), `read_canonical_config` (`:202`), `restore_canonical_config` (`:220`), `open_or_create_snapshots` (`:247`), and `cache_git!` macro (`:62`); `discard_incomplete_entry` (`retry.rs:27`), `may_preserve_cache` (`:65`); `RetainedGitSnapshots` (`snapshots.rs:18`).
3. In/out: runs sealed Git commands through `GitExecutor` (`creation.rs:84-94`); writes `GIT_CACHE_METADATA` with mode `0600` and `create_new` (`:116-158`); config bytes must equal `GIT_CONFIG_SHA1` or `GIT_CONFIG_SHA256` (`repository.rs:211`).
4. Decides: metadata bytes equal `git_cache_metadata(url, rev, transport)` exactly (`repository.rs:180`); the object format comes from a full object id (`is_object_id`) or from `ls-remote --symref` rows, which must all agree (`creation.rs:186-234`); `custody.rs:106-111,130-135` refuses symlinks and special files in the repository; the cache entry identity is a SHA-256 over policy, URL, revision and transport tag (`identity.rs:14-19`). `may_preserve_cache` keeps a verified cache for a fixed list of errors: `Git`, `GitTimedOut`, `GitOutputOverflow`, `GitResolutionTimedOut`, `GitResolutionCommandLimit`, `GitResolutionCapturedOutputLimit` (`retry.rs:65-74`).
5. Cases by shape: forbidden paths are the three literals `objects/info/alternates`, `objects/info/http-alternates`, `commondir` (`repository.rs:187-197`); object format is SHA-1 or SHA-256 (`creation.rs:77,95-98`); `may_preserve_cache` list (see 4).
6. Same job elsewhere: see custody entry in Part 6, item 6.
7. Usage (grepped): `may_preserve_cache` 6 lines.
8. Stated limits: `mod.rs:13-22`; `retry.rs:4-10`.
9. Fail-open:
   - `custody.rs:187-193`: on non-Unix the multiply-linked-file check returns `Ok(())`. Stated (header `:5-6` says "`nlink != 1` on Unix").
   - `custody.rs:209`: `reject_retained_git_path` returns `Ok(())` when an intermediate directory is `NotFound` (the leaf cannot exist). Sound by construction; no comment.
   - `retry.rs:41`: `discard_incomplete_entry` returns `Ok(false)` (keeps the entry) when `GIT_CACHE_METADATA` exists. Stated (comment `:4-8`: only incomplete entries are discarded).

### `git/executable/{mod,budget,executor,selection,validation}.rs` (7, 4, 225, 352, 49)
1. Purpose: choose one host Git binary outside package-controlled roots and carry launch/time/output budgets for one resolution.
2. Shape: `PrimaryGitSelection` (`selection.rs:18`) `capture` (`:23`), `capture_optional` (`:31`), `verify_outside` (`:51`), `resolver_package_controlled_roots` (`:216`); `GitExecutor` (`executor.rs:57`) `selected` (`:68`), `begin_launch` (`:171`), `remaining_time` (`:190`), `GitCommandDeadline` (`:27`); `verify_git_executable_launchability` (`validation.rs:8`); `GitCapturedOutputBudget` is a re-export of `bounded_process::BoundedCaptureBudget` (`budget.rs:4`).
3. In/out: automatic selection reads `PATH` once (`selection.rs:35-39`), keeps only absolute entries (`:93-95`), skips excluded or non-launchable candidates silently (`:97-105`), freezes the canonical path.
4. Decides, SPELLING: Windows launchability is "file name equals `git.exe` ignoring case" (`validation.rs:44-49`); the automatic file name is `"git"` or `"git.exe"` (`selection.rs:151-158`). Unix launchability is "any execute bit" `mode & 0o111` (`validation.rs:24`). Exclusion is `starts_with` on both the given and the canonicalized root (`selection.rs:186-194`); a root that cannot be canonicalized contributes only its lexical form (`:179`).
5. Cases: `GitCommandDeadline::new` records `resolution_timeout` only when the remaining whole-resolution time is `<= GIT_COMMAND_TIMEOUT` (`executor.rs:36`), so timeouts project as resolution timeouts only then.
6. Same job elsewhere: `selection.rs` `ExcludedRoots::contains` (`:186`) and `resolver-execution` `require_outside_roots` (`path_custody.rs:68-93`) both test "inside a package-controlled root" with the same two-prefix test. Two copies.
7. Usage: `budget.rs` has 11 grepped references to the alias name.
8. Stated limits: `selection.rs:9-10` "`capture_optional` allows local-only storage sessions to initialize when Git is absent".
9. Fail-open: none seen that accepts a wrong executable. Skipping a candidate that fails launchability (`selection.rs:103`) ends in `Ok(None)` and then `GitExecutableUnavailable`, not acceptance.

### `git/git_command.rs` (124)
`GitCommandCapture` (`:28`), `run_git_output` (`:44`), `run_git`, `run_git_stdout`, `run_git_bytes_stdout` (`:71-124`). `run_git_output` builds a sealed command, claims a launch slot, runs with the stdout limit `GIT_STDOUT_LIMIT` and stderr limit `GIT_STDERR_LIMIT`, then reconciles the result with `executor.verify_budget()` (`:68`). The wrappers require a zero exit; a failure becomes `SourceResolveError::Git { operation: "command", ... }` (`:118-122`): `operation` is the literal `"command"` for all callers of `run_git_bytes_stdout`. `run_git_stdout` lossily decodes UTF-8 (`:95`). Fail-open: none seen.

### `git/execution/src/{lib,path_custody,resolver_execution}.rs` (29, 110, 101), `git/execution/src/tests/{mod,unix,validation}.rs` (28, 54, 144)
1. Purpose: crate `resolver-execution`: freeze a resolver executable and prepare a bounded process per phase.
2. Shape: `ResolverExecutionBackend::open` (`resolver_execution.rs:38`), `prepare` (`:66`), `ResolverExecutionPhase` (`:25`: `TransportDiscovery`, `RepositoryInitialization`, `Fetch`, `RepositoryInspection`), `resolver_process_limits` (`:87`: 120 s CPU, 8 GiB address space, 1 GiB file size, 256 open files, 16 processes, 2 GiB per process, 4 GiB aggregate); `require_absolute`, `require_lexically_canonical_bounded_path` (32 KiB limit, `path_custody.rs:8,20`), `require_regular_file`, `require_outside_roots` (`:68`).
3. In/out: `open` checks the executable path and its canonicalized target against the roots; `prepare` sets only the working directory and the limits.
4. Decides: path is absolute, equal to its component rebuild, within 32 KiB, outside roots, and a regular file. The `root_name` text is chosen by phase (`:71-77`) but all phases apply the same checks.
5. Cases: four phases; phase affects only the error label.
6. Same job elsewhere: see executable entry (two "outside roots" implementations).
7. Usage: tests in `tests/` assert the limit values (`validation.rs:23-38`) and the file-descriptor limit on Unix (`unix.rs:53`).
8. Stated limits: `lib.rs:14-16` "Nothing in this crate produces canonical package evidence".
9. Fail-open: `path_custody.rs:77`: `canonicalize().ok()` for a root; an unresolvable root is checked lexically only. Traced: a root that does not exist cannot contain the executable after canonicalization, so no wrong executable passes (inferred).

---------------------------------------------------------------------------------------------------

## Coverage

All of the following were read in full with no ranges skipped:
- 177 rows of `units/unit_083.tsv`: rows 1-177, line counts as listed, total 21,964 lines (command: `awk -F"	" "{s+=$4} END{print s, NR}" units/unit_083.tsv` printed `21964 177`).
- Read in full: all 177 files, every line.
- Skimmed: none.
- Not read: none.

Commands run: `cat -n` of each file in batches; `grep -rnE`/`grep -rE ... | wc -l` over `source-rust/subsystems` and `source-rust/application` for the counts above (with `_recycling` filtered). No build, no test run.

## Five things a maintainer most needs to know about this unit

1. Recovery is closed and bounded, but one value is read by spelling. All enum vocabularies in `E/encoding/recovery/policy/` are tag bytes with `InvalidTag` on anything else, and every list passes through `Reader::sequence` with a hand-written minimum size; but an integer literal's arithmetic domain is a string compared against `"Exact"/"Wrapping"/"Saturating"/"Trapping"` (`structural_expressions/scalars.rs:40-45`, written at `encode/values/crashes/vocabulary.rs:80-81`).
2. Many identity checks are text matches on framed strings, not on structured values: `matches_policy_requirement` / `matches_named_callable` / `matches_applied_trait_declaration` (`record/contracts/expressions_operator_policy.rs`), `service-parameter:<n>` identities (`record/package/terminal_permissions_generics.rs:67-76`), and target names via `identity().as_str()` (`selected_providers.rs:63-71`, `baseline/validation.rs:82-95`). Toolchain owner digests are deliberately ignored by those text matches (`expressions_operator_policy.rs:141`, test `:236`).
3. Validators exist twice or more for the same shapes. Crash-route, progress-premise and synchronous-invocation validation are written in both `callable_policy/validation/behavior.rs` and `public_policy/validation/behavior.rs`; the lifetime-partition normalization is hand-written once (`external_policy/validation.rs:31-45`) and a library call twice; the cache-node owner/mode check is duplicated for `Metadata` and `CapabilityMetadata` (`custody/platform.rs:53,88`); the "outside package-controlled root" test is in both `git/executable/selection.rs:186` and `git/execution/src/path_custody.rs:68`.
4. Platform custody is strictest on Unix. Windows gets handle identity and ancestry reparse checks but no owner/mode, no ACL, no hard-link check, no parent fsync; macOS adds extended ACL checks; the "other target" ancestry check returns `Ok(())` (`custody/tree.rs:293-296`). These are stated for the first two and not stated for the third.
5. Code that looks live but has no outside caller: `RestrictedBuildCheckpoint` / `CheckpointRestrictedBuildGrants` (`R/restricted_build_grants.rs:91,142`), `TerminalPermissionAcceptance::compare_policy` (`E/permission_acceptance.rs:39`), `source_file_read_count` (`A/capture/read_accounting.rs:7`, test-only), `inspect_package_source` as a `pub fn` (`R/inspect_source/execution.rs:52`, used only by its sibling), and an unused `_cache_identity` parameter (`A/custody/publication.rs:197`). 31 empty `impl X {}` blocks sit in `E/record`.

## What you are unsure about

- Whether `lockfile`/`compiler` code rejects the shapes I marked "open" in validators (overlapping record fields, trait-application arity for foreign traits, empty `plan.target`, `EvidenceProjection` source fields). I read only this unit.
- Whether source imports to paths outside every captured root are refused before `SourceFileSystem::route` reads the live disk (`A/capture/filesystem.rs:213-215`); the test at `A/capture/tests.rs:102` shows the read succeeding at this layer.
- The count of non-test callers of `recover_text_with_usage` for the baseline type: the grep matched a similarly named method on another type; I did not split them.
- `PackagePolicyRecoveryUsage`/`PackagePolicyRowLimits` default numbers (64 MiB, 128 MiB, 65_536, ...): I report them as written; I did not trace why they differ.
- The `E/encoding/recovery/policy/{baseline,identity,calling_application,conformance,declarations,contracts,behavior,callable_policy,external,intrinsic}.rs` files that the readers import from are outside this unit; statements about them rely on grep lines only.
- (none further on line totals; the total 21,964 comes from the awk command above.)
