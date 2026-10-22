# Copied call operands keep their evaluation epoch

A copied first argument equal to 7 stays 7 when the second argument overwrites its former home with 9.

Parent: **STATE-LOCAL-VALUE-FRONTIER**; prerequisite evidence for **CHECKED-PACKAGE-REUSE**. The first argument completes as 7, then `overwrite` stores 9 through the original home. The caller must establish `receive`'s explicit `requires first == 7` from that completed copy. `receive` transports its declared premise through `seven`'s entry contract, establishes its result as 7, and permits the caller's exact subtraction `observed - 7`. A runtime guard does not establish that fact.

The nine places are [`local`](local/main.omg), [`field`](field/main.omg), [`parameter`](parameter/main.omg), [`call_result`](call_result/main.omg), [`match_join`](match_join/main.omg), [`const_name`](const_name/main.omg), [`other_state`](other_state/main.omg), [`other_machine`](other_machine/main.omg), [`target_body`](target_body/main.omg). Every place has an `invalid/main.omg` twin changing only the initial first operand from 7 to 9. It must refuse the same 7 obligation. Matching pins live under `tests/omega/{pass,fail}/facts/copied_call_argument_snapshot_through_*`.

The original baseline `55898d082fc8` reread the first operand's home after later operands ran. The producer already retained exact `FlowOperandFact` completion constraints, and call admission must consume that capture; live views instead consult their exact invocation-time referents. The by-value entry-contract correction in `9ac3908938b6` also requires each `receive` body to declare its premise: a caller's literal cannot justify an unrestricted callee body. Positive and false-input twins share that same contract; only the initial first operand differs. A conservative unknown-input fallback must never re-establish a fact from a predecessor subset.

## Requirements and evidence

- Establishment: initial 7 and the completed operand capture establish the fact; checked call/arrival contracts carry it, never the later 9 or an unproved goal.
- Identity and lifetime: the copied first operand is detached; `overwrite` changes its original home, not that copy.
- Composition: all nine places consume the same snapshot through `seven`, checked result equality and exact subtraction. Twins change the source to 9, not a caller guard.
- Soundness: ordinary exclusive access authorizes the overwrite; operand facts grant no loan or disposal authority. Unknown/missing captures participate conservatively.
- Checked-to-Terminal Psi and native realization: every positive binds all four hosted targets and declares exit 0. Acceptance requires observed native exit 0 and interpreter exit 0 through `omega run --both`; flags alone establish no other-host execution.
- Remaining scope: this row proves one scalar operand-epoch mechanism. Case/view composition and the rest of both parent outcomes remain open.

Actual baseline diagnostics, producer/source/oracle identities, engine observations and reject reasons belong in the owning qualification/claim records. No unexecuted target is claimed.
