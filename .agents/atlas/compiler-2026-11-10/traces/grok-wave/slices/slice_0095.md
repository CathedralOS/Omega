slice_0095

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0728 | UV:15482-15484 | REACHABLE | A `None` dispatch row skips the dependency edge; the walk returns `Ok` without visiting a callee that a populated row would have enqueued. |
| A0729 | UV:15623-15625 | CLOSED | `calls.rs:2340-2347` rejects proof-output runtime sites that never appear in the rebuilt operation list. |
| A0730 | UV:15709-15715 | REACHABLE | Evidence bodies omit values with no descriptor `scalar_expression`, so the shared validator never sees those values in `scalar_expressions`. |
| A0731 | UV:16024-16046; UV:18223; UV:18225; UV:15920 | REACHABLE | On `revalidate_rewrite_output`, refresh overwrites `root_service_reach` before `validate_root_service_reach` compares declared to derived. |
| A0732 | UV:16084-16144 | REACHABLE | Boundary catalog checks never run `structural_projected_qualifications_match` on boundary `structural_parameters`. |
| A0733 | UV:16763-16764 | CLOSED | Later witness and operation-to-place checks (`UV:9603-9604`, `UV:16820-16834`) require producing operations. |
| A0734 | UV:16919-16920 | REACHABLE | Missing field identity text becomes `""` via `unwrap_or_default`, conflated with a real empty field name in binding comparison. |
| A0735 | UV:17919-17921 | CLOSED | Full unit validation rejects unknown attachments at `UV:16131-16132` / `UV:16696-16702` before acceptance. |

### A0728

1. Read `validate_alternative_result_dependencies` at `unit_validation.rs:15438-15493` (full) and the `Dynamic` arm at `UV:15474-15487` (read).
2. Search: `rg -w 'validate_alternative_result_dependencies' /agent/repos/omega/omega-rust` — **2 hits** (definition `UV:15438`, call `UV:15375`).
3. Call chain read: `UV:15375` inside `validate_function_references` (`UV:15212-15436`, full); `UV:9619` from `validate_function` (`UV:9546-9624`, header and call site only); top-level `validate_psi_optimization_unit_with_control_cycles` at `UV:18260-18269` (read phase list only).
4. Input on the accepting branch: structural reference result whose `reference_sources` triggers `UV:15365-15375`, with a `CallStructural` using `AbstractCallTarget::Dynamic` where `table.rows[slot]` is `None` (in-bounds slot per `UV:15481`, read).
5. **Verdict: REACHABLE** — A machine returning a reference type with an alternative invocation mapping can call a dynamic slot whose table row is `None`; the DFS completes without following a callee machine edge that a `Some(machine)` row would add.
6. Unsure whether skipping that edge can change outcomes beyond this walk; post-walk invocation checks at `UV:15388-15407` (read) do not substitute for callee traversal.

### A0729

1. Read `runtime_evidence::validate_current` `UV:15524-15765` (operations loop `UV:15584-15690` in full; tail `UV:15754-15764`).
2. Search: `rg 'runtime_evidence::validate' /agent/repos/omega/omega-rust` — **1 hit** (`UV:18267`). Caller chain: `validate_psi_optimization_unit_with_control_cycles` `UV:18267` (read).
3. Dynamic `CallStructural` hits `let Some(callee) = callee.machine() else { continue }` at `UV:15623-15625` (read).
4. Input: function with dynamic structural call; that operation is absent from `RuntimeEvidenceBody.calls`.
5. Read `terminal-psi/semantics/src/calls.rs:2340-2347` (read): any `proof_output_calls` entry with `runtime_call: Some` whose `(caller, site)` was not retained during the per-operation loop returns an error.
6. **Verdict: CLOSED** — A catalog proof-output runtime site for an omitted dynamic call still fails at `calls.rs:2340-2347` when evidence validation is not short-circuited at `calls.rs:2127-2137`.

### A0730

1. Read `UV:15697-15715` and enclosing function through `validate_runtime_evidence_inputs_in` invocation `UV:15754-15764`.
2. Search: `rg -w 'validate_runtime_evidence_inputs_in' /agent/repos/omega/omega-rust` — **3 hits** (import/use `UV:15515`, call `UV:15754`, definition in `calls.rs:2123`).
3. Input: observed evidence function collects value ids, then `filter_map` drops any value for which `descriptor_results::scalar_expression` returns `None` (`UV:15709-15714`, read; `descriptor_results.rs:2733-2738`, read).
4. **Verdict: REACHABLE** — A value that should supply a scalar expression for runtime evidence can be omitted from `scalar_expressions`; the downstream validator only sees the filtered map.
5. Unsure whether `validate_runtime_evidence_inputs_in` always requires every collected value id to appear; did not read that function past `calls.rs:2347`.

### A0731

1. Read `refresh_root_service_reach` `UV:16024-16046` (full), `validate_root_service_reach` `UV:15885-15932` (full, especially `UV:15920-15927`), `revalidate_rewrite_output` `UV:18213-18226` (full).
2. Search: `rg -w 'refresh_root_service_reach' /agent/repos/omega/omega-rust` — **2 hits** (definition `UV:16024`, call `UV:18223`). `revalidate_rewrite_output` also at `candidates.rs:334,2463,10680` (grep only, not read).
3. Input on rewrite path: `revalidate_rewrite_output` mutates `unit.root_service_reach` to derived at `UV:18223` then runs `validate_psi_optimization_unit` at `UV:18225`; `validate_root_service_reach` at `UV:2268` compares `unit.root_service_reach` to freshly derived.
4. **Verdict: REACHABLE** — An optimization unit whose stored `root_service_reach` disagrees with `derive_root_service_reach` can pass rewrite revalidation because refresh aligns declared with derived before comparison.
5. Unsure whether non-rewrite entry points ever call `refresh_root_service_reach` first; only the `revalidate_rewrite_output` order was read.

### A0732

1. Read `boundary_structural_signature_matches` `UV:16074-16123` and `structural_signature_matches` `UV:16125-16145` (both full).
2. Search: `rg -w 'boundary_structural_signature_matches' /agent/repos/omega/omega-rust` — **3 hits** (`UV:1081` re-export, `UV:1197`, `UV:16074`).
3. Read caller `UV:1191-1205` (boundary loop in catalog indexing). Compared to function parameters checked at `UV:16714-16718` in `validate_function_structural_catalog` (read).
4. Input: `boundary.structural_parameters` reach `structural_qualifications_match` only (`UV:16139-16143`); no `structural_projected_qualifications_match`.
5. **Verdict: REACHABLE** — A boundary machine declaration can carry invalid `projected_qualifications` on its structural parameters yet pass `boundary_structural_signature_matches`.
6. Unsure whether every bad boundary projection is caught on `BoundaryCall` operations; read `UV:12642-12657` call-site match only, not full `structural_arguments_match` (`UV:13182+`).

### A0733

1. Read `validate_function_structural_catalog` `UV:16683-16836` (place loop and operation matching through `UV:16834`).
2. Search: `rg -w 'validate_function_structural_catalog' /agent/repos/omega/omega-rust` — **4 hits** (definition, re-export, call `UV:9581`, module export).
3. Caller read: `validate_function` `UV:9581-9604` schedules `validate_byte_sequence_literal_witnesses` and `validate_trivial_affine_local_witnesses`.
4. At `UV:16763-16764` `ByteSequenceLiteral` and `TrivialAffineLocal` set `known_type = true` without a producer check (read). `UV:16820-16834` ties `EstablishByteSequenceLiteral` to place kinds (read). Witness functions `UV:17958-18045` (read headers and mismatch returns).
5. **Verdict: CLOSED** — A place without a producing operation fails later witness or establish-vs-place enforcement even though `known_type` is permissive here.
6. Entry description matches the code at `UV:16763-16764` (read).

### A0734

1. Read `validate_function_structural_catalog` content-entry binding block `UV:16892-16947` (full).
2. Search: `rg 'unwrap_or_default' /agent/repos/omega/omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs` near `UV:16917` — single site in this binding (read).
3. Input: `StructuralPathSegment::Field(identity)` with `unit.identities.text(*identity)` missing maps to `Field("")` at `UV:16913-16918` (read).
4. **Verdict: REACHABLE** — A missing identity is indistinguishable from a deliberate empty field name when comparing claim segments to entry declarations.
5. Did not read whether upstream identity indexing forbids referencing absent identities on this path (`identity_indexes` at `UV:18264` named only).

### A0735

1. Read `attachment_storage_types` `UV:17908-17947` (full) and caller in `index_structural_types` `UV:16215-16219` (read).
2. Search: `rg -w 'attachment_storage_types' /agent/repos/omega/omega-rust` — **3 hits** (`UV:16157`, `UV:16215`, `UV:17908`).
3. At `UV:17916-17920`: id is inserted into `storage` before `types.get`; missing declaration uses `continue` without error (read).
4. Full-unit path: `structural_signature_matches` rejects unknown attachment types `UV:16131-16132`; per-function `UV:16696-16702` (read).
5. **Verdict: CLOSED** for `validate_psi_optimization_unit` — unknown attachment ids are rejected in function signature validation despite being retained in the storage set during catalog indexing.
6. Did not trace `validate_psi_structural_catalog` (`UV:18240-18243`, read) as a product acceptance gate; it only runs `index_structural_catalogs`.

### What I did not read

- Full bodies of `structural_arguments_match` / `structural_arguments_match_at_invocation` beyond `UV:13182-13230`.
- Full `validate_runtime_evidence_inputs_in` in `calls.rs` between `UV:2145` and `UV:2339`.
- `candidates.rs` rewrite paths that call `revalidate_rewrite_output` (grep hits only).
- Complete `catalogs::index_and_validate_unit_catalogs` and every `validate_function` caller.
- `identity_indexes::validate_identity_and_fact_indexes` implementation.

SLICE COMPLETE
