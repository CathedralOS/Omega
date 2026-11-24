# Psi interpreter findings

These are partial dispositions for [area 14](../digests/14-interpreter.md).

| Finding | Current evidence | Remaining question |
| --- | --- | --- |
| Section 5 item 14 and section 6 describe the same RFLAGS write. | The wire validator checks the declared U64 operand. Execution now also reads the register image through the existing typed unsigned operand reader. Missing, noninteger, wrong-width and overflowing runtime values stop the operation before it changes the flags. | A direct interpreter-frame fixture completed normally with no operand before the fix. It now refuses each malformed frame, preserves flags on error, and sets or clears the carry bit from a valid U64 while retaining the interrupt-enable bit. These tests exercise instruction execution with controlled frame values. |
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

## Findings traced to their consumers

| Digest lines | Disposition and evidence |
| --- | --- |
| 160, 213 | Build's source invocation matches a normal completion and projects its value before calling `take_call_observations`. Any other status returns an error. On completion, `invoke_to` first charges result custody through `CallObservations::finish`. The call observation fixture checks the empty drain before execution, actual records on completion, an empty drain after taking them, and an empty drain following a crash. The drain alone does not encode the status. `close` drops the execution frame and its established erased fields; the instance separately exposes `is_closed`. |
| 165 | Entry-result observations name operations in one entry activation. The test `entry_results_ignore_recursive_activation_and_capture_restored_caller` covers the identical operation in a recursive activation and after the caller resumes. `entry_results_missing_occurrence_cannot_use_final_return` checks an absent requested occurrence. |
| 176 | Re-establishing a byte literal in its own activation reuses its existing storage only after checking its allocated identity, structural type, root path, qualifiers and immutable bytes. The instruction test `repeated_literals_keep_their_home_but_nested_calls_get_distinct_storage` distinguishes repetition from a new activation. |
| 188 | An `AsciiOnly` domain constrains the **new** octet through `primitive_store_restriction` and the interpreter's `satisfies_field` check, with an upper bound of 127. The separate `ValidUtf8` backing scan checks the **existing** bytes. The L6 fixture accepts an ASCII buffer update and refuses replacing an ASCII byte with a lone continuation. Changing only the initial two bytes to the UTF-8 for é in the same 28-byte source buffer caused the source checker to refuse even the unchanged ASCII write at position 13. The interpreter's handling of UTF-8-only writes and the source proof limit remain open. |
| 197 | `admit_c_word_record` independently checks the native result extent and alignment, the exact field count, each field offset against its ordinal times eight, and the leaf class, size and alignment. An absent authored storage row alone cannot admit the original example with an incorrect **native** offset. |
| 221 | The extent owner test `borrowed_mapping_retains_source_loan_and_shared_polarity` follows a shared mapping through activation and unmapping. It verifies that the destination is returned and that the borrowed source is never yielded as owned. The owned mapping has a separate two-authority round-trip test. |
