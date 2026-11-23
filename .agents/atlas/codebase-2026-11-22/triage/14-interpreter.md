# Psi interpreter findings

These are partial dispositions for [area 14](../digests/14-interpreter.md).

| Finding | Current evidence | Remaining question |
| --- | --- | --- |
| Section 5 item 14 and section 6 describe the same RFLAGS write. | The instruction executor advances if its input is missing or is not an unsigned integer. The portable artifact's `validate_register_access` requires a declared operand of the exact unsigned carrier in `foundation_validation.rs`. The assembly catalog's `SystemWrite` input carrier is U64. | A malformed declared operand is rejected at wire validation. Trace which in-memory execution routes can supply a wrong value despite this check before claiming a runnable program reproduces it. |
| Section 5 item 8 and section 6 describe the same continuation handoff. | `continue_checked_entry` checks the parked frame's call observations, then replaces the receiving execution frame. `install_compiler_observations` can put a call observer and an issued layout target on the receiver. | Exercise the configured receiver alongside an ordinary continuation. Establish which observations must be carried or refused before replacing either frame. |
| Section 5 item 1. A readiness poll defaults to ready. | The digest traces completion separately through the effect handler and `commit_boundary_result`. | The readiness default alone does not demonstrate acceptance of an invalid result. Any proposed defect needs a failing completion case. |

The remaining engine, embedding, memory and host-service candidates still need review.
