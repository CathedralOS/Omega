# CANARY-CORE-NAME-COLLISION — resolves as already-landed at 27deadf412

Mined candidate; the row is a stub ("verify scope then implement"). Scope
resolves to the fixture
`tests/omega/fail/modules/bundled_core_name_collision_rejected`: a
requester-local source at `omega/language/core/marker.omg` sitting below the
source root, while `use omega::language::core::marker;` binds the bundled
toolchain namespace — the collision must be *named*, not silently shadowed.

## Already landed

- `542046b78f` "frontend: refuse omega::language imports that shadow
  requester-local sources" — both import lanes name the collision:
  `discover_imports` rejects when a bundled spelling reaches an existing
  local source, and `reconciled_package_import` checks the requester's
  package root before routing `omega::language::core` to the toolchain.
- Fixture rostered as `BUNDLED_CORE_NAME_COLLISION_REJECTED`
  (`fixture_rosters/surface_and_targets.rs:30`) and pinned through
  `surface_and_targets::duplicate_overload_and_visibility_admissions_reject`
  — `expected.txt` fragment "collides with a requester-local source" must
  appear in checked-semantics diagnostics.

## Verified at 27deadf412 (Linux x86-64)

`cargo nextest run -p compiler --test canary_suite -E
'test(=surface_and_targets::duplicate_overload_and_visibility_admissions_reject)'`
— PASS (covers this fixture plus the sibling duplicate-admission pins).
No residual slice remains under this name.
