# slice_0134

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1029 | codec/semantic_module.rs:323 | REACHABLE | Dangling `requirement_identity` in installation reach can pass `decode_module` because canonical order maps it to `""` and foundation never resolves that index (read). |
| A1030 | codec/semantic_module.rs:74-80 | REACHABLE | `validate_canonical_order` treats `boundary: None` as inbound when `entry_admitted` is false; foundation does not contradict that for ordinary callbacks (read). |
| A1031 | codec/semantic_module.rs:4072-4076 | REACHABLE | `encode_execution_module` stops after canonical order; a module that fails only `validate_structural_foundation` still encodes (read). |
| A1032 | codec/term_wire.rs:1543-1548 | CLOSED | `ProofTerm::Formal` / `InvocationFormal` have no extra wire checks because `ProofTerm::validate` is a no-op for those variants (read). |
| A1033 | codec/structural_wire.rs:1901-1905 | CLOSED | Tag 12 may decode loosely, but `declaration_foundations` rejects bad scalar restrictions and layout ids before a portable module is accepted (read). |
| A1034 | sem/calls.rs:2127-2137 | REACHABLE | Early `Ok(())` skips the orphan-runtime-call guard at `sem/calls.rs:2340-2347` when there are no Requires lanes and no nonempty evidence arguments (read). |
| A1035 | sem/scalars.rs:3159-3161 | REACHABLE | `scalar_field_requirement` ignores `AtomicBoundedInteger`, so a plain same-carrier scalar is reported as entailing an atomic bounded field (read). |
| A1036 | sem/references.rs:3942-4024 | REACHABLE | `Field`, `Case`, `AllElements`, `FixedRange`, and `RepresentationView` segments do not run access-ceiling checks; the function can end `true` (read). |

## A1029

**Cited:** `omega-rust/psi/compilation/representations/terminal-psi/codec/src/semantic_module.rs:313-328`, `:424-436`, `:516-536` (digest cited `codec/semantic_module.rs:323`; line numbers match at read).

**Function (read):** `canonical_order::validate_canonical_order` (`semantic_module.rs:53` onward). At `:318-326`, `:429-434`, and `:517-535`, ordering keys call `module.identities.text(...).unwrap_or_default()` so an out-of-range `TerminalIdentityIndex` becomes `""` instead of an error.

**Callers (read):** `rg validate_canonical_order` under `omega-rust/` → 7 matches in 2 files: `semantic_module.rs` (definition plus `encode_execution_module` `:4074`, `decode_module` `:4108`, re-export `:3925`) and `module_wire.rs` tests (`:446`, `:449`, `:455`).

**Upward trace (read):** `decode_module` (`semantic_module.rs:4093-4119`) runs `validate_canonical_order` then `validate_structural_foundation`. For `closed_conformance_applications.declaration_identity`, `terminal_semantics::closed_conformances::validate_closed_conformances` (`sem/calls.rs:1078-1083`) calls `identity()` (`sem/calls.rs:965-974`), which errors on invalid/empty text — so a dangling index there is rejected on decode, not accepted. For `provider_candidates`, `declaration_foundations::validate_provider_candidate` (`foundation_validation.rs:1868-1909`) requires nonempty resolved texts for provider identities — closed on decode for those sites. For `root_service_reach.installation_dependencies.requirement_identity`, `declaration_foundations` (`foundation_validation.rs:1760-1765`) only checks `requirement_identity.is_absent()`, not `identities.text(dependency.requirement_identity)` (read).

**Input that reaches the branch (read):** A portable module whose `installation_dependencies` row carries a non-absent but out-of-range `requirement_identity`; canonical order maps it to `""` (`semantic_module.rs:429-434`), foundation does not reject the index, and `decode_module` can return `Ok(module)`.

**Verdict: REACHABLE** — smallest program, in plain words: a terminal artifact listing an installation reach dependency that names a requirement identity index not present in the identity table, with no Requires-style conformance rows that would force `identity()` checks on that same dangling index.

## A1030

**Cited:** `semantic_module.rs:74-80` (matches digest `codec/semantic_module.rs:74-80`).

**Function (read):** Same `validate_canonical_order` callback loop (`:68-95`). `exact_inbound` is `!application.entry_admitted` when `application.boundary` is `None` (`:74-80`).

**Callers:** Same search as A1029 (7 hits, 2 files).

**Upward trace (read):** `decode_module` always runs this loop. `validate_callback_registrars` (`foundation_validation.rs:10505-10528`) only checks that each `registrar` is a `BoundaryCall` operation — it does not require `boundary: Some(...)` or `entry_admitted: true` on stored applications. Interrupt installations require `entry_admitted` (`foundation_validation.rs:2214-2216`) but that does not apply to `module.callback_applications` alone. `module_wire.rs` test `callback_application_wire_preserves_identity_and_rejects_truncation` (`:435-456`, read) expects `validate_canonical_order` to accept `boundary: None` with `entry_admitted: false`.

**Input (read):** A module pushed to `callback_applications` with nonempty requirement/contracts, a registrar operation that is a boundary call, `entry_admitted: false`, and `boundary: None`.

**Verdict: REACHABLE** — smallest program: a stored callback application marked not entry-admitted and with no inbound boundary link, while still satisfying registrar and contract nonempty checks.

## A1031

**Cited:** `semantic_module.rs:4072-4076` (digest `codec/semantic_module.rs:4072-4076`).

**Functions (read):** `encode_module` (`:4063-4069`) rejects only invocation-scoped identities, then calls `encode_execution_module`. `encode_execution_module` (`:4072-4075`) runs `scalar_qualification_wire::validate` and `validate_canonical_order`, then `encode_raw` — not `validate_structural_foundation`. `decode_module` (`:4107-4109`) runs foundation validation.

**Callers (read):** `rg encode_module` under `omega-rust/` → many production and test uses (codec `lib.rs:167` export; pipeline/tests call `encode_module` after building modules). `encode_execution_module` is `pub(crate)` and used from `encode_module` only (read at `:4069`).

**Input (read):** An in-memory `TerminalModule` that satisfies scalar qualifications and canonical order but would fail `validate_structural_foundation` (for example the orphan runtime evidence pattern in A1034 if embedded only in tables foundation checks later).

**Verdict: REACHABLE** — smallest program: a terminal module table snapshot that is internally inconsistent on foundation rules yet still serializes through `encode_module` because encode never runs the foundation validator.

## A1032

**Cited:** `term_wire.rs:1543-1548` (path `omega-rust/psi/compilation/representations/terminal-psi/codec/src/term_wire.rs`).

**Function (read):** `decode_proof_term` match arms return `ProofTerm::Formal` / `InvocationFormal` without calling `validate()` (`:1543-1548`), unlike neighboring arms (`:1529`, `:1540`, `:1551`).

**Closing check (read):** `ProofTerm::validate` in `omega-rust/psi/foundation/semantic-vocabulary/src/proposition.rs:1387-1452` — arms `ProofTerm::Formal { .. } | ProofTerm::InvocationFormal { .. } => {}` (`:1452`) perform no checks.

**Upward trace (read):** Omission of `validate()` on decode does not skip any enforced predicate for these variants because the validator would succeed trivially anyway.

**Verdict: CLOSED** — no accepting branch becomes permissive relative to the only defined validation hook for those variants.

## A1033

**Cited:** `structural_wire.rs:1901-1905`, `:1942-1945`; cross-refs `term_wire.rs:754-760`, `:893` (digest paths; all read at cited lines in `terminal-psi/codec/src/`).

**Wire decode (read):** Tag `12` sets `scalar_restriction` from any `decode_structural_field_type` result and reuses `PrimitiveScalar(carrier)` (`structural_wire.rs:1901-1905`). Tag `13` reads `LayoutApplicationId(reader.u32()?)` without an immediate bounds test (`:1942-1945`). Mathematical term tag `1` reads `Term::Variable(reader.u32()?)` (`term_wire.rs:893`); child indices are bounded at `:885-888`.

**Foundation on portable accept (read):** `validate_structural_foundation` → structural declaration validation (`foundation_validation.rs:8084-8092`) requires `scalar_field_requires_evidence(restriction)` and matching carrier — rejects restrictions that are merely plain scalars or `AtomicBoundedInteger` without evidence shape. Target layout records later require a matching `target_layout_applications` row when geometry walks the type (`foundation_validation.rs:5903-5910`). Identity formation certificates go through `verify_mathematical_certificate` (`identity_formations.rs:44-45`) after wire decode.

**Verdict: CLOSED** for portable acceptance via `decode_module` — permissive wire shaping is closed by foundation and kernel checks before `Ok(module)`.

## A1034

**Cited:** `sem/calls.rs:2127-2137` and guard at `:2340-2347`.

**Function (read):** `validate_runtime_evidence_inputs_in` (`:2123-2349`). Preconditions: if no `EvidenceContractLaneKind::Requires` lane and no `proof_output_call` with both `runtime_call.is_some()` and nonempty `evidence_arguments`, return `Ok(())` (`:2127-2136`). The later check for ordinary runtime calls not tied to operations (`:2340-2347`) is unreachable when the early return fires.

**Callers (read):** `rg validate_runtime_evidence_inputs` in `omega-rust/` → 7 matches: `sem/calls.rs`, `sem/lib.rs` re-export, `foundation_validation.rs:10479`, `machine_lowering.rs:1801`, `unit_validation.rs` (2). Portable receive path: `foundation_validation.rs:10479` inside `validate_structural_foundation`.

**Input (read):** Module with no Requires evidence lanes, a `ProofOutputCall` with `runtime_call: Some(...)`, empty `evidence_arguments`, `static_requirement_dispatch: None`, and no matching machine operation — passes the early exit and never hits `:2340-2347`.

**Verdict: REACHABLE** — smallest program: a proof-output call that claims a runtime call site but carries no evidence arguments, in a module with no Requires contract lanes.

## A1035

**Cited:** `sem/scalars.rs:3159-3161`.

**Function (read):** `scalar_field_entails` (`:3146-3180`). After carrier match, `scalar_field_requirement(required, ...)` (`:3159`) returns `None` for types other than `RefinedScalar` and `BoundedInteger` (`scalar_field_requirement` at `:3052-3062`); `AtomicBoundedInteger` is handled only in `atomic_resident_requirement` (`:3065-3072`), which `scalar_field_entails` does not call on the required side.

**Callers (read):** `rg scalar_field_entails` in `omega-rust/` → 8 matches: definition `scalars.rs:3146`; uses `structural.rs:114`, `descriptors.rs` (`:1154`, `:1286-1287`, `:1336`), `lib.rs` re-export, `emission.rs:279`, `:4108` (`receiving_entails`).

**Input (read):** Established field is plain `StructuralFieldType::Scalar` with carrier C; required field is `AtomicBoundedInteger` with the same integer carrier; `scalar_field_entails` returns `true` at `:3159-3160` without range proof.

**Verdict: REACHABLE** — smallest program: lowering or descriptor logic asks whether a non-atomic scalar field entails an atomic bounded integer field of the same carrier; the entailment query answers yes.

## A1036

**Cited:** `sem/references.rs:3942-4024` (and `:4026` is the following function header; read through `:4024`).

**Function (read):** `reference_source_projection_accessible` (`:3942-4024`). Loop handles `Referent`, `FixedIndex`, `RuntimeIndex` with partial checks; `_ => {}` at `:4020` covers `Field`, `Case`, `AllElements`, `FixedRange`, `RepresentationView` (`StructuralPathSegment` enum at `terminal-psi/src/types_and_values.rs:1539-1557`, read). Function returns `true` at `:4023` if authority admission at `:3959-3960` succeeded.

**Callers (read):** `rg reference_source_projection_accessible` in `omega-rust/` → 18 file hits including `foundation_validation.rs:7162`, `7225`, `calls.rs:55`, lowering/optimization pipelines — used as a yes/no accessibility gate.

**Input (read):** A projection path whose suffix is only `Field` or `Case` segments after authority is already sufficient at `:3959-3960`; ceiling rules for those segments are not applied, yet the function returns `true`.

**Verdict: REACHABLE** — smallest program: a reference projection that walks through record field or enum case path segments where access ceilings should constrain the request but the loop’s default arm skips them.

## What I did not read

- `foundation_validation.rs` in full (only cited regions and targeted `rg` hits; ~11k lines).
- `descriptors.rs` beyond the `scalar_field_entails` call sites at `:1154` and `:1286-1287`.
- `emission.rs` beyond `:279` and `:4107-4109`.
- `proof_admission` / kernel internals beyond `identity_formations.rs:44-45` hook.
- Digest commit `457adf4a11f` snapshot via `git show` (current checkout lines matched cited sites at read).

SLICE COMPLETE
