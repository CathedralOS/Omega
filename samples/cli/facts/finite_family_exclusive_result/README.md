# Cleanup of a returned exclusive counter

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../TASKS.md).

Fact: A finite-family result retains its exclusive input counter through storage and forwarding, and consuming that result increments the counter once.

Each program selects both providers at widths 16 and 32, stores and forwards the
returned ticket, then returns from `finish`. The ticket retains an exclusive
borrow of the original counter. Its data declaration selects the independent
`drop_token(value: &mut Token)` cleanup hook. That hook increments the counter;
`finish` has no explicit increment or disposal call. The program observes one
increment after the first result and two after the second. A missing, repeated
or misdirected cleanup fails its exit-zero check.

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

Each twin changes only the produced token's backing to an invocation-local
array. Returning it must reject with `does not outlive the call`. Main and Build
sources match their copies under
`tests/omega/{pass,fail}/facts/finite_family_exclusive_result/`. All positives
bind the four hosted targets. The match example constructs tags 17 and 29 on
distinct arms and checks the selected tag against its boolean input after cleanup. It executes both arms;
inverting the selection or always taking the fallback traps. The selected
target example also checks tags 17 for x86-64 and 29 for ARM64, so choosing the
wrong helper traps.

## Requirements

The selected return must transfer the original exclusive pointer and its parent
loans exactly once. Storing it, crossing a call or state transition, and reading
its marker must preserve that custody. A local backing cannot escape through the
same operation.

The data owner's default domain selects one nominal cleanup hook. Cleanup
begins consumption before lending the whole valid token to that hook. Reading
the token's stored pointer uses the hook's original record borrow; modifying
the pointed-to counter still uses the separate exclusive loan. Lifetime debts,
live children, write-only access, writes and disposal retain their ordinary
checks. An unknown origin or inline-looking state alias grants no authority.

These obligations follow [ownership and cleanup](../../../../wiki/spec/language/ownership.md#consumers-and-cleanup),
[nominal cleanup](../../../../wiki/spec/terminal-psi/ownership.md#nominal-cleanup),
[structural access](../../../../wiki/spec/terminal-psi/structural_access.md) and
[returned carrier lifetimes](../../../../wiki/spec/language/lifetimes.md#returned-views).
The checked cleanup, Terminal Psi interpretation, target calling/storage plans
and native execution must preserve the same counter and once-only effect.

## Evidence and remaining work

At committed inputs `91805a009c1d`, verified optimized compiler `538cc2df6785`
checks nine positives and rejects nine lifetime twins. All nine ordinary CLI
programs already execute with native/Terminal exits 0; all nine ordinary twins
reject. The corpus's independent target audit instead refuses all nine valid
native cases with `descriptor storage access conflicts with a retained loan`.
The exact sources, producer profiles, raw refusal reports and ordinary engine
observations are preserved in the owning qualification record.

The repair separates a read of the original record's inline pointer storage
from the pointer's captured pointee. Every concrete storage alternative, unknown
origin and lifetime debt still participates in its applicable check. It uses no
cleanup-specific source pattern and grants no extra write or disposal access.
The focused [two-pointer record](../../../../tests/omega/pass/borrows/borrowed_record_descriptor_carriers/main.omg)
checks both updates and six canaries without a cleanup hook or family call.
Existing ownership controls test the restrictions above. The returned-parent
control forms its child directly from the returned reference. Its previous
extra projected-return call rejected during formation, hiding the intended
active-child mutation check. The original refusal is retained, and the revised
control reaches the unchanged child-borrow diagnostic.

Nineteen existing Rust reference tests passed at `e625011af644`. Their compiler
code, library and configuration remain identical through `6a48b27b468b`.
Local compiler preparation then exhausted disk space twice; both failures are
retained. The first hosted Linux run at `6a48b27b468b` checks ten positives,
executes all ten with native/Terminal exits 0, and passes nine sample pairs and
nine reject twins. One older ownership control rejects during unrelated result
formation; its corrected source reaches the intended active-child diagnostic.

Independent review also required retaining real loans of pointer-field storage.
That correction is in `ed656648a42a`; its fresh compiler qualification is
pending. The revised match example and ownership control require their own
current-input observations. Earlier explicit-settler results do not establish
nominal cleanup, and the broader parent task remains open.
