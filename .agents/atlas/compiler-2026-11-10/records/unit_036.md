# Unit 36 notes

Files covered (203 rows of unit_036.tsv, all paths under `omega-rust/omega/packages/`):
- `review-evidence/src/encoding/encode/**` rows 1-50 (encoder, membership, policy_rows, public_api, representation, selected_providers, terminal_permissions, values) plus `encode/mod.rs`
- `review-evidence/src/encoding/mod.rs` (row 51)
- `review-evidence/src/encoding/recovery/**` rows 52-100 (everything under `recovery/policy/`)
- `review-evidence/src/lib.rs`, `permission_acceptance.rs` (rows 101-102)
- `review-evidence/src/record.rs` and `record/**` rows 103-194
- `source-acquisition/src/custody/{lock,mod,platform,publication}.rs` and `custody/tests/{limits,locks,macos_acl,mod,publication}.rs` rows 195-203

All paths below are relative to `review-evidence/src/` unless they start with `source-acquisition/`. Not in this unit but referenced: `encoding/encode/{baseline,callable_policy,calling,declarations}/**`, `encoding/recovery/` row recovery (none exists under recovery/ in this unit), `capture/**`, `source-acquisition/src/custody/tree.rs` and the other custody test files.

---------------------------------------------------------------------------

## A. encoding/encode/encoder.rs and encoder/** (rows 1-7)

**Purpose.** One `Encoder` struct produces canonical little-endian binary, a canonical named-text markup, row measurements, and a membership walk. Which job it does is chosen by which fields are set.

**Shape.**
- `Encoder` (encoder.rs:11) fields: `output`, `text: Option<Writer>`, `membership: Option<&mut dyn Observer>`, `membership_error`, `encoded_bytes`, `maximum_bytes`, `exceeded`, `measure_only`, `retain_binary_with_text`, `policy_elements: Option<usize>`, `policy_depth`, `maximum_policy_depth`.
- Constructors: `bounded` (:27), `policy_bounded` (:44, ceilings from `PackagePolicyRecoveryLimits::default()`), `policy_bytes` (:53, 4 MiB), `policy_preallocated` (:66), `policy_text` (encoder/text.rs:48), `policy_membership` (encoder/membership.rs:7), `row_measure`/`row_output` (encoder/policy_rows.rs:5,20).
- Combinators: `sequence` (:155), `option` (:184), `field_by`/`option_field`/`sequence_field` (:206-231), `nested` (:84), `field`/`record`/`scope` (encoder/text.rs:64-97), scalar writers via macros (encoder/scalars.rs:6,37).
- Text `Writer` (encoder/text/writer.rs:14): fixed array `scopes: [Scope; 1024]`, constructors `new` (:26), `measured` (:45), `preallocated` (:52); `expected: Option<&[u8]>` verifies output in place (:101).
- `encode_canonical_component!` macro (encoder.rs:283): generates `canonical_bytes()` for a component given magic, version, encode fn.

**Inputs/outputs.** Reads record values passed by callers; produces `Vec<u8>`, `String`, or `(bytes, text, elements)` metrics; membership mode produces no bytes.

**How it decides.**
- Byte ceiling: `encoded_bytes + len > maximum_bytes` sets `exceeded` (encoder.rs:133-140). Aggregate element ceiling: `policy_elements` checked_sub (:103-111). Depth: `policy_depth >= maximum_policy_depth` (:92). All keyed on counts.
- `append` drops bytes when `measure_only`, or text mode without `retain_binary_with_text`, or membership observer present (:142-147).
- Text scoping: a scalar is allowed only in a `field` scope (`open(kind=="field")`, writer.rs:153) and only one per scope (:165). Labels must satisfy `label()` (framing.rs:6: nonempty, <=64, first lowercase, `[a-z0-9_]`), keyed on source spelling of the static field name (writer.rs:146,186).
- `bytes` text form: kind=="fixed" and len==32 prints `digest <hex>` (writer.rs:219); otherwise printable ASCII with `\"`, `\\`, `\xNN` escapes (:229-240).
- Text `finish` requires depth 0 and, if `expected` is set, position == expected length (writer.rs:77-85).

**Cases by shape.** `maximum markup depth 1024` (framing.rs:3). Tags/scalars must be inside a named field.

**Same job elsewhere.** Text writer escapes/scan are mirrored by recovery text `Tokens::quoted`/`decoded` (recovery/policy/text/tokens.rs:54-104). Label predicate shared with recovery (`label` imported at recovery/policy/text/framing.rs:5).

**Unused / one-caller.** `policy_preallocated`: 1 caller (encode/baseline/recovery.rs:11; grepped `policy_preallocated`). `canonical_text_with_element_count` (encoder/text.rs:20): 1 caller, `canonical_text` in the same file (:16) (grepped; README mentions it). `render` (text.rs:27) is used by recovery/policy/text.rs:50. `encode_canonical_component!` users in this unit: representation.rs:65, selected_providers.rs:26, terminal_permissions.rs:17, values/external_policy.rs:20 (4; others outside unit).

**Stated limits.** None beyond ceilings.

**Fail-open risks.**
- `optional_package_identity` (encoder.rs:241) discards `self.option(...)` with `let _ =`; `encode_carry_policy` (values/providers/carry.rs:5-33) discards all four `field` results with `let _ =`. Traced: errors set sticky state (`exceeded` / text `error` / `membership_error`) and surface at the next `check()` (encoder.rs:248) or `finish` (:114), so a failure there is not lost. Traced.
- `Encoder::finish` (:114) rejects an open text writer; `finish_text` (text.rs:55) rejects a binary-only encoder. No accept-on-unknown path seen.

---------------------------------------------------------------------------

## B. encode/membership.rs and membership/** (rows 8-12)

**Purpose.** Verify that every package identity named inside a baseline's rendered identity strings belongs to the source graph, without admitting anything. It runs the normal baseline encoder in observer mode and parses identity text as it passes.

**Shape.**
- `PackagePolicyBaseline::validate_package_membership` (membership.rs:24): builds `Visitor`, `Encoder::policy_membership`, calls `framed_policy`, returns `Visitor::usage()`.
- trait `Observer` (membership.rs:17): `package`, `type_identity`, `nominal_path`; the only implementer is `Visitor` (visitor.rs:123; grepped `impl Observer` within unit).
- `Visitor<Contains>` (visitor.rs:13): `contains` closure, `limits`, `usage`, `depth`, `failure`, `unreadable` excerpt.
- `PackagePolicyMembershipLimits` default 64 MiB / 1,048,576 nodes / depth 128; `bounded` clamps to 64 MiB / 256 Mi nodes / 128 (membership_accounting.rs:33-45). Errors enum `PackagePolicyMembershipError` (:63), `MalformedIdentityExcerpt` (:119, window 256 bytes, 192 before the stop).
- framing.rs: hand-written inverse parser: `Fields` (:6, `len:value` frames), `visit_type` (:50), `visit_name` (:68), `owner_label` (:126), `topology` (:148), `constraint` (:183).
- tests.rs: 13 unit tests, includes real std identities with const operands (:154-170).

**How it decides.** Accept/reject keyed on the spelling of the identity text:
- `visit_type`: first byte not a digit => handed to `language_semantics` visitor `runtime()` (framing.rs:54-56); digit => must be `signature-type` frame with runtime + topology children (:59-64).
- `visit_name`: first byte not digit => returns Ok without inspection (:73-75); digit => labels `conformance-caller-binder`, `callback-static-parameter`, `boundary-operator-policy`, `conformance-callable` (:78-119); anything else `MalformedIdentity` (:120).
- `owner_label`: `package` => checks `contains` through `self.package(...)`; `toolchain-source` => `Ok(())` with no lookup (:140-145).
- Package check itself: `(visitor.contains)(package)` else `UnknownPackage` (visitor.rs:126-130).
- Resource limits: node count and depth in `enter_node` (visitor.rs:50-60); owned bytes in `reserve` (:153-162); identity longer than 4 MiB => `InvalidPolicy` (framing.rs:51,69).

**Cases by shape.** Closed lists of frame labels (framing.rs:78-120), topology words `named|unit|dynamic-trait|const-expression|elided` (:150-153), `binder:<n>`, `reference|array|slice|generic|constrained` (:161-176), constraints `declared-domain|compiler-domain` (:188-197). Conformance callable must have exactly four nonempty leading fields then parameter/result children (:92-118).

**Same job elsewhere.** `language_semantics::type_identity::visit_type_identity_package_owners` (external, imported visitor.rs:7-10; not read) walks the non-framed grammar; framing.rs duplicates the framed half. `ordinal`/`Fields` digit parsing is local to this file.

**Unused / one-caller.** `validate_package_membership`: 1 external caller (package-manager `lock/validation.rs:43`). `Observer::package` is also called from `Encoder::package_identity` (encoder.rs:235).

**Fail-open risks.**
- framing.rs:73-75, plain names are accepted without scanning. Traced: a non-digit name has no framed owner; owners embedded in such names would only be found by the `runtime` path when passed through `embedded_name` (visitor.rs:169). I did not read the external visitor, so what it does with names that contain owners but are not framed is open.
- framing.rs:143, `toolchain-source` owner labels are never checked against `contains`. No comment at the site; module header (membership.rs:1) says membership is "across retained semantic package owners", so toolchain sources are by design outside the package graph. Inferred.
- visitor.rs:77-78 maps `ResourceLimitExceeded`/`UnknownPackage` from the external visitor to `InvalidPolicy`; this rejects, not accepts.
- membership.rs:40: `result.map_err(|_| InvalidPolicy)` only after membership_error is checked, so a membership error wins over an encoder error (traced).

---------------------------------------------------------------------------

## C. encode/mod.rs, policy_rows.rs, policy_rows/** (rows 13-18)

**Purpose.** Project a baseline into sorted, individually canonical review rows (binary key, binary value, text value) under row, byte, element and depth budgets; also acceptance-only subsets.

**Shape.**
- `PackageReviewEncodingError{message:&'static str}` (mod.rs:24); `type_identity` encoder (:42, observes then writes canonical string); `PackageReviewCallableContract::canonical_set_text` (:50).
- `canonical_rows_with_limits` / `acceptance_rows_with_limits` (policy_rows.rs:17,27), terminal-permission acceptance rows (:39), `restricted_build_request_acceptance_text` (:54), `runtime_permission_rows` (:101).
- `Builder` (builder.rs:14): `new` (:25), `push` (:73), `finish` (:172), `frame_row` (:196).
- assembly.rs: `count` (:16), `acceptance_count` (:60), `project` (:98), `assumed` (:269). components.rs: `terminal` (:17), `terminal_permission` (:57), `representation` (:77). declarations.rs: `project` (:12) over seven public-API collections.

**How it decides.**
- `push` runs key measure pass, key emit pass, then value measure pass and emit pass, and requires measured length == emitted length (builder.rs:109, :155). Rows are sorted by `(kind, key_bytes)`; equal pairs reject (:180-191); count must equal `expected_count` (:175).
- `acceptance_only` builder silently skips rows whose `initial` flag is false (:81-83). `initial` and `audit` booleans are passed per kind at each `push` site (e.g. assembly.rs:147-152, native bindings `true,true` :196-198).
- Row version/header fields: `frame_row` writes magic `OMEGA-PACKAGE-POLICY-ROW\0`, row schema, baseline schema, package, target, kind tag, flags (:205-217). `update_requires_decision` is written equal to `initial` (:212).
- `count` hardcodes `3` for rows not tied to a collection (Header, SelectedProviderAssociation, RepresentationTarget) (assembly.rs:20); `acceptance_count` counts only callables with supply `AdmissionClaim|ExternalRealization` (:68-75).

**Cases by shape.** `project` destructures `PackagePolicyBaseline` with no `..`, so a new field is a compile error (assembly.rs:102-119). Native bindings require `native.producer()` else error "native row has no Build producer" (:202-204).

**Same job elsewhere.** The three-pass measure/emit/check pattern is also in `restricted_build_request_acceptance_text` (policy_rows.rs:72-93), a second copy of `Builder::push`'s value half (builder.rs:116-160). Suspected (not read) more in `encode/baseline/**`.

**Unused / one-caller.** `runtime_permission_rows` is re-exported (encoding/mod.rs:17); `restricted_build_request_acceptance_text`: 1 external caller (package-manager `review/restricted_build_grants.rs:48`). `rejected` (policy_rows.rs:97) used by builder/assembly/components.

**Fail-open risks.** none seen (every size or count disagreement returns an error). `acceptance_only` skipping (builder.rs:81) is by design: callers pass `initial` per row; traced for acceptance_count/assembly parity: if `acceptance_count` and the `initial` flags disagree, `finish` fails (":175" count check), so a mismatch rejects.

---------------------------------------------------------------------------

## D. encode/public_api*, representation, selected_providers*, terminal_permissions (rows 19-28)

**Purpose.** Field-by-field encoders for public API shapes, representation policy, selected-provider policy, terminal permission policy.

**Shape / how it decides.** Exhaustive `match` over record enums with a hardcoded wire tag per variant, no wildcard arms, e.g. type parameter kinds Type 0, Const 1, Machine 2, Proposition 3, Value 4, Implementation 5 (public_api/signatures.rs:80-95); provider bindings tags 1-7 with tag 0 "permanently retired" (selected_providers/bindings.rs:1); terminal authority classes get both a name and `class.canonical_tag()` (terminal_permissions.rs:54-74). Macros `encode_callable_shape!` and `encode_callable_behavior_tail!` (public_api/signatures.rs:18,38) share field lists between traits, operators and machine contracts.

**Same job elsewhere.**
- service method progress premises are encoded twice with different spellings: nominal identities in selected_providers/authority.rs:19-56 and strings in selected_providers/service.rs:101-138; likewise `service_reach` (authority.rs:12 as nominal vs service.rs:88-92 as strings) and `synchronous_invocations` (authority.rs:13-18 vs service.rs:93-97). Both are written by `method` (service.rs:25-26, :88-97).
- `type_parameter`, `formal`, `encode_conformance_bound` used from several places.
- Representation selection fields are shared with calling opaque uses via `encode_opaque_representation_fields!` (representation.rs:21) and `encode_selection_owner` (values/identity.rs:36), documented there as the shared wire shape. Users outside its file: `encode/calling/opaque.rs:7` and `representation.rs:28` (grepped, 2).

**Unused / one-caller.** `encode_selection_owner` 2 users (above). `encode_returned_borrow_origin`/`encode_parameter_qualification` are private to public_api/declarations.rs.

**Fail-open risks.** none seen.

---------------------------------------------------------------------------

## E. encode/values/** (rows 29-50)

**Purpose.** Canonical encoders for contracts, contract expressions, crash/structural expressions, declarations (propositions, const shapes, operator coordinates), effects, external policy, identities, physical calling policy, providers, conformance policy.

**Key facts.**
- `encode_nominal` (identity.rs:10): `Unresolved` owner => `Err` (:24-28); also calls `observe_nominal_path` so the membership observer sees every nominal path (:32).
- `encode_contract_expression` wraps `nested` (expressions.rs:14); body tags 0-27 (expressions/body.rs), call-target tag 3 retired (body.rs:2).
- `encode_callable_contract` rejects `Requires` with a result-case guard (contracts.rs:29-33).
- `encode_target` rejects any target string that is not some `TargetProfile::ALL` identity (external_policy.rs:90-100).
- `encode_physical` rejects clobber lists that are not strictly increasing (physical_calling_policy.rs:18-26).
- `encode_target_context` calls `validate_canonical_structure` first (identity.rs:83).
- `compiler intrinsic` tags: 0 builtin, 1 float neg, 2 conversion, 3 float binary, 4-6 hosted, 7 int compare, 8 accessor scalar, 9, 10, 11 access lifecycle, 12-15, 16 empty run release, 17 static interrupt (providers/intrinsics.rs:13-146).
- Structural expression vocabulary: `primitive_type_row` 0-12 incl. `Selector => (12,"selector")` (crashes/vocabulary.rs:10-28); `integer_binary_row` 0-30 (:48-82).

**Same job elsewhere.**
- Tag tables are written again in recovery (see F), and `ieee_float_comparison_row`, `integer_comparison_row` have recovery-side copies of the table as `tag!` arms (recovery/policy/structural_expressions.rs:81-92, :38-42, tags.rs:53-85).
- `crashes/vocabulary.rs:2` says these rows are shared by "binary encoding, canonical text, and recovery". Grep for `primitive_type_row|integer_binary_row|integer_comparison_row`: users only in encode/values/crashes/{scalar,boolean,vocabulary}.rs; no recovery user. Comment overstates.
- `ieee_float_comparison_row` (boolean.rs:120) comment says structural and scalar arms read one roster: both arms are in the same file (:73,:104). True within the file.
- identity.rs:2 documents `encode_selection_owner` as shared by calling-opaque and representation policy; see D for the 2 users.

**Unused / one-caller (grepped).** `encode_installation_reach` (effects.rs:25): 1 caller, encode/callable_policy/row.rs:142. `encode_boundary_application` (providers/applications.rs:7): 1 caller, encode/baseline/boundary.rs:69. `encode_supply` (identity.rs:57): 1 caller, encode/callable_policy/row.rs:39. `encode_physical`: 1 caller, encode/calling/application.rs:123.

**Fail-open risks.** `encode_carry_policy` discards errors (see A; traced sticky). Otherwise none seen.

**Tests in this group.** contracts.rs:170 (carry fact wire tags), declarations.rs:166 (const meaning), expressions.rs:38 (recast tags, builtin member, unresolved nominal rejects).

---------------------------------------------------------------------------

## F. encoding/mod.rs and encoding/recovery/policy/** (rows 51-100)

**Purpose.** Decode canonical binary policy bytes into typed records with hostile-input accounting, and accept a decode only if re-encoding it reproduces the input bytes. Also recover from named text by converting text to binary and then verifying by re-render.

**Shape.**
- Versions/magics (encoding/mod.rs:22-41): baseline v27, external supply v10, calling v16, representation v14, selected provider v21, terminal permission v13, callable v16.
- `recover_component` (recovery/policy.rs:33): `Reader::new` -> `literal(magic)` -> version -> `decode` -> `finish` -> `validate` -> `canonical_scratch(len)` -> re-encode via encoder -> compare to input (:42-53). `PackagePolicyBaseline::recover_canonical_with_usage` (baseline.rs:25) does the same by hand with `canonical_bytes_for_recovery` (:57-63).
- `recover_canonical_component!` macro (policy.rs:57); users: callable_policy.rs:25, calling_application.rs:17, external.rs:21, representation.rs:19, selected_providers.rs:24, terminal_permissions.rs:19 (6 in this unit).
- `tag!` macro (policy.rs:97): `match reader.byte()? { tag => value, _ => Err(InvalidTag) }`.
- `Reader` (reader.rs:6): remaining slice, `remaining_elements`, `remaining_owned_bytes`, `depth`. `sequence(minimum_encoded_bytes, decode)` (:126), `option`, `nested`, `boxed`, `bytes`, `string`, `allocate`, `elements`.
- `PackagePolicyRecoveryLimits` default 64 MiB bytes / 64 MiB field / 65,536 elements / 64 MiB owned / depth 128 (policy_recovery_accounting.rs:72-81); `bounded` clamps to defaults (:59). Error enum with fixed messages (:84-125).
- Leaf decoders: `identity.rs`, `expressions.rs`, `structural_expressions.rs` (+`scalars.rs`, `tags.rs`), `contracts.rs`, `signatures.rs`, `declarations/*`, `public_api/*`, `callable_policy.rs`(+`behavior.rs`), `calling_application.rs`(+callbacks/opaque/shapes), `conformance.rs`, `physical_calling_policy.rs`(+placement/state), `representation.rs`, `selected_providers.rs`(+authority/bindings/service/signature), `terminal_permissions.rs`, `external.rs`, `intrinsic.rs`, `baseline/{boundary,dependencies,restricted_build}.rs`, `behavior.rs`.
- Text: `PackagePolicyBaseline::recover_text_with_usage` (text.rs:35), `PackagePolicyTextRecoveryLimits` (:14), `framing::binary` (text/framing.rs:9), `Tokens` (text/tokens.rs:6).

**Inputs/outputs.** Input: `&[u8]` or `&str` plus limits. Output: typed record and `PackagePolicyRecoveryUsage`, or a fixed-message error.

**How it decides.**
- Every closed vocabulary is a `tag!` table of explicit numeric arms (e.g. expressions.rs:24-151, intrinsic.rs:15-50, tags.rs:12-25).
- Booleans: byte 0/1 else `InvalidTag` (reader.rs:99-105).
- Sequence size gate: `count > remaining_bytes / minimum_encoded_bytes` => `UnexpectedEnd` (reader.rs:133). The minimum-byte literals are hand-chosen per call site (e.g. 164 conformance.rs:25, 112 public_api/traits.rs:30, 96 expressions.rs:72, 90 calling_application/callbacks.rs:16, 83 external progress routes behavior.rs:105). Only restricted_build.rs:2 explains how its three constants are derived; the others have no derivation comment (read).
- `declaration_abi` recomputes `abi.id()` and requires it to equal the stored u64 (calling_application.rs:88).
- `target` decoders: selected_providers.rs:43 (returns `TargetProfile`) and external.rs:106 (returns `String`) each match against `TargetProfile::ALL` identity strings; `target_context` (identity.rs:68) only rejects an empty target string.
- `disposition` (terminal_permissions.rs:50): classes must be strictly increasing by `TerminalAuthorityClass` order else `NonCanonicalEncoding`.
- `state_set` uses `from_canonical` (state.rs:71) which accepts only strictly sorted input.
- Text recovery trusts nothing by name: `framing::binary` (text/framing.rs:2 comment) rebuilds binary from the scalars, then the binary path verifies, then `render(&policy, Writer::new(max, Some(text)))` must reproduce the exact text (text.rs:50).

**Cases by shape.** `ServiceProgressSubject` only tag 1 `Parameter` (authority.rs:21, service.rs:66); `PackageReviewProgressSubject` tags 0 `Declaration` and 2 `Parameter` (callable_policy/behavior.rs:100-103). `opaque_representation_fields` accepts only origin tag 0 and lifecycle tag 0 (representation.rs:64-69). Coverage family tag 0 only (selected_providers.rs:106).

**ENCODE/RECOVERY DRIFT (read plus grep).**
1. `recovery/policy/intrinsic.rs:15-50` `execution` has arms 0-7, 9, 10, 12-15. The encoder emits tags 8 (`AccessorScalarAccess`), 11 (`AccessLifecycle`), 16 (`EmptyRunRelease`), 17 (`StaticInterrupt`) (encode/values/providers/intrinsics.rs:13,16,68,101). `PackageReviewCompilerIntrinsicExecution` has all four variants (record/package/providers.rs:19-31) and capture builds them (capture/providers/intrinsics.rs:67-108; grepped). A provider row or boundary realization carrying one of those four encodes, then recovery returns `InvalidTag`. Fail-closed (rejects), but a legitimate policy would not round-trip.
2. `recovery/policy/structural_expressions/tags.rs:12-25` (`primitive_type`) and `:32-45` (`primitive_type_name`) stop at `Addr` (11). The encoder vocabulary includes `Selector => (12, "selector")` (encode/values/crashes/vocabulary.rs:26), and capture maps `PrimitiveType::Selector` to it (capture/behavior/crash/expressions.rs:364; grepped). A structural expression with a `Selector` type encodes, then fails to recover. Fail-closed.

**Same job elsewhere.**
- Whole decode tables mirror the encoders in `encode/values/**`, `encode/public_api/**`, `encode/selected_providers/**` (the pair must stay manually in sync; the round-trip compare at policy.rs:51 is the only mechanical check).
- `primitive_type` (tags.rs:8) vs `primitive_type_name` (:28): two decoders of the same 12-value vocabulary, one by tag byte, one by string.
- `integer literal landing arithmetic_domain` decoded from strings "Exact|Wrapping|Saturating|Trapping" (scalars.rs:40-46) vs `arithmetic_domain` tag decoders (expressions.rs:52-57, intrinsic.rs:112-120) of the same enum.
- Named-text `framing.rs:148-155` accepts `u128`/`i128` tokens that the text `Writer` never emits (writer.rs:173 only formats via `number(kind, value)` with kinds u8,u16,u32,u64,i64). Any text using them would later fail the exact re-render (inferred).

**Unused / one-caller (grepped).**
- `PackagePolicyBaseline::recover_text_with_usage` (text.rs:35), `PackagePolicyTextRecoveryLimits` (:14), `framing::binary` (text/framing.rs:9), `Tokens` and `decoded`/`nibble` (text/tokens.rs): grep for `recover_text_with_usage`, `PackagePolicyTextRecoveryLimits`, `PackagePolicyBaseline::recover`, `framing::binary` over `omega-rust` finds no caller of the baseline text recovery outside its own file (only README.md:250,294 mention it; the other `recover_text_with_usage` hits are on package-manager types `CanonicalSourceClosureSubject`, `HistoricalPackagePolicyDecisions`, a different method). The module is `pub(in crate::encoding::recovery) mod text` (policy.rs:25) and `encoding/mod.rs:19` re-exports only `PackagePolicyRecoveryLimits`, so `PackagePolicyTextRecoveryLimits` cannot be named outside the crate. 0 call sites for ~353 lines (text.rs 58 + framing.rs 183 + tokens.rs 112).
- `PackagePolicyBaseline::recover_canonical`: callers package-manager `review/candidate/review_cache.rs:108` and compiler tests (4 in compilation/compiler/tests/package_review.rs).
- recovery/mod.rs:1-8 says recovery owns "both directions of the public recovered-row envelope"; the only files under recovery/ in the unit are `mod.rs` and `policy/**` and mod.rs declares only `mod policy` (mod.rs:10). No row recovery exists in this unit.

**Stated limits.** "Closed-vocabulary tag retired without reuse" (policy_recovery_accounting.rs:94); external.rs:79 and selected_providers/bindings.rs:25 reject tag 0 as `RetiredVocabulary`; body/expressions comments "Call-target tag `3` is retired" (expressions.rs:2).

**Fail-open risks.**
- None seen: every unmatched tag is `InvalidTag` and the final byte compare rejects non-canonical input.
- Note on `target_context` (identity.rs:71-80): accepts any nonempty string; stated at record/target_context.rs:5-6 ("Uncatalogued tags remain valid inert disclosure for pure bodies").
- `recover_component` validate step is `|_| Ok(())` for macro form without `validate` (policy.rs:89); only `external.rs:21` uses that form in this unit, and its record validation is in baseline validation `validate_external_associations` (record/baseline/external.rs:23).

**Tests.** codec tests: contracts.rs:139, declarations/values.rs:63, expressions.rs:265, external.rs:151.

---------------------------------------------------------------------------

## G. lib.rs and permission_acceptance.rs (rows 101-102)

- lib.rs:1-39: module list (`capture`, `encoding`, `permission_acceptance`, `record`) and re-exports. `#![forbid(unsafe_code)]` (:26).
- permission_acceptance.rs: `RuntimePermissionInput` (:22) bundles prepared policy, selected execution, provider settlement, source custody; `TerminalPermissionAcceptance` (:43) `{schema_version, package, target, rows: Vec<([u8;32], String)>}`.
  - `from_public_build` (:51): requires package and target (via `or_evidence`), requires `evaluated != 0 && evaluated == installed` assumed bindings (:62), projects rows via `declarations.rows(...)`, keeps only `AssumedBindingContract` and `TerminalPermission` kinds (:71-74), requires the count of assumed rows == evaluated (:81), sorts by digest of key bytes (:86).
  - `validate` (:95): schema version must equal `PACKAGE_POLICY_ROW_VERSION` (:96); recomputes the same row set and requires exact equality with the accepted rows (:117).
- Same job twice: the filter-map-sort of `(content_identity(key_bytes), canonical_text)` appears in `from_public_build` (:70-86) and `validate` (:102-116).
- Fail-open: none seen (any mismatch returns an error).

---------------------------------------------------------------------------

## H. record/** (rows 103-194)

**Purpose.** The inert vocabulary: `PackagePolicyBaseline` and its component records, plus structural validators (`validate_canonical_structure`) that check ordering, ownership, telescopes and cross-references without compiler state.

**Shape.**
- `PackagePolicyBaseline` (record/baseline.rs:33): 16 fields (package, target, public_api, callables, selected_providers, terminal_permissions, representation, native_bindings, external_supplies, dangerous_capabilities, slack_uses, semantic_dependencies, boundary_applications, assumed_bindings, service_audits, restricted_build_requests).
- `validate_canonical_structure` (baseline/validation.rs:23) aggregates component validators and cross-component checks.
- Row kinds: `PackagePolicyRowKind` 26 variants with a single `tag_and_name` table (policy_rows.rs:48-77); `PACKAGE_POLICY_ROW_VERSION = 9` (:15); limits default 65,536 rows / 128 MiB / 1,048,576 elements / depth 128 / 1 MiB key / 4 MiB canonical / 32 MiB text (policy_rows/limits.rs:14-26).
- Contracts vocabulary: `PackageReviewContractExpression` (contracts/expressions.rs:215), `PackageReviewContractFact` (callable_contracts.rs:17), propositions (propositions.rs), operator coordinate matching (expressions/operator_policy.rs).
- Calling: `PackagePolicyCallingPlan` (calling/application.rs:20) with validation in `calling/application/validation{,/callbacks,/shapes,/target}.rs`; physical contract (calling/physical.rs:16).
- Package-level: callables (`package/callable_policy*`), providers (`package/provider_policy*`), boundary policy, terminal permissions, `package/providers.rs` intrinsic executions and boundary application types.
- Representation, restricted build, signatures, public policy shapes, target context, semantic dependency, source locations.
- `accessors.rs`: macros `opaque_representation_selection_accessors!`, `package_policy_callable_signature_accessors!`, `signature_parameter_accessors!`, `field_projection_accessors!` (used throughout).

**How it decides (selected judgments, all keyed on identity / ordinal / structure, not names).**
- Ordering: `ordered`/`ordered_by` require strictly increasing order (`compare(..).is_ge()` => Err) (public_policy/validation.rs:88-104).
- `validate_resolved`: path nonempty and owner not `Unresolved` (identity.rs:55-61).
- `owned_pair`: both resolved and same owner (callable_policy/validation/signature.rs:188-199).
- Component package/target agreement (baseline/validation.rs:24-34).
- Callable roles vs supply: `PrivateAssumption` => `AdmissionClaim`, `PrivateExternal` => `ExternalRealization`; boundary role requires boundary supply (`Boundary|TopLevelRequirement|AdmissionClaim`), `Public` forbids it (callable_policy/validation.rs:33-53). `Build` role is exempt from the boundary/public supply check (:49-50) and at most one `Build` allowed (:18-26).
- External supplies: unique callable coordinate, signature/requirement must equal the callable's retained surface, callable supply must be `ExternalRealization` with role Public or PrivateExternal (baseline/external.rs:14-86); target in binding must equal expected target (baseline/validation.rs:70-85).
- Native bindings: `validate_native` accepts only `Import` with target==selected and valid locator, or `Syscall` with `evaluated: Some` on `LinuxArm64|LinuxX64` and a number fitting u32 (provider_policy/binding_validation.rs:10-31). Locator validity keyed on the target: PE only on `WindowsX64`, ELF only on `LinuxArm64|LinuxX64`, Mach-O only on `MacosArm64|MacosX64` (:131-155).
- Provider plan coverage: methods.len == rows.len, each method exactly one row with matching spelling (provider_policy/validation.rs:36,46-57); family coordinates must point at a plan whose declaration equals the family identity (:149-171).
- Calling plan: native/semantic/physical telescope sizes agree, shape DAG acyclic with fixed-array/record geometry (calling/.../shapes.rs:39-90, scratch `[u8;256]` :24), callback binders/demands/materializations one-to-one with layouts, composed placements recomputed via `layout_plans` and compared (callbacks.rs:166-179), opaque uses each tied to a unique nominal owner and shape path (shapes.rs:92-192).
- Representation target: only `CrossPlatformCli|LocalUnchecked` get a pointer-geometry-only check; other profiles must match `backend_profile().native_target()` (representation/target/validation.rs:13-43).
- Operator coordinate matching: `matches_policy_requirement` renders two candidate spellings and compares by prefix writer: `boundary-operator-policy` framed form or `named-callable(path(...),parameters(...),result-dispatch(...))` form (operator_policy.rs:27-83). `ToolchainSource` owner renders as `toolchain-owner` with no digest (:65), so any toolchain source digest matches (test at :172-179).
- Callable ordering for flows: `kind_tag` re-spells `CapabilityFlowKind` as 0-4 (behavior_policy.rs:29-37), a third copy of that table besides the encoder/recovery (encode/ and recovery/policy/callable_policy/behavior.rs:27-33).

**Cases by shape.**
- Hard-coded capacity 256 for calling shape graph, parameters, callback catalog, opaque catalog (shapes.rs:21,99; validation.rs:30).
- Representation producer surface: exactly 2 interface arguments, no lifetime args, no requirements (representation/policy/validation.rs:46-50); selections: exactly 2 trait args, no rows (:67-72).
- `provider_policy/validation.rs:29-35`: nested boolean exception allowing a plan with empty `provider_type` and no `provider_type_declaration` when `realizing_package.is_none()` or all rows have `native_build`; no comment at the site.
- Linux-only evaluated syscall (binding_validation.rs:24,60).

**Same job elsewhere (read).**
- Crash route + progress premise validation exists twice: callable surface (package/callable_policy/validation/behavior.rs:95-133,161-190) and public policy (public_policy/validation/behavior.rs:36-54,58-87). Same rules: guards nonempty, no `Truth` with siblings, subjects in telescope, `owned_pair` on routes.
- Scope/binder telescope logic: `Scope` in callable_policy/validation/signature.rs:29-94, built by `scope()` (public_policy/validation.rs:106).
- Contract expression validation (signature_expressions.rs) and contract fact validation (signature_contracts.rs) are used by public policy via `shared::*` imports (public_policy/validation.rs:14-17).
- Role enums: `PackageReviewCallableRole {Boundary,Public,Build}` (contracts/expressions.rs:27) vs `PackagePolicyCallableRole` with two more variants (package/callable_policy.rs:45); mapped in capture/callables/policy.rs:97-102 (grepped).
- Parallel type families `PackageReview*` and `PackagePolicy*` exist side by side (e.g. `PackageReviewTraitShape` record/signatures/traits.rs:96 vs `PackagePolicyTraitShape` public_policy/declarations.rs:15; `PackageReviewOperatorShape` contracts/declarations.rs:31 vs `PackagePolicyOperatorShape`; `PackageReviewCrashRoute/Termination/ProgressPremise` authority.rs:100-126 vs `PackagePolicyCrashRoute/Termination/ProgressPremise` behavior_policy.rs). Grepped the `PackageReview*` family (TraitShape, ConformanceShape, OperatorShape, DataShape, DomainShape, CrashRoute, Termination, MachineParameter*, TypeParameter*, SourceLocation*, ...): 186 occurrences across 30 files, the users are `capture/**` plus record; not dead.
- Several empty `impl X {}` blocks remain (authority.rs:111; authority_expressions.rs:41,60,68,76; calling/application.rs:95,97; calling/callbacks.rs:84-89; conformances/policy.rs:49; data.rs:22,50; propositions.rs:63,93,103,113,153,161; signatures/*.rs:11,28,37,46; provider_policy/authority.rs:45; rows via `PackagePolicyProviderEvaluatedSyscall {}` rows.rs:56; behavior_policy.rs:97; public_policy/signatures.rs:59). Read.

**Unused / one-caller (grepped).**
- `PackagePolicyBaseline::apply_service_audit_policy` (baseline.rs:55): 2 callers (capture/package_policy.rs:110, package-manager review/candidate/passes/occurrences.rs:828).
- `validate_boundary_application_owners` (baseline/boundary_owners.rs:15): 1 caller (package-manager lock/validation.rs:55).
- `PackageReviewOperatorCoordinate::policy_requirement_identity` (operator_policy.rs:18): callers capture/semantics/declarations/policy_requirements.rs:26 and the test at :257 (2).
- `PackageReviewEvaluatedBindingUsage` alias (signatures/external_supply.rs:71): all 3 occurrences are in its own file.
- Per-row accessor macros: many getters (e.g. `PackagePolicyRow::update_requires_decision` policy_rows.rs:143) mirror fields; not counted.

**Stated limits / comments.**
- "`UnscopedFilesystemExecution` remains in the enum so review surfaces it if an evaluator ever carries it" (restricted_build.rs:9-11).
- "version 8 rows must be re-reviewed rather than decoded" (policy_rows.rs:6-7).
- Comment on `PackagePolicyRow` kinds: `tag_and_name` "so wire and text representations cannot drift" (policy_rows.rs:8-9); encoder uses `as_str()` + `canonical_tag()` from that table (encode/policy_rows/builder.rs:210).
- target_context.rs:5-6 as above.
- "Foreign operator declarations belong to their own package's baseline" (baseline/validation.rs:4-6); boundary_owners.rs:4-5 "Toolchain declarations have no managed-package baseline to look up", and `ToolchainSource` demand owners are skipped with `continue` (boundary_owners.rs:30).

**Fail-open risks.**
- record/public_policy/validation/declarations.rs:199-205 and :235-246: a trait application or evidence requirement whose declaring trait is not in `api.traits` is not checked against a declaration (the checks are inside `if let Some(declaration) = api.traits.iter().find(..)`). Traced: foreign-package traits are therefore accepted unverified; no comment at the site. Reachable by any baseline naming a foreign trait.
- record/package/provider_policy/binding_validation.rs:180: `(ToolchainSource(_), None) => true` accepts any toolchain digest when no package is expected. Open (no comment).
- record/baseline/boundary_owners.rs:30: `ToolchainSource` owners skip demand validation (`continue`). Stated at the file header (:4-5).
- record/baseline/validation.rs does not inspect `assumed_bindings` (strings) at all (no ordering/duplicate/emptiness check) and only checks `restricted_build_requests` target (`:127-131`); order/uniqueness of restricted build requests is not enforced here. Read; header says assumed bindings are retained "in issue order" (baseline.rs:5-7).
- record/package/callable_policy/validation/signature_expressions.rs:125-148: `Call` with targets `BuiltinFunction`, `ByteSequencePredicate`, `ProofView` gets no target check; evidence arguments check only owners (:144-147). Open.
- record/package/provider_policy/validation.rs:30: an empty `plan.target` string is accepted (`!plan.target.is_empty() && ...`). Open.
- structural.rs scalar `Float32Literal`/`Float64Literal`/`IntegerLiteral.landing` are not validated beyond nonempty text (structural.rs:106-112). Open.

**Tests.** `public_policy/validation/tests.rs` (2 tests: binders start at declared statics), `validation.rs:123` (const dependency), `operator_policy.rs:122` (4 tests).

---------------------------------------------------------------------------

## I. source-acquisition custody (rows 195-203)

**Purpose.** Per-user cache custody for Git and local-snapshot caches: advisory lock files tied to a retained parent directory, host ownership/mode/ACL checks, private staging and atomic publish by rename.

**Shape.**
- lock.rs: `CacheLockBudget` trait (:30), `CacheEntryLock` (:35) with `acquire_from_parent` (:126), `acquire_local_from_parent` (:210), `verify_path_identity` (:265), `open_verified_cache_lock` (:276), `verify_cache_lock_path_identity` (:315), `verify_retained_cache_parent_path` (:349), `same_std_and_capability_file_identity` (:381, per OS cfg). `Drop` unlocks without unlinking (:422-426).
- platform.rs: `same_capability_file_identity` (:43), owner/mode verifiers, Windows identity re-open functions, macOS extended-ACL functions, `non_windows_verify_stub!` (:28).
- publication.rs: `publish_cache_directory_from_open_parent` (:104), `PendingCacheEntry` (:185), `ProvisionalCacheDirectory` (:319), `create_private_cache_directory` (:355), `retain_private_cache_directory` (:373), `direct_cache_child_name` (:62), `retained_cache_directory_exists` (:88).
- mod.rs: module declarations; tests/ in `custody/tests/`.

**How it decides (keyed on filesystem identity, not names).**
- Lock: opened handle's metadata and the parent's `symlink_metadata(lock_name)` must be regular files with same device/inode (Windows: volume serial + file index) (lock.rs:289-301, :381-405); then owner==effective uid and mode bits `0o022` clear (platform.rs:60-74); then macOS extended ACL allow entries rejected (:283-304).
- Publish: stage must be a concrete directory, optionally identity-equal to the retained stage; destination must not exist; after `rename` the published entry must have the stage's identity (publication.rs:115-159).
- Private directory: mode forced to `0o700` and checked (publication.rs:391-420).
- Lock wait: loop of `try_lock`, with a budget trait for Git and a local `Duration` timeout for snapshots (lock.rs:139-151, :224-240); polls with `PROCESS_POLL_INTERVAL`.

**Cases by shape.** Uses `cfg` splits per OS: unix owner/mode check; non-unix returns `Ok(())` (platform.rs:78-85,113-120). ACL: macOS only; stubs `Ok(())` elsewhere (platform.rs:274-280,315-322,385-401,431-440). Windows file/link identity re-open functions; non-windows stubs `Ok(())` via macro (:216,246). `verify_macos_cache_ancestor_acl_custody` has a unix-non-macos stub (:362-370) and no stub for windows (callers must be cfg-guarded; not traced).

**Same job elsewhere (read).**
- `verify_capability_cache_node_owner_and_mode` (platform.rs:53, capability `Metadata`) and `verify_cache_node_owner_and_mode` (:88, `std::fs::Metadata`) have identical bodies (uid == effective uid; non-symlink mode `& 0o022` == 0), each with an `Ok(())` non-unix twin. Callers: `verify_cache_node_owner_and_mode` tree.rs:145,201 (2); `verify_capability_cache_node_owner_and_mode` tree.rs:361,374, publication.rs:408, lock.rs:302,340, git/objects/batch/custody.rs:180, git/cache/creation.rs:143, git/cache/repository.rs:261 (grepped, 8).
- The `try_lock`/sleep acquisition loop is written four times: `acquire_within` (lock.rs:94, cfg(test, unix)), `acquire_from_parent` (:126), `acquire_local` (:167, cfg(test)), `acquire_local_from_parent` (:210). Production callers: `acquire_local_from_parent` local/snapshot.rs:163, local/recovery.rs:200, git/snapshot.rs:89 (3); `acquire_from_parent` git/resolution/acquisition.rs:141 (1); `acquire_local` local/snapshot.rs:66 (1, cfg not test-gated at the caller; the definition is `#[cfg(test)]` at lock.rs:166, so that call site must itself be test-only or cfg'd; not traced). Test-only: `open_retained`, `open_git`, `acquire`, `acquire_within` (grepped: used only in custody/tests/locks.rs).
- Windows/macOS "open by name without following links, compare identity to classification" appears three times: `verify_windows_open_cache_regular_file_identity` (:189), `verify_windows_open_cache_link_identity` (:219), `verify_macos_open_cache_regular_file_acl_custody` (:404).

**Unused / one-caller.** `PendingCacheEntry::create` takes `_cache_identity: &str` that it ignores (publication.rs:197; comment :9-11 says stage names omit the identity). `ProvisionalCacheDirectory` fields are `pub(crate)`.

**Stated limits.** "Non-Unix targets enforce portable kind, handle identity, and bounded topology checks without promoting host ACLs into resolver isolation evidence" (platform.rs:6-8).

**Fail-open risks.**
- platform.rs:78-85 and :113-120: on non-Unix, owner/mode checks return `Ok(())`; stated at the file header (:5-9). On non-macOS ACL checks return `Ok(())`; header says macOS applies ACL policy (:8-12).
- lock.rs:401-405: on a target that is neither unix nor windows, `same_std_and_capability_file_identity` returns `false` (rejects).
- lock.rs:113 and :153: `let _ = file.unlock()` on budget failure; the file is also dropped on return; no accept path.

**Tests (rows 199-203).** `tests/limits.rs` (byte ceiling, no symlink leaf read), `tests/locks.rs` (replaced locked path, replaced parent path, 8x32 contended cold lock creation, local deadline, Git lock budget, error precedence), `tests/macos_acl.rs` (macOS only, 7 tests), `tests/mod.rs` (declares `limits, locks, macos_acl, publication, repository_integrity, snapshots, traversal, windows_ancestry, windows_siblings`; the last five files are not in this unit), `tests/publication.rs` (2 tests).

---------------------------------------------------------------------------

## Coverage

All 203 rows of unit_036.tsv were read in full, in order, in whole-file reads.
- Read in full: 21,927 lines (sum of the line-count column; I added the 203 values by hand in blocks of ten).
- Skimmed: 0 lines.
- Not read: 0 lines.

Greps that fed counts above (all run across `omega-rust` with the Grep tool, uncapped except where noted): `policy_preallocated|encode_installation_reach|encode_boundary_application|encode_supply|encode_selection_owner|encode_physical|primitive_type_row|integer_binary_row|integer_comparison_row|ieee_float_comparison_row`; `AccessorScalarAccess|AccessLifecycle|EmptyRunRelease|StaticInterrupt` (review-evidence); `PackageReview(TraitShape|...)` count mode (186 over 30 files); `verify_cache_node_owner_and_mode|verify_capability_cache_node_owner_and_mode`; `acquire_from_parent|acquire_local_from_parent|acquire_within|open_retained\(|acquire_local\(|CacheEntryLock::acquire\(`; `validate_boundary_application_owners|apply_service_audit_policy|restricted_build_request_acceptance_text|policy_requirement_identity|canonical_text_with_element_count|recover_text_with_usage|validate_package_membership`; `PackagePolicyTextRecoveryLimits|recover_canonical_with_usage|canonical_bytes_for_recovery|recover_text\b|PackagePolicyBaseline::recover`; `PrimitiveType::Selector` (head-limited to 20 results; the review-evidence hits were confirmed by a separate grep of `capture/`).

## Five things a maintainer most needs to know about this unit

1. Recovery accepts a decode only if re-encoding it reproduces the input bytes (recovery/policy.rs:51, baseline.rs:57), and every numeric tag table is written by hand in both `encode/**` and `recovery/policy/**`. Two tables have drifted: compiler intrinsic tags 8, 11, 16, 17 (encoder emits, recovery has no arm) and primitive type `Selector` tag 12 (same).
2. `Encoder` is one struct acting as binary writer, text writer, row measurer and membership observer, selected by which optional fields are set (encoder.rs:11-24). Errors are sticky and are surfaced at `check()`/`finish`, which is why `let _ =` discards at encoder.rs:242 and values/providers/carry.rs:6-33 do not lose failures.
3. Membership checking works by re-parsing rendered identity text with a hand-written grammar (membership/framing.rs): toolchain-source owners and non-digit "plain" names are never looked up, and the closed list of frame labels is the only coverage.
4. The named-text recovery path (`recover_text_with_usage`, `PackagePolicyTextRecoveryLimits`, text/framing.rs, text/tokens.rs, ~353 lines) has no caller anywhere in `omega-rust` (grepped) and its limits type cannot be named outside the crate.
5. Validation is duplicated in two parallel trees (`record/package/callable_policy/validation/**` and `record/public_policy/validation/**`) for crash routes, progress premises and scope; the `PackageReview*` and `PackagePolicy*` type families coexist; and the custody code repeats the lock-acquire loop four times and an identical owner/mode check twice.

## What I am unsure about

- Whether `language_semantics::type_identity::visit_type_identity_package_owners` (external, not read) closes the "plain name" gap in membership/framing.rs:73.
- Whether any non-Rust or generated caller uses `PackagePolicyBaseline::recover_text_with_usage` (I grepped `*.rs` and saw README mentions only).
- Whether the hand-written `minimum_encoded_bytes` literals in recovery (e.g. 164, 112, 96, 90) are correct lower bounds; no derivation exists to check against, and I did not compute them from the encoders.
- Whether `CacheEntryLock::acquire_local` (defined only under `cfg(test)`, lock.rs:166) is legitimately called from `source-acquisition/src/local/snapshot.rs:66`; I did not read that caller's cfg.
- `PackagePolicyMachineStateSet::new` caller count: the combined grep was head-limited and I did not obtain a clean count.
- The `PackageReview*` types used by `capture/**` were counted but their capture usage was not read.
