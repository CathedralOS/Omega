# slice_0135

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1037 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage.rs:1213` | REACHABLE | Activation replay and zeroed-structural checks on module admission skip or weaken cases the digest names. |
| A1038 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/structural.rs:2090` | REACHABLE | Catalog validation and path typing accept skipped carriers and non-literal array indices. |
| A1039 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/substitution.rs:365` | REACHABLE | Boundary `requires` instantiation rebases content terms without the stored path prefix. |
| A1040 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptor_replay.rs:830` | UNDECIDED | Flow prep drops and case-select optimism reach lowering analysis; decode acceptance not fully traced. |
| A1041 | `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/descriptors.rs:1648` | REACHABLE | Reference-membership decode uses descriptor helpers that drop obligations or ignore failed graph build. |
| A1042 | `omega-rust/psi/compilation/representations/terminal-psi/src/identity.rs:89` | REACHABLE | `CheckedMachineDeclaration::same_identity_as` treats missing callable/result text as mutually equal. |
| A1043 | `omega-rust/psi/compilation/representations/terminal-psi/src/boundary.rs:301` | REACHABLE | Conformance commitment hashing maps absent identity indices to the empty string, collapsing distinct rows. |
| A1044 | `omega-rust/psi/compilation/representations/terminal-psi/src/boundary.rs:1288` | UNDECIDED | Unknown structural ids are skipped in non-observation walks; whether malformed modules carry such ids on admission is not shown. |

## A1037

Digest path `sem/storage.rs:1213` is `omega-rust/psi/compilation/representations/terminal-psi/semantics/src/storage.rs:1213` (read, lines match).

Read `ModuleActivation::validate_machine` calling `validate_flow` (`storage.rs:618-636`, `638-647`). In the per-operation match, unlisted `OperationKind` variants fall through `_ => {}` at `storage.rs:1213` while later checks at `1215-1238` still run.

Search `activation_storage::validate_module` over `omega-rust`: 2 production call sites — `codec/src/foundation_validation.rs:10476` inside `validate_structural_foundation` (`10256+`), and `pipeline/08_lowered-psi-to-terminal-psi/src/lib.rs:113`.

Successor plumbing: `arguments.get(index).copied().unwrap_or(&[])` at `storage.rs:1263`; paired `zip` relocations at `1304-1306` and `1370-1372` truncate when argument and parameter counts differ (read).

Read `zero_valid_record_storage` at `storage.rs:3620-3665`: `Record` fields at `3638-3640` call `zero_valid_field` with no `is_erased` filter. Search `zero_valid_record_storage` over the repository: 3 matches (definition, recursive call, `lib.rs` re-export only) — no production caller (read).

Read `zeroed_structural_shape` / `zeroed_declaration_graph` at `structural.rs:5396-5534` and `5481-5504`, `5729-5737`, `5777-5782`: sum/mixed zero-validity uses the first case only; `SumSelection` uses signed 32-bit integer tag at `5731` (read). `validate_establish_zeroed_structural` at `foundation_validation.rs:9727-9755` calls `zeroed_structural_shape` on decode admission (read).

Read `foreign_opaque_carrier` at `storage.rs:2251-2265`: returns `Ok(selected)` with `selected` possibly `None` when no representation row and no quotient carrier error (read); used from `foreign_storage_extent_dependencies` visit at `2292` (read), not from `validate_structural_foundation`.

**Input / acceptance:** A terminal module whose activation machine performs an operation kind omitted from the activation match yet keeps an activation-bearing structural result can pass `activation_storage::validate_module` when that module is admitted via structural foundation validation. Separately, a module that establishes a zeroed structural value whose sum tag zero does not select the first case can pass `validate_establish_zeroed_structural` when decoded.

## A1038

Digest `sem/structural.rs:2090-2099` → `structural.rs:2090-2099` (read). In `walk_primitive_projection_with_policy`, arm `FixedIndex` + `FixedArray` with `length.literal().is_none_or(|length| *index < length)` at `2092` accepts any index when length is not a literal (read).

Search `primitive_projection_declaration` over `omega-rust`: many call sites including `foundation_validation.rs:10664` on decode path (read listing, count ≥ 30 lines in grep).

Read `validate_inline_carriers` at `structural.rs:75-92`: fields with `inline_carrier == None` hit `continue` at `90-91` (read). `validate_catalog_type_shape` at `foundation_validation.rs:8074-8080` calls `validate_inline_carriers` and maps errors to `MalformedStructuralFoundation` (read).

**Input / acceptance:** A structural catalog entry whose fixed-array length is given only as a scalar application, paired with a constant index at or beyond the real length, can still yield a declared projection tip during foundation validation. A record type whose erased or structural field lacks an inline carrier is not rejected at catalog validation because that field is skipped.

## A1039

Digest `sem/substitution.rs:365-383` → `substitution.rs:365-391` (read). `substitute_content_term_places` rebases `ContentTerm::Projection` root via `substitutions.get(&subject.root).map(|(root, _)| *root)` and clones `segments` unchanged at `377-381`, discarding the prefix in the map value (read). Contrast: `substitute_proposition_structural_places` uses `subject.rebase(*root, prefix)` for structural place propositions at `200-203` (read).

Read callers: `substitute_proposition_structural_places` invoked from `foundation_validation.rs:2068-2070` when instantiating boundary `scalar_requires` against machine contract conjuncts (`2057-2083`) (read); also `descriptors.rs:3547`, `7040` (grep, read call lines only).

**Input / acceptance:** A boundary proposition checked during structural foundation validation whose content-conservation side names a projected place can compare against machine `requires` after rebasing only the root, so a forged or lowered module can appear to satisfy requires that should fail when the path prefix must move with the root.

## A1040

Read `prepare_flow` at `descriptor_replay.rs:818-904`: steps with no snapshot `continue` at `830-832`; calls missing fully resolved `argument_contents` `continue` at `898-904` (read).

Read `case_flow::select` at `1842-1852`: after `valid_subject`, `canonical_storage` returning `None` yields `Some(true)` at `1850-1851` (read). `establish` at `1821-1828` requires `canonical_storage` with `?`, so missing storage cannot establish a case fact (read).

Search `case_feasibility` / `DescriptorAnalysis` over `omega-rust`: production uses in `pipeline/06_checked-trees-to-lowered-psi/src/emission.rs:2153-2252` and `machine_lowering.rs:2141` (read grep hits). Search `prepare_flow(`: 2 call sites, both in `descriptor_replay.rs` (read).

**UNDECIDED:** Whether optimism in `select` or skipped `prepare_flow` steps lets a terminal module that should be rejected pass `decode_module` / `validate_structural_foundation` was not traced; lowering uses `FlowFeasibility` but the link from a misleading `Possible` feasibility to a module that later decodes cleanly was not read end-to-end.

## A1041

Read `expiring_input_predicate` at `descriptors.rs:1648-1665`: pure-predicate domains listed only in `additional_input` return `false` at `1656-1660` and do not constrain the store (read). Used from `enclosing_byte_predicates` / `whole_scalar_domains` at `1411-1418`, `1508-1516` (read).

Read `reference_carrier_memberships` at `1675-1709`: `SharedBorrow` with reinterpreted referent returns `Ok(Vec::new())` at `1705-1706` (read). `validate_reference_membership` calls `reference_carrier_memberships` at `foundation_validation.rs:7544-7555` inside `validate_reference_memberships` (`7436-7492`), which runs from `validate_structural_foundation` at `10478` (read).

Read scalar write replay at `descriptors.rs:5208-5244`: unresolved or multi-origin destinations `continue` without retiring observations at `5216-5222` (read). Payload capture gaps at unlisted operation kinds cited in digest (`9634`, `11663`, `12426`) not opened line-by-line in this slice.

Read `terminal_graph` at `descriptors.rs:13487-13488`: `terminal_graph_for(...).unwrap_or_default()` (read). `validate_reference_memberships` builds graph via `terminal_graph_from` at `7451-7458` (read) — closure errors surface from `terminal_graph_from`, but `terminal_graph` used elsewhere returns empty on failure (read at `13588-13593`).

Read missing-step edges at `descriptors.rs:653-655`, `421-423`: `steps.get(position)` absent → `continue` (read).

Read `_ => Some(DescriptorDefinition::Local)` at `13694` inside slot classification (read).

**Input / acceptance:** A decoded module with an establish-reference operation whose shared-borrow reinterpretation should retain source membership qualifications can pass membership validation when obligations are cleared. Descriptor analysis that drops write retirements or builds from an incomplete graph can admit reference operations that should fail foundation checks.

## A1042

Digest `TP/identity.rs:89-98` → `identity.rs:89-98` (read). `TerminalIdentityTable::text` returns `Option<&str>` at `identity.rs:354-356` (read). `CheckedMachineDeclaration::same_identity_as` compares `identities.text(self.callable) != other_identities.text(other.callable)` (and likewise parameter types and result type) at `92-98`; when both sides are `None`, `!=` is false and comparison proceeds as equal for those fields (read). `NominalDeclarationIdentity::same_identity_as` treats absent/empty text as unequal at `33-39` (read); unit tests at `168-194` cover nominal ids only, not `CheckedMachineDeclaration` (read).

Search `\bsame_identity_as\b` on `CheckedMachineDeclaration` uses: `mathematical_proof_actual.rs:877`, `provider_admission` / `callback_artifacts.rs` / `calls.rs:819` (grep, read `codec/src/semantic_module.rs:1537-1554` — decode validates commitments only, not text equality) (read).

**Input / acceptance:** Two checked machine declarations that differ only by missing identity-table entries for callable or result type text are treated as the same contract for identity comparison, so provider or proof matching that relies on `same_identity_as` can accept the wrong declaration.

## A1043

Read `closed_conformance_application_bytes` at `boundary.rs:289-348`: `identity!` macro uses `identities.text($identity).unwrap_or_default()` at `300-302`; similar `unwrap_or_default` at `339`, `345`, `419-421` (read). `ownership.rs:1033` hashes field path with `unwrap_or_default()` (read).

Search `closed_conformance_application_report_fingerprint` over `omega-rust`: uses in `boundary.rs`, `calls.rs:1338`, `pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:2289`, tests (grep, read).

**Input / acceptance:** Two closed-conformance applications whose distinct missing identity indices both hash as empty strings produce identical commitment fingerprints, so conformance or retention logic can treat distinguishable applications as the same.

## A1044

Read `requires_checked_non_observation` at `boundary.rs:1267-1343`: `lookup(current)` returning `None` → `continue` at `1288-1290` (read). `structural_type_contains_by` at `types_and_values.rs:1259-1265` skips unknown ids similarly (read).

Read external provider `validate` at `boundary.rs:1243-1251`: only empty vtable/table field strings error; other `ExternalBindingIdentity` kinds hit `_ => Ok(())` at `1250` (read).

Search `requires_checked_non_observation` over `omega-rust`: `provider_admission.rs:251`, interpreter tests, `engine_calls.rs:3354` (grep, read admission line). `structural_type_contains_reference` used in legalization and unit validation, not directly in `validate_structural_foundation` (grep, inferred).

**UNDECIDED:** Production structural foundation validation supplies lookups over the module’s own structural type table; whether a module can be admitted with parameter types referencing unknown `StructuralTypeId` values that should force non-observation rejection was not demonstrated (read stopping at lookup-skip semantics).

## What I did not read

- Full bodies of `descriptor_replay` consumers beyond cited call sites; entire `descriptors.rs` capture/invalidate tables at `9634`, `11663`, `12426`, `13488`, `13589`, `13593` line-by-line.
- `git show 457adf4a11f` diff for line drift (current checkout lines matched for opened sites).
- Typed-trees lowering between `FlowFeasibility::Possible` and final `decode_module` outcome for A1040.
- Every caller of `foreign_storage_extent_dependencies` / opaque boundary layout beyond `storage.rs:2292`.

SLICE COMPLETE
