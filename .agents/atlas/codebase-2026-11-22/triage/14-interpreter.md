# Psi interpreter findings

These are partial dispositions for [area 14](../digests/14-interpreter.md).

| Finding | Current evidence | Remaining question |
| --- | --- | --- |
| Section 5 item 14 and section 6 describe the same RFLAGS write. | The instruction executor advances if its input is missing or is not an unsigned integer. The portable artifact's `validate_register_access` requires a declared operand of the exact unsigned carrier in `foundation_validation.rs`. The assembly catalog's `SystemWrite` input carrier is U64. | A malformed declared operand is rejected at wire validation. Trace which in-memory execution routes can supply a wrong value despite this check before claiming a runnable program reproduces it. |
| Section 5 item 8 and section 6 describe the same continuation handoff. | Checked continuation now refuses a call observer or issued layout target on either frame. Replacing the receiving frame previously discarded its observation request. Keeping the parked frame also carried its issued target into the successor. | The fixture records an integer argument on uninterrupted execution and refuses the same selector on the receiving frame. Layout-target fixtures test both directions and the ordinary unconfigured continuation still completes. |
| Section 5 item 1. A readiness poll defaults to ready. | The digest traces completion separately through the effect handler and `commit_boundary_result`. | The readiness default alone does not demonstrate acceptance of an invalid result. Any proposed defect needs a failing completion case. |
| Host services, digest line 201. The Windows descriptor adapter accepted arbitrary translation modes. | The adapter now returns -1 without changing the descriptor for invalid modes. It reports wide text as the previous mode for all three Unicode selections. Two regression tests cover both state transitions, including independence between descriptors. The installed Windows msvcrt.dll supplied the comparison. | This covers standard descriptor mode state. The public native fixture did not complete successfully on the host. |

The remaining engine, embedding, memory and host-service candidates still need review.

## Original findings still under review

Sections 5 and 6 contain 59 top-level observations. The linked ranges cover each observation once in this table. Individual dispositions above apply only to their stated conditions.

| Subject | Digest lines | Evidence and open work |
| --- | --- | --- |
| Entry checks and continuation | [154–166](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/14-interpreter.md#L154-L166) | 154 and 161 are investigated above. Production Build resumption calls ordinary instantiate; compiler evaluations install observers through instantiate_compiler_evaluation. The embedding regressions above cover configured receivers. |
| Instruction execution | [169–176](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/14-interpreter.md#L169-L176) | 169 and 218 describe the same RFLAGS operand. Check the other instructions at admission and execution. |
| Memory and external custody | [179–194](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/14-interpreter.md#L179-L194) | Trace accepted values, deferred checks, write failures and their callers individually. |
| Host services and worker | [197–207](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/14-interpreter.md#L197-L207) | Compare each modeled service with its actual host or declared adapter contract. |
| Drops and deferred checks | [212–222](https://github.com/CathedralOS/Omega/blob/1ab250720c8cecccb704d7da49b5889af13138f8/.agents/atlas/codebase-2026-11-22/digests/14-interpreter.md#L212-L222) | 214 repeats the receiver-frame observation, and 218 repeats RFLAGS. Test the other observation, projection and cleanup paths against their callers. |
