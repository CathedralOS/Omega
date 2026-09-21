# RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY — lane status at 2e5d4a73247

Board row: `TASKS.md` (restore ordinary native descriptor invocation and
forwarding — one borrowed two-word descriptor, forwarded once, requirement
invoked, result used across computations and branches). Owner:
Zergling-108. Host: Linux x86-64, revision `2e5d4a73247`.

## Re-verified legs (green at this revision)

| Leg | Command | Result |
| --- | --- | --- |
| Source-side forwarded-helper custody, incl. pinned regression `forwarded_descriptor_calculations_execute_through_verified_artifact` | `RUST_MIN_STACK=67108864 cargo nextest run -p checked-trees-to-lowered-psi --lib -E 'test(dynamic_composed_unit)'` | 46/46 PASS (2.2s) — the macOS-named recipe passes on Linux x86-64 too |
| a2t descriptor parameter lowering + replay | `cargo nextest run -p abstract-operations-to-target-operations --lib -E 'test(dynamic_parameters)'` | 9/9 PASS — roster binding, requirement slot, dispatch plan, table offset, two-pointer-word ABI all replay |

## Landed state confirmed in tree

- `TargetUnitOperation::DynamicParameter{Scalar,Unit}Call` legalized and
  replayed (`legalization/scalar_graph_input/target/unit/indirect_calls.rs`,
  dispatched at `unit.rs:534`).
- `LegalizedDynamicParameterCall` exists in `legalized-operations`; selection
  construction/validation classify the kind but fall to `return Err(invalid())`
  (`selection/construction/scalar_graph.rs:1153`,
  `selection/validation/scalar_graph.rs:1234`) — no
  `SelectedInstructionKind` names it, matching the row's residual.
- `machine_code::{DynamicParameterCallRecord,
  ForwardedDynamicParameterCallRecord}` and image-emission's
  `object_artifact/replay/dynamic/{forwarded_descriptor,forwarded_parameter}.rs`
  recognizers still exist — the delete-with-closure candidates named by the row.

## Residual fence map at claim time

Every surface the remaining work would touch is live-claimed:

- `selection/construction/scalar_graph.rs` + `selection/validation/scalar_graph.rs`
  (where a DynamicParameterCall selection arm lands) — SIGNED-CALL-PREMISES
  legs (Codex), exp 22:23Z.
- `backend/image-emission` wholesale — WIRE-RUNTIME-AND-INSTALLATION (z163),
  exp 02:09Z; `object_artifact/replay/unit/call_custody*` —
  STRUCTURAL-BORROW-IDENTITY, exp 05:17Z.
- `native-artifact/src/physical` + `mixed_structural_scalar.rs` —
  PHYSICAL-ACCESS-PROFILES, exp 02:00Z.
- `checked-trees-to-lowered-psi/src/tests/dynamic_composed_unit*` —
  BASELINE-SERVICE-CARRIER-FAILURES, exp 05:15Z.

The row's own constraint applies: "the dependency is an ordinary indirect-call
operand… not another whole-body recognizer. Do not resume target-only
descriptor composition: two such milestones left this native customer
unsupported." A selection-stage arm alone — without ISA encoding, emission,
and the native differential fixture — would be exactly that milestone, so no
unfenced genuinely-implementable slice exists under this claim surface.
