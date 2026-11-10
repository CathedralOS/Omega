# slice_0095

Preamble read: `digests/area_05.md` lines 1–27 (`UV` → `omega-rust/omega/compilation/representations/abstract-operations/optimization-unit/semantics/src/unit_validation.rs`). Cited lines opened in this checkout; line numbers match the digest citations (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0728 | unit_validation.rs:15482-15484 | REACHABLE | Empty dynamic-dispatch row adds no callee dependency during alternative reference dependency walk. |
| A0729 | unit_validation.rs:15623-15625 | REACHABLE | Dynamic `CallStructural` sites are skipped when building runtime-evidence operation lists. |
| A0730 | unit_validation.rs:15709-15715 | REACHABLE | Observed scalar value IDs without a descriptor scalar expression never enter `scalar_expressions`. |
| A0731 | unit_validation.rs:16024-16046,18223-18225,15920-15930 | REACHABLE | Rewrite revalidation refreshes reach before compare, so declared/derived mismatch cannot fail there. |
| A0732 | unit_validation.rs:16084-16144 | REACHABLE | Boundary signature helpers check qualifications only, not `projected_qualifications` on boundary parameters. |
| A0733 | unit_validation.rs:16763-16764 | CLOSED | Orphan literal/local arms here are followed by witness validators that require matching establishments. |
| A0734 | unit_validation.rs:16919-16920 | REACHABLE | Missing field identity text becomes `""`, which can match an entry-declaration path segment `""`. |
| A0735 | unit_validation.rs:17919-17921 | CLOSED | Unknown attachment type IDs still fail `structural_signature_matches` before unit validation completes. |

## A0728

Digest claim 65 (read). Opened `unit_validation.rs:15438-15493` function `validate_alternative_result_dependencies` (read). At `15479-15484` (read), for `AbstractCallTarget::Dynamic`, when `table.rows.get(*slot)` is `Some(row)` and `row` is `None`, the loop does not `pending.push` any machine—no dependency edge. Out-of-bounds slot still errors at `15481` via `ok_or_else(invalid)`.

Caller search: `\bvalidate_alternative_result_dependencies\b` in `omega-rust`, count mode → **2** matches, all in `unit_validation.rs` (definition + call at `15375`) (read).

Callee path (read): `references::validate_function_references` `15212-15376` calls this only when `result.reference_sources` contains `StructuralReferenceOrigin::Invocation` (`15365-15375`). `validate_function` invokes `validate_function_references` at `9619`. `validate_psi_optimization_unit_with_control_cycles` runs catalogs then `runtime_evidence::validate` at `18264-18267`.

Input that reaches the branch (read): a function whose structural reference result uses invocation-style alternative sources and has a `CallStructural` to a dynamic target whose dispatch table row for the slot is present but `None`, while the result type still carries reference dependencies.

Smallest wrong program (plain words): a reference-typed return whose proof relies on a dynamically dispatched callee that has no machine in the dispatch table row, so the validator never walks that callee but still accepts the reference result graph.

Verdict **REACHABLE** (read).

## A0729

Digest claim 66 (read). Opened `runtime_evidence::validate_current` `15524-15765` (read). At `15615-15625`, `O::CallStructural` uses `callee.machine()?`; on `None` (dynamic callee) the match arm `continue`s, so no `RuntimeEvidenceOperation` is recorded for that node.

Caller search: `\bruntime_evidence::validate\b` in `omega-rust` → **1** production call at `unit_validation.rs:18267` (read). `validate_current` is private; only `validate` exports.

Input (read): unit with `unit.runtime_evidence` set, evidence lanes requiring call checking (`checks_calls` true), and a block node that is `CallStructural` with `AbstractCallTarget::Dynamic`.

Smallest wrong program (plain words): a machine with runtime-evidence obligations and a dynamic structural call that should participate in evidence replay, but the call never appears in the collected operation list passed to `validate_runtime_evidence_inputs_in` at `15754`.

Verdict **REACHABLE** (read); did not read full `validate_runtime_evidence_inputs_in` in `terminal-psi/semantics/src/calls.rs` beyond its import site.

## A0730

Digest claim 67 (read). Same function `validate_current`, lines `15697-15715` (read). When `observes` is true, value IDs are gathered, then `filter_map` drops any ID where `descriptor_results::scalar_expression(function, value)` returns `None`.

Caller path: identical to A0729 through `runtime_evidence::validate` at `18267` (read).

Input (read): runtime-evidence catalog present, a caller observes evidence (`observes == true`), and a collected argument or scalar result value has no reconstructible scalar expression in the descriptor graph.

Smallest wrong program (plain words): an evidence-bearing call whose scalar actual is a value the descriptor layer cannot express, omitted from `scalar_expressions` and thus invisible to downstream evidence input checking.

Verdict **REACHABLE** (read).

## A0731

Digest claim 68 (read). Read `refresh_root_service_reach` `16024-16046` (read): assigns `unit.root_service_reach` from `derive_root_service_reach`. Read `revalidate_rewrite_output` `18213-18226` (read): `services::refresh_root_service_reach(output)?` at `18223`, then `validate_psi_optimization_unit(output)?` at `18225`. Read `validate_root_service_reach` `15885-15932` (read): at `15920-15930` compares `derive_root_service_reach` to `unit.root_service_reach`.

Caller search: `\brefresh_root_service_reach\b` in `omega-rust` → **2** matches (definition + `18223`) (read). `\bvalidate_root_service_reach\b` → **3** matches including `catalogs::validate_final_authorities` at `2268` (read).

Paths (read): primary validation calls `validate_root_service_reach` without preceding refresh (`2268`). Rewrite output path always refreshes first (`18223`), so the comparison at `15920` is derived versus already-overwritten declared reach.

Smallest wrong program (plain words): rewrite output carrying an incorrect `root_service_reach` declaration that should be rejected, silently replaced by derived reach and then validated as matching itself.

Verdict **REACHABLE** on the rewrite path (read). Primary validation without refresh would still reject mismatch at `15921-15927` (read)—not a bypass on that path.

## A0732

Digest claim 69 (read). Read `boundary_structural_signature_matches` `16074-16123` and `structural_signature_matches` `16125-16145` (read). Parameter validation uses `structural_qualifications_match` on `parameter.qualifications` only (`16139-16143`); no `structural_projected_qualifications_match` on boundary `structural_parameters`.

Caller search: `\bboundary_structural_signature_matches\b` in `omega-rust` → **3** matches in `unit_validation.rs` (definition + `1197` in `index_and_validate_unit_catalogs`) (read).

Contrast (read): `validate_function_structural_catalog` checks function parameters' `projected_qualifications` at `16714-16718`, but boundary declarations at `1191-1205` only call `boundary_structural_signature_matches`.

Repository search: `boundary.*projected_qualifications` in `unit_validation.rs` → **no matches** (read).

Smallest wrong program (plain words): a boundary machine declaration whose structural parameters carry non-empty or invalid `projected_qualifications` while ordinary qualifications and domains look valid, accepted at boundary catalog indexing.

Verdict **REACHABLE** (read).

## A0733

Digest claim 70 (read). In `validate_function_structural_catalog` `16683-17062`, place loop at `16763-16764` sets `known_type` true for `ByteSequenceLiteral` and `TrivialAffineLocal` without requiring a producer in that match (read). Same function later pairs establishments at `16820-16835` (`EstablishByteSequenceLiteral` only; `EstablishTrivialAffineLocal` maps to `None` at `16822`) (read).

Closing checks (read): `validate_function` at `9603-9604` calls `validate_byte_sequence_literal_witnesses` (`17958-17997`) and `validate_trivial_affine_local_witnesses` (`18000-18045`). Witnesses require a bijection between expected places from the catalog and establishment operations; leftover expected places fail at `17990-17995`.

Caller search: `\bvalidate_byte_sequence_literal_witnesses\b` → **4** matches, production use at `9603` (read).

Smallest wrong program blocked (read): a function listing a byte-sequence literal or trivial-affine-local place in `structural_places` with no matching establishment operation fails witness mismatch, not accepted.

Verdict **CLOSED** via `unit_validation.rs:9603-9604` and `17990-17995` / `18038-18041` (read).

## A0734

Digest claim 71 (read). In `validate_function_structural_catalog`, content-entry-claim check `16892-16947` (read). At `16913-16919`, `StructuralPathSegment::Field(identity)` uses `identities.text(*identity).unwrap_or_default().to_string()` for comparison to `entry_claim_declarations` paths.

`validate_identity_and_fact_indexes` `9684-9729` (read) checks unit identity hash and fact indexes only; no field-identity resolution for content claims.

Other empty-text guard (read): structural-domain establishment routes reject absent/empty identity text at `16430-16436`, a different site.

Smallest wrong program (plain words): a content entry claim whose field segment uses a missing identity index while a declaration path segment is the empty string, making `structural_binding_matches` true at `16905-16940` without a real field binding.

Verdict **REACHABLE** (read).

## A0735

Digest claim 72 (read). Read `attachment_storage_types` `17908-17947` (read). At `17915-17920`, missing `types.get(&identity)` still inserts `identity` into `storage` and `continue`s without error.

Caller search: `\battachment_storage_types\b` in `omega-rust` → **5** matches (`unit_validation.rs` definition + call at `16215`; parallel copy in `foundation_validation.rs`) (read).

Closing path (read): `index_structural_types` calls this at `16215` before per-function validation, but any function with `attachment: Some(id)` where `id` is absent from the type map fails `structural_signature_matches` at `16131-16132` when `validate_function_structural_catalog` runs (`16696-16702`), invoked from `validate_function` `9581-9586`. Boundaries fail similarly at `1197-1205` via `boundary_structural_signature_matches` → `structural_signature_matches`.

Smallest wrong program blocked (read): attachment type id not in the structural catalog cannot pass function or boundary signature matching; unit validation errors before acceptance.

Verdict **CLOSED** via `unit_validation.rs:16131-16132` and `16696-16702` (read). Silent `continue` at `17919-17921` does not admit the unit.

## What I did not read

- Full `validate_runtime_evidence_inputs_in` in `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/calls.rs` (beyond import and early lines).
- `structural_arguments_match` / `BoundaryCall` projected-qualification handling elsewhere in `unit_validation.rs` beyond the boundary-signature site.
- `unit_026_gap_013.md` (digest paraphrase only, read from `area_05.md`).

SLICE COMPLETE
