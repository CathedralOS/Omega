# Cleanup of a returned exclusive counter

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).

Fact: A finite-family result retains its exclusive input counter through storage and forwarding, and consuming that result increments the counter once.

Each positive calls `First` at width 16 and `Second` at width 32, stores and
forwards the returned ticket, then returns from `finish`. The ticket retains an
exclusive borrow of the original counter. Its data declaration selects
`drop_token(value: &mut Token)` as the cleanup hook. The hook increments the
counter; `finish` has no explicit increment or disposal call. The program checks
one increment after the first result and two after the second. Missing,
repeated or misdirected cleanup fails the exit-zero check.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| Selected target code | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |


Each twin changes only the produced token’s backing to an invocation-local
array. Returning it rejects with `does not outlive the call`. All Main and Build
sources match their copies under
`tests/omega/{pass,fail}/facts/finite_family_exclusive_result/`. All positives
bind the four hosted targets. The match example executes both boolean inputs
and checks distinct tags 17 and 29 against those inputs. Inverting its selection
or always taking the fallback traps. The target example also distinguishes
x86-64 tag 17 from ARM64 tag 29.

## What the repair preserves

The independent target audit previously confused the bytes holding a stored
pointer with the storage that pointer addresses. Reading the pointer now uses
the original record’s read permission. Updating the counter still requires the
pointer’s separate exclusive permission. A real loan of the pointer bytes
continues to block an overlapping read, even though its type is a reference.
Unknown origins, lifetime obligations, live children, writes and disposal keep
their ordinary checks. The repair uses no cleanup-specific Source pattern.

The separate [two-pointer record](../../../../tests/omega/pass/borrows/borrowed_record_descriptor_carriers/main.omg)
checks two updates to each pointed-to array, distinct results 17 and 39, and six
surrounding values. It uses neither nominal cleanup nor a family call. Existing
negative controls cover exclusive access, active children and write-only borrows.
The returned-parent control now forms its child directly from the returned
reference; the same mutation reaches its intended active-child diagnostic.
Its previous extra projected-return call stopped at unrelated result formation.
That original refusal is retained and is not counted as child-borrow evidence.

The governing contracts are [ownership and cleanup](../../../../wiki/spec/language/ownership.md#consumers-and-cleanup),
[nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md),
[loans](../../../../wiki/spec/terminal-psi/loans.md) and
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views).

| Requirement | Evidence in this row |
| --- | --- |
| Establishment | The data declaration selects the independent cleanup hook; each call starts with the lawful caller-owned counter. |
| Identity and lifetime | The counter is observed after each result dies. Local-backing twins reject the escaping borrow. |
| Composition | Storage, forwarding and each named place retain that same counter and its exclusive loan. Both match inputs and target-specific helpers are observable. |
| Ownership and proof | Cleanup lends the valid token to its selected hook, consumes it once and restores caller access. Proof grants no extra storage authority. |
| Checked-to-Terminal Psi | Terminal execution observes the normal-return cleanup effect. General proof-erasure and crash-route requirements retain their own acceptance. |
| Build, layout and calls | Four hosted entries execute the same counter contract; the two-pointer control checks independent fields and neighboring storage. |
| Instructions and frames | Repeated calls preserve the counter, pointer fields and markers. Explicit activation bounds and unrelated instruction families are outside this row. |
| Native images | Native and Terminal Psi produce the authored result on all four matching hosts. Other image, relocation and branch-encoding requirements retain their own witnesses. |

## Measured results

| Matching host | Run | Actual compiler and program source |
| --- | --- | --- |
| Linux x86-64 | [41951791438](https://github.com/CathedralOS/Omega/actions/runs/41951791438) | `ed656648a42a` |
| Linux ARM64 | [41952351050](https://github.com/CathedralOS/Omega/actions/runs/41952351050) | `ed656648a42a` |
| macOS ARM64 | [41953420693](https://github.com/CathedralOS/Omega/actions/runs/41953420693) | `46cad3817d2f` |
| Windows x86-64 | [41954199920](https://github.com/CathedralOS/Omega/actions/runs/41954199920) | `46cad3817d2f` |

Every host records ten actual native/Terminal exit-zero agreements, nine sample
exit-zero pairs, nine intended lifetime rejections and the focused ordinary CLI
agreement. Mac and Windows also pass all 25 Source cases. The Linux reports
remain failed because they contain the older, formation-masked ownership
control. Its revised input passes the intended rejection check on Mac and
Windows. The seven compiler, library, tool and configuration objects are
identical across these sources; all positive programs and lifetime twins are
also identical. Each result retains its actual source and producer identity.

The Source record replaces only twenty changed fixture rows. Mac, ARM and
Windows combined records copy their actual ten execution rows; unrelated bytes
remain unchanged. Linux combined execution stays in its original report rather
than being projected into a native-only record or a new corpus file. Profiles,
324 frozen inputs per host, raw reports, exact source archives and independent
reviews are retained with [PR #1873](https://github.com/CathedralOS/Omega/pull/1873).

The frozen baseline uses program `91805a009c1d` and verified compiler
`538cc2df6785`: all nine ordinary programs already execute at 0/0, while the
independent target audit wrongly refuses all nine. Two local compiler builds
later exhausted disk space; both failures remain recorded. Nineteen earlier
Rust tests apply to the unchanged compiler range `e625011af644` through
`6a48b27b468b`, before the final pointer-storage guard correction.

This completes the returned-counter cleanup requirement. Other finite-family
requirements remain open, including the documented remaining host evidence for
[inline array results](../finite_family_static_inline_result/README.md).
