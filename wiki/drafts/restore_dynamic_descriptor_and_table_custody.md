# RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY — lane status at 479ceb0e68

Board row: `TASKS.md` (restore ordinary native descriptor invocation and
forwarding — one borrowed two-word descriptor, forwarded once, requirement
invoked, result used across computations and branches). Lane status drafted
by Zergling-108 at `2e5d4a73247`; re-verified and fence map refreshed by
zergling-z168 at `479ceb0e68` (Linux x86-64).

## Re-verified legs (green at this revision)

| Leg | Command | Result |
| --- | --- | --- |
| Source-side forwarded-helper custody, incl. pinned regression `forwarded_descriptor_calculations_execute_through_verified_artifact` | `RUST_MIN_STACK=67108864 cargo nextest run -p checked-trees-to-lowered-psi --lib -E 'test(dynamic_composed_unit)'` | 46/46 PASS (2.3s) — the macOS-named recipe passes on Linux x86-64 too |
| a2t descriptor parameter lowering + replay | `cargo nextest run -p abstract-operations-to-target-operations --lib -E 'test(dynamic_parameters)'` | 9/9 PASS — roster binding, requirement slot, dispatch plan, table offset, two-pointer-word ABI all replay |

## Landed state confirmed in tree

- `TargetUnitOperation::DynamicParameter{Scalar,Unit}Call` legalized and
  replayed (`legalization/scalar_graph_input/target/unit/indirect_calls.rs`,
  `legalization/source/scalar_graph/instruction/call_instructions.rs:354`).
- `LegalizedDynamicParameterCall` exists in `legalized-operations`; selection
  construction/validation classify the kind but fall to `return Err(invalid())`
  (`selection/construction/scalar_graph.rs:1153`,
  `selection/validation/scalar_graph.rs:1234`) — no
  `SelectedInstructionKind` names it, matching the row's residual.
- `machine_code::{DynamicParameterCallRecord,
  ForwardedDynamicParameterCallRecord}`
  (`representations/machine-code/src/machine_code/calls/dynamic.rs:160`) and
  image-emission's
  `object_artifact/replay/dynamic/{forwarded_descriptor,forwarded_parameter}.rs`
  recognizers still exist — the delete-with-closure candidates named by the row.

## Residual fence map at claim time (refreshed)

Every surface the remaining work would touch is live-claimed:

- `target-operations-to-selected-instructions/src/selection/construction`
  (where a DynamicParameterCall selection arm lands) —
  CALLBACK-PRIVATE-MATERIALIZATION (Zergling-55), exp ~09:13Z Sep 21;
  the same claim fences `machine_code/calls` (stated path
  `backend/machine-code/...` — the crate now lives under
  `representations/machine-code/`).
- `target-operations-to-selected-instructions/src/legalization` wholesale —
  X86-FMA-PROVIDER-TRANSPORT (Jarod/swarm-w9), exp ~09:07Z.
- `isa-aarch64/src` wholesale — NATIVE-WRAPPER-ENCODING-AARCH64
  (claude-opus-goal), exp ~12:04Z.
- `isa-x86_64/src/lib.rs` + `semantic_unit_wrapper_encoding.rs` —
  UEFI-PHYSICAL-SEMANTIC-ENTRY (z88), exp ~08:44Z (partial coverage; the ISA
  leg needs both ISAs regardless).
- `image-emission/src/lib.rs` + `hosted_receiver{,.rs}` + Cargo.toml + its
  artifact tests — PLAN-LAID-VIEWS (zergling-z27), exp ~09:25Z.
- `selected-instructions-to-register-homes/src/unsequenced_spill_stages` —
  POC-SPILL-FAMILY-SEQUENCING (Jarod/swarm-w9), exp ~06:53Z (post-allocation
  territory).
- Expired since the prior map: SIGNED-CALL-PREMISES on selection/validation,
  WIRE-RUNTIME-AND-INSTALLATION on image-emission wholesale,
  PHYSICAL-ACCESS-PROFILES on `native-artifact/src/physical`,
  BASELINE-SERVICE-CARRIER-FAILURES on the dynamic_composed_unit tests.

The row's own constraint still applies: "the dependency is an ordinary
indirect-call operand… not another whole-body recognizer. Do not resume
target-only descriptor composition: two such milestones left this native
customer unsupported." The selection-stage arm — the gate every later leg
depends on — sits inside the live selection/construction fence, so no
unfenced genuinely-implementable slice exists under this claim surface even
though `native-artifact` is currently unclaimed.
