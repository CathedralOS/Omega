# PoC orphan-entrance audit

Status: point-in-time inventory of the parked proof-of-concept spill boundaries
under `omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/unsequenced_spill_stages/`,
recorded at revision `61e01c1d66`. This audit fixes the entrance inventory and
its caller graph; it authorizes no sequencing and no deletion — disposition
belongs to POC-SPILL-FAMILY-SEQUENCING and the allocation owners.

Affected subject: executable spill recovery in
[register allocation](../../omega-rust/omega/pipeline/selected-instructions-to-register-homes/src/register_allocation.rs).

## Finding 1 — zero production entrances

`stage_register_allocation` routes exclusively through
`assignment::{transformed,recovery,baseline,runtime_spill}`: replay of the
selected X-to-X evidence, the admitted recovery catalog (`SharedEntryFixedView`,
`ActiveResidentImmediateU64Rematerialization`), the fixed/precolored composing
legs, and `assignment::runtime_spill` on `NoCompatibleHome` pressure. It calls
none of the 18 parked families — matching the park's own module contract:
"Sequencing one means calling it from `stage_register_allocation`, not
re-declaring it beside the live stages."

Outside the park each entrance is reachable only from:

- `src/lib.rs` — the crate's re-export surface;
- `tests/architecture/optimizer_source_organization/entrances/requirements/executable/selection_allocation.rs` —
  the architecture inventory pinning them as stage requirements;
- `tests/native-differential/tests/pipeline_ownership/stages/allocation/register_allocation/*` —
  the native-differential replay fixtures (44 entries in that stage dir; every
  parked family has a named fixture, and the original-tranche fixtures
  `original_spill_recovery_actions.rs`, `original_recursive_spill_insertion.rs`
  and `guarded_original_spill_recovery_choice.rs` pin the recursive legs).

No executable route produces any of these facts.

## Finding 2 — entrance inventory

| Family | Entrance | Boundary |
| --- | --- | --- |
| `abstract_spill_access_constraints` | `constrain_abstract_spill_accesses` | Orders compiler-private abstract accesses; records data, declared-barrier and overlapping-slice dependencies. No executable op/address/frame. |
| `abstract_spill_insertion` | `schedule_abstract_spill_insertion` | Join: logical spill actions + slot colors → one store/reload/rewrite schedule; stops before reload homes and machine ops. |
| `abstract_spill_memory_effects` | `derive_abstract_spill_memory_effects` | Rows describing reads/writes of compiler-private abstract spill storage; no executable memory effect. |
| `logical_spill_operations` | `plan_logical_spill_operations` | Logical spill planning over `ValidatedLiveRanges`/`ValidatedSpillChoices` + independent replay. |
| `stack_slot_coloring` | `color_logical_spill_stack_slots` | Canonical logical-spill slot coloring over `ValidatedLogicalSpillOperations` + replay. |
| `reload_value_homes` | `assign_reload_value_homes` | One bounded block-local reload → physical view; **dual role** (see edge below). |
| `synthetic_reload_values` | `bind_synthetic_reload_values` | Deterministic synthetic identity per validated reload. |
| `spill_recovery_worklist` | `seed_spill_recovery_worklist` | Epoch-one work identity from reproduced reload pressure; no victim choice. |
| `spill_recovery_choice` | `choose_spill_recovery_victims` | Names one original resident whose removal recovers a reload candidate; does not authorize removal. |
| `spill_recovery_actions` | `plan_spill_recovery_actions` | Victim choices → target-neutral store/reload/rewrite obligations. |
| `recursive_spill_insertion` | `schedule_recursive_spill_insertion` | Extends the epoch-zero/one schedule with epoch-two recovery obligations + recolors the abstract slot set. |
| `recursive_reload_value_homes` | `assign_recursive_reload_value_homes` | Closes every logical reload segment in a recursive schedule with a replayable physical-view assignment. |
| `spill_pseudo_instructions` | `lower_recursive_spill_pseudos` | Recursive schedule → named compiler-private store/reload pseudos + operand rewrites. |
| `generalized_spill_recovery_worklist` | `seed_generalized_spill_recovery_worklist` | Retained generalized reload pressure → one work identity. |
| `generalized_spill_recovery_choice` | `choose_generalized_spill_recovery_victims` | Retained blocker roster → one compiler-private victim value. |
| `generalized_spill_recovery_actions` | `plan_generalized_spill_recovery_actions`, `plan_generalized_original_spill_recovery_actions` | **Two entrances on one family** — the generalized boundary and its original-tranche replay share the module. |
| `generalized_spill_insertion` | `schedule_generalized_spill_insertion` | Recolors first-spill insertion + second-spill logical actions → one target-neutral event schedule. |
| `generalized_reload_value_homes` | `assign_generalized_reload_value_homes` | Replays allocation after generalized abstract scheduling; assigns physical views to the two logical reload actions. |

## Finding 3 — internal edges inside the park

The families are mostly disjoint at their entrances; two structural couplings
matter for any sequencing order:

- `spill_recovery_worklist::{compute,replay}` calls
  `reload_value_homes::{compute,replay}` — `reload_value_homes` is both a
  standalone entrance and the subordinate proof the worklist seeds from, so it
  must sequence before or with its consumer.
- `generalized_spill_recovery_actions` carries two public entrances (the
  generalized join plus the original-tranche replay), matching the
  `guarded_original_*` fixtures: the generalized legs re-check original
  evidence rather than replacing it.

## Finding 4 — one downstream acceptance, still not an entrance

`backend/machine-emission/src/frame_layout/spill_requirements/` consumes
`ValidatedAbstractSpillAccessConstraints` under
`NonAuthoritativeSpillFrameRequirementPolicy::AbstractSpillAreaAndPreservationConventionV1`
— it replays the constraint output into frame-requirement custody and seals it
in the plan identity. That is acceptance of an already-produced fact, not a
producer: nothing on the executable path creates the constraints.

## Disposition boundary

The audit supports the park's own framing: every family is validated,
independently replayed, pinned by architecture + native-differential coverage,
and deliberately unsequenced. Sequencing any family means wiring it into
`stage_register_allocation` beside `assignment::runtime_spill` — the
executable pressure-recovery route — under POC-SPILL-FAMILY-SEQUENCING.
Deleting the park removes retained validation substrate and the 44-entry test
surface; the row owns that tradeoff. No entrance is ambiguous about ownership:
all 18 are `pub(crate)` inside this crate and leave it only through the test
re-exports above.
