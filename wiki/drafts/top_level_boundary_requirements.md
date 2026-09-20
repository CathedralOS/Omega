# TOP-LEVEL-BOUNDARY-REQUIREMENTS — lane re-verification at 17fec446ef

Board row: `TASKS.md` "TOP-LEVEL-BOUNDARY-REQUIREMENTS" (explicit public
`boundary requirement Owner::name(...);` declarations). Owner:
Zergling-108. Host: Linux x86-64, revision `17fec446ef`.

## Verified legs (all green at this revision)

| Leg | Command | Result |
| --- | --- | --- |
| Pass canaries `*boundary_requirement*` (dispatch, terminal, statement-call, external-via-exit) | `OMEGA_PASS_CANARY_FILTER=boundary_requirement cargo nextest run -p compiler --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile` | PASS (82.9s) |
| Fail fences `*boundary_requirement*` (direct-call unselected, statement-call unselected, private) | `OMEGA_FAIL_CANARY_FILTER=boundary_requirement cargo nextest run -p compiler --test canary_suite proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment` | PASS |
| Owned-`self` receiver route (`tests/fixtures/boundary-requirement-member-call`) | `cargo nextest run -p compiler --test canary_suite top_level_requirement_member_call` | 2/2 PASS — checked interpreter 3.4s, native artifact 80.1s |
| External satisfier scalar control | `cargo nextest run -p compiler --test efb3_flat_record_probe top_level_external_requirement_returns_and_reuses_its_result_natively` | PASS (14.5s) on Linux x86-64 — the board row names this as the macOS ARM64 control; it is green here as well, though the row's caveat stands that other hosts remain unverified *by that test's* assertions |

## Residual bullets — confirmed still open at this revision

- **Parameterized / projected receivers**: `is_directly_callable_top_level_requirement`
  (`omega/build/selected-dispatch/src/boundary_dispatch.rs:645`) still requires
  `lifetime_parameters.is_empty()`, no machine type parameters, and no `self`
  state parameter. `Task::finish<T>` / `request_cancel` /
  `InterruptMaskGuard::restore` / `InterruptAcknowledgement::complete` remain
  unsatisfied and route to BOUNDED-INSTALLATION-REACH-ROWS.
- **Terminal identity + era replay**: `terminal_module/boundary/conformances.rs`
  conformance rows remain trait-keyed; no canonical requirement-kind row yet.
- **Operator respell**: `source/library/core/float_operations.omg` still holds
  124 `boundary operator` rows; `MachineSupplyMode::Boundary` is still assigned
  in `syntax-trees-to-symbol-resolved-trees/src/lowering/machine.rs` (lines
  148, 163). Both are gated on the migration leg (OPERATOR-MACHINE-SUPPLY owns
  the `operator` introducer removal).
- **Borrowed-record external join**: remains a frontend dependency per the row
  (mixed-argument probe `Move::shift` rejected at Terminal production
  `statement sequence: call: call operation, statement 2` at `4752d94c3f6`);
  not re-executed on this host — macOS ARM64 recipe.

## Scope note

The claimed slice here is the verification record only; the four residual
bullets are each multi-surface features (selected-dispatch admission, provider
execution composition, terminal conformance identity, library respell +
mode removal) owned across `selected-dispatch`, `provider-planning`, Psi
lowering, and Terminal — none is a single-session slice under the current
claim surface.
