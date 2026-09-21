# SIGNATURE-FREE-TRAIT-CANDIDATE-SCOPE — scope verification record

Board row: TASKS.md `SIGNATURE-FREE-TRAIT-CANDIDATE-SCOPE` (mined
candidate, ~:13559), assigned as NEW-SV-SIGNATURE-FREE-TRAIT-CANDIDATE-SCOPE
with the planner scope `wiki/drafts/scope_signature_free_trait_candidate_scope.md`.

## Surface

`omega-rust/psi/pipeline/syntax-trees-to-symbol-resolved-trees/src/selection/signature_free_requirements.rs`
is the shared law for every path that names one trait requirement
**without a call signature**. Neither visible satisfiers nor an expected
call shape may select among overloads. Three consumers resolve through it:

- **Domain establishment routes** (`domain.authored_routes`) — a route
  names either a trait requirement or an exact machine.
- **Nominal machine binders**
  (`data_type_parameters`, `MachineParameterContract::AuthoredNominal`) —
  the requirement path a binder must satisfy.
- **Occurrence lookup** — `signature_free_trait_candidates` /
  `signature_free_machine_candidates` for selection-time joins.

## Selection law (verified at `59e0b5ec22d09`)

1. The trait path (all members but the last) resolves through
   `symbols::lookup_signature_free_top_level_from_source_matching` —
   ordinary namespace selection scoped to the occurrence's source
   package — restricted to `SymbolKind::Trait`. Anything but one exact
   trait is `TraitNotUnique`.
2. The requirement leaf must name exactly one machine signature on that
   trait **visible from the use span**
   (`source_reference_can_see_symbol`); else `RequirementNotUnique`.
3. Exact-machine routes (`a::b::make`, `Data::method`) resolve through
   the same namespaced lookup filtered to machines with
   `spelling.is_none()` (operator declarations are not exact-machine
   routes); any non-singleton is `NotUnique`.
4. `validate_signature_free_requirement_compatibility` reports the
   overload break on **both** sides before either normalizer consumes
   the paths: authored use sites (nominal machine parameter
   requirements + domain routes, sorted by use span) and declaring-trait
   families (sorted by trait declaration span). A route that uniquely
   names a machine skips the requirement-ambiguity diagnostic — trait
   ambiguity does not apply to exact-machine routes; cross-kind
   ambiguity still rejects at normalization.

## History

`known_baseline_failures.md` (package-evidence section) records the
over-collection this seam shipped: before `9898f252ec` /
`0d51bad72a`, `resolve_signature_free_requirement` collected every
same-named trait **program-wide** filtered only by resolution stratum,
so a fixture package declaring its own `ExtentRootProvider` collided
with the seeded `core/extent.omg` trait as `TraitNotUnique`. The repair
scoped candidates to the occurrence's package via the
`lookup_signature_free_top_level_from_source_matching(..., use_span, ...)`
route both candidate functions now share.

## Re-verification at `59e0b5ec22d09` (linux x86-64, cargo nextest)

`cargo nextest run -p syntax-trees-to-symbol-resolved-trees -E
'test(/signature_free/)'` — **11/11 pass**, including:

- `signature_free_route_keeps_the_imported_trait_with_an_unimported_competitor`
  — imported trait wins over an unimported same-named competitor
  (the original package-evidence regression pin).
- `signature_free_route_selects_the_occurrence_package_scope` —
  package-scoped candidate set.
- `signature_free_routes_keep_imports_file_local_and_nontransitive` —
  import visibility is not transitive through signature-free routes.
- `signature_free_route_rejects_unimported_module_{trait,machine}`,
  `signature_free_route_rejects_applicable_flat_trait_competitors`,
  `signature_free_route_still_rejects_a_contested_same_package_leaf` —
  ambiguity still refuses, now per-scope.
- `signature_free_route_selects_the_imported_attached_machine` —
  the exact-machine arm.
- `rejects_overloaded_signature_free_domain_requirement_route`,
  `signature_free_overload_reports_one_declaration_and_every_affected_use`,
  `authored_signature_free_requirement_ignores_current_activation_extension_overloads`
  — the two-sided compatibility diagnostic.

## Slice status

No independent slice under this name: the candidate-scope law is landed,
package-scoped, and pinned by the 11-member battery above. Named
residuals live elsewhere:

- Symbolic (generic-owner) boundary applications reject in
  `typed-trees-to-checked-trees/src/operators/applications.rs`
  ("symbolic statement boundary applications are not yet supported") —
  upstream-gated on final substitution, not a resolver gap.
- The `same_semantic_name` leaf-vs-qualified equality helper remains the
  only non-namespaced name equality on this surface; any widening of it
  is a compatibility decision, not an implementation slice.
