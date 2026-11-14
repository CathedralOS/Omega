# Cleanup of a returned Linear token

A finite generic call returns a case containing a Linear token that borrows
the caller's counter exclusively. Storing, forwarding and consuming that token
must increment the same counter exactly once. The active case keeps the token's
mandatory cleanup obligation through every transfer.

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).

Fact: A finite-family result retains its exclusive input counter through storage and forwarding, and consuming that result increments the counter once.

`First` at width 16 returns `Narrow`; `Second` at width 32 returns `Wide`.
Each call puts its result in a record, forwards it and consumes its active
payload. The ticket's `Token` selects `drop_token` as its cleanup machine.
That machine increments the counter. The consumer has no explicit increment
or disposal call: it checks the case and width, then returns. The caller
checks one increment after the first call and two after the second. Missing,
repeated or misdirected cleanup traps.

Both alternatives explicitly carry the input counter's lifetime. The trait
has independent input and output data parameters, and each named
implementation captures that lifetime. A twin changes only the returned
token's backing to a local array. Returning that reference must reject with
`does not outlive the call`; no caller guard makes the return lawful.

The place names describe how each program receives its input counter. Every
returned case follows the common storage, forwarding and consumption path;
this row does not claim nine different places for storing the returned case.
The constant example copies an owned array into runtime storage before
borrowing it. The match example checks both inputs and distinguishes tags
17 and 29. The target example distinguishes the x86-64 and ARM64 helpers.
All Main and Build sources match their copies under
`tests/omega/{pass,fail}/facts/finite_family_exclusive_result/`.
All positives bind the four hosted targets and require exit 0.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [invalid](invalid_local/main.omg) |
| Field | [program](field/main.omg) | [invalid](invalid_field/main.omg) |
| Parameter | [program](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| Call result | [program](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| Match join | [program](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| Const name | [program](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| Nested machine | [program](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| Other machine | [program](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| Selected target code | [program](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

## Contracts and coverage

The governing rules are [ownership and cleanup](../../../../wiki/spec/language/ownership.md#consumers-and-cleanup),
[nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup),
[returned views](../../../../wiki/spec/language/lifetimes.md#returned-views) and
[finite method families](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md#finite-generic-method-families).

| Requirement | What the programs check |
| --- | --- |
| Establishment | A lawful caller-owned counter enters the selected call. The data declaration selects its cleanup machine. |
| Identity and lifetime | The caller observes that same counter after each ticket dies. Local-backing twins reject an escaping reference. |
| Composition | Both selected rows retain the exclusive counter through case construction, record storage, forwarding and payload consumption. |
| Ownership and proof | Cleanup lends the valid token to its selected machine, consumes it once and restores caller access. Proof grants no extra authority. |
| Checked-to-Terminal Psi | Interpreter execution observes the cleanup effect on normal return. Proof erasure and crash-route obligations keep their own acceptance. |
| Build, layout and calls | Four hosted entries carry the case, pointer and marker through repeated calls. Other calling conventions retain their own witnesses. |
| Instructions and frames | The selected counter, case and marker survive storage and calls. This row does not establish explicit activation bounds or unrelated instructions. |
| Native images | Native and Terminal Psi agree with the authored exit on each matching host. Other relocation and encoding requirements remain separate. |

## Linear result

The only program change from the earlier affine case is declaring `Token`
`[linear]`. The operation, widths, case payloads, counter and expected exit stay
the same in every place and its pinned copy. The lifetime twin still changes
only the returned reference's backing.

With compiler `948f19e774da`, the valid local program wrongly refused the call
that forwards the stored ticket. The checker demanded replacement authority
over the plain integer counter. Its lifetime twin stopped at that same wrong
error, so it did not establish the intended lifetime check.
[The baseline run](https://github.com/CathedralOS/Omega/actions/runs/42014661806)
retains both refusals before the repair.

The repair asks which writable referents can require ownership restoration.
The existing cleanup check may prove a referent has no owned cleanup obligation;
only then can this query omit it. Unknown referents retain the original check.
Ordinary mutation checks remain complete, and expiring retained loans are always
included. This introduces no special handling for finite families or cases.

[Focused checking](https://github.com/CathedralOS/Omega/actions/runs/42023922300)
with optimized compiler `2cf8f750ade0` accepts the local program and the existing
borrowed-claim control. All six rejection controls reach their intended causes,
including the lifetime twin, missing borrowed-storage repair, retained loans,
undisposed Linear ownership and observing write-only storage.

[The full Linux run](https://github.com/CathedralOS/Omega/actions/runs/42025261656)
reuses that prepared compiler on program source `9df129e92410`, without rebuilding.
All nine positives check, all nine twins report the six expected lifetime errors,
all nine corpus and nine sample executions agree at exit 0, and all nine invalid
samples reject. The local ordinary CLI also updates and runs at 0/0.

The same complete scope passes on every matching hosted target:

| Host | Run | Actual compiler | Program source |
| --- | --- | --- | --- |
| Linux x86-64 | [42025261656](https://github.com/CathedralOS/Omega/actions/runs/42025261656) | `2cf8f750ade0` | `9df129e92410` |
| Linux ARM64 | [42026333825](https://github.com/CathedralOS/Omega/actions/runs/42026333825) | `9df129e92410` | `9df129e92410` |
| macOS ARM64 | [42026338394](https://github.com/CathedralOS/Omega/actions/runs/42026338394) | `9df129e92410` | `9df129e92410` |
| Windows x86-64 | [42026343121](https://github.com/CathedralOS/Omega/actions/runs/42026343121) | `9df129e92410` | `9df129e92410` |

Each host supplies nine Source accepts, nine intended lifetime refusals, nine
corpus and nine sample native/Terminal exit-zero agreements, nine invalid sample
refusals and an ordinary CLI update followed by both engines at 0/0. The compiler
inputs are identical between `2cf8f750ade0` and `9df129e92410`; the latter adds the
eight remaining places' Linear declarations and their mirrors. All compilers
have verified optimization level 1. The Linux replay does not become a `9df`
compiler execution. Historical milliseconds remain when the recorded outcome is
unchanged; actual fresh timings stay in the raw reports.

These are nine ways to receive the input counter, with the common returned-case
path described above. They do not establish nine different storage forms for
the returned case. The broader parent task remains open.

## Earlier affine case results

| Matching host | Run | Actual compiler | Program source |
| --- | --- | --- | --- |
| Linux x86-64 | [41993126604](https://github.com/CathedralOS/Omega/actions/runs/41993126604) | `4264bc537ced` | `4264bc537ced` |
| Linux ARM64 | [41990133234](https://github.com/CathedralOS/Omega/actions/runs/41990133234) | `555bdbb56368` | `e2e2bad96355` |
| macOS ARM64 | [41990141523](https://github.com/CathedralOS/Omega/actions/runs/41990141523) | `555bdbb56368` | `e2e2bad96355` |
| Windows x86-64 | [41990149995](https://github.com/CathedralOS/Omega/actions/runs/41990149995) | `555bdbb56368` | `e2e2bad96355` |

Together, each host supplies nine Source accepts, nine intended lifetime
refusals, nine corpus native/Terminal exit-zero agreements and eighteen sample
outcomes. The local ordinary CLI also updates and runs at 0/0 on each host.
The original Linux local and eight-place runs remain at
[41985245723](https://github.com/CathedralOS/Omega/actions/runs/41985245723) and
[41990718557](https://github.com/CathedralOS/Omega/actions/runs/41990718557). A
real observation-file merge conflict required integrating published main, which
changed common checking code. The fresh Linux compiler then passed the entire
cleanup scope. Its whole job remains failed because two additional, unchanged
finite-family controls timed out at their existing adaptive limits. No timeout
is recorded as an intended rejection. All compilers have verified optimization
level 1. Only the integrated Linux compiler needed a fresh Rust build.

The three `555bdbb56368` compilers retain that actual identity. Their only
compiler difference from `f37c9cfc83e2` consolidates two public-contract conflict
checks during target-neutral lowering. Independent review establishes identical
catalog output on admitted paths; these are not executions of `f37` on those
three hosts. The later integration changes common proof/checking consumers,
which the fresh Linux scope covers, and fixed-byte construction absent from
these cleanup values. Independent review found no changed target-specific
consumer requiring another three builds. Original other-host identities and
outcomes remain intact; no integrated-compiler execution is claimed there.

Two earlier attempts remain failed: the first local source omitted the lifetime
required by its two case paths, and the first eight-place Linux observer passed
an empty ordinary selection to a helper that requires a nonempty list. Neither
attempt supplies execution acceptance. Their corrected runs are listed above.

## Earlier plain-ticket repair

[PR #1873](https://github.com/CathedralOS/Omega/pull/1873) qualified the original
plain-ticket row. Its pointer-storage repair distinguished reading the bytes
holding a pointer from updating the storage that pointer addresses. Reading
uses the record's permission; updating the counter still needs its separate
exclusive permission. Loans of the pointer bytes continue to block overlapping
reads. The repair added no cleanup-specific source exception.

Those older runs used `ed656648a42a` on Linux and `46cad3817d2f` on macOS and
Windows. They remain evidence for their original plain-ticket programs, with
raw reports, baseline failures and independent reviews preserved on that PR.
The new results above establish the active-payload extension. The parent task remains open.

## Restored contract checks

The two older controls now reach their expected errors with optimized compiler
`7ae329e5c101`, [run 42005565919](https://github.com/CathedralOS/Omega/actions/runs/42005565919).
The implementation that accepts fewer widths than its public requirement rejects
in 111343 ms; the call using a width outside the family rejects in 134342 ms.
Both finish within their unchanged adaptive limits of 167289 and 199035 ms.
The existing write-only-array observation and stale copied-argument controls
also reject for their expected reasons. Profiling is off.

The checker now prepares stored-field observations once within each immutable
read and compares expression children directly. It preserves storage identity,
observation order, conflicting-value rejection and access checks.
[Run 42009429411](https://github.com/CathedralOS/Omega/actions/runs/42009429411)
reuses that exact compiler without rebuilding: all nine positives check, all
nine twins reject for their lifetime error, all nine corpus executions and nine
sample executions agree at exit 0, and all nine invalid samples reject. The
local ordinary CLI also updates and runs with both engines at 0/0. These are
Linux x86-64 observations; the earlier three-host results retain their original
compiler identities. The failed baseline runs above remain failed.
