# Private collection bounds

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).
The [invocation fact](../README.md) reaches the existing collection-coordinate
consumer through the same closed incoming frames as arithmetic formation:

> Every private arrival carries index < items.len, so the helper can read items[index].

Each program guards the exact descriptor/index pair before invoking a private
helper. Its body indexes directly, with no duplicate authored requirement or
caller result guard establishing the fact after the call. A twin changes only
`<` to `<=`, admitting the end position as an invalid array index.

| Place | Program | Twin |
| --- | --- | --- |
| local | [positive](local/main.omg) | [reject](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [reject](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| call_result | [positive](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| match_join | [positive](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| const_name | [positive](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| other_state | [positive](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| other_machine | [positive](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| target_body | [positive](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

Optimized baseline `061b60ff113a256a28fb450fd1bcf9b8b27a6bde` refuses all nine
positives and all nine twins at the helper's index. Repaired producer
`40c6fdc57750f9d3c23ac0d754c9ad4fcdb7e194` executes every positive with
native/Terminal exit-zero agreement on macOS ARM64. All twins still reject,
as do controls for a [changed index](controls/changed_index/main.omg),
a [different collection](controls/other_collection/main.omg),
an [unbounded second arrival](controls/alternate_arrival/main.omg), and a
[minimum extent](controls/minimum_extent/main.omg) that is insufficient for
the requested index. Identical sources are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_private_bounds_*`.

The runtime oracle observes two distinct selected bytes, a boundary fallback
and an empty view. The match place returns different values on its two arms.
An inverted guard, wrong index or fallback replacing an accepted read traps.
Every positive authors the four hosted entry bindings; other-host native
execution remains unverified.

Requirement evidence:

- Establishment: the prior guard establishes the exact collection/index bound;
  local failure is retried through independently checked closed arrivals.
- Identity and lifetime: captured descriptors and scalar indices remain paired.
  Replacing the index or substituting another descriptor supplies no bound.
- Composition: local values, record fields, parameters, completed scalar calls,
  match joins, constants and deeper/nonnested/target bodies transport the fact.
  All feasible or unresolved arrivals participate; lexical ancestry grants none.
- Soundness: builtin-coordinate selection remains mandatory. Minimum length is
  not exact extent; unknown origins or incomplete coverage remain conservative.
- Psi 05–07: actual Terminal execution preserves guarded selection, view extents,
  explicit borrows, helper calls and byte reads through ordinary lowering.
- Omega 00–02: existing view descriptors and calling plans carry the same
  explicit operands. This does not establish another ABI placement lane.
- Omega 03–06: native execution checks both byte results and both fallback paths.
  The acyclic row does not qualify cyclic custody or a new stack bound.
- Omega 07–09/emission: direct macOS ARM64 images execute with Terminal agreement;
  authored result checks distinguish incorrect read/branch outcomes.

The std admission replay removes 17 of its 39 diagnostics. Remaining refusals
in cyclic bounds, effectful helper completion and joint cycle checking still
prevent the product frontend gate. The POSIX namespace and hosted call-marker
source migration in this series does not claim full std or parent closure.
Producer-bound run records and raw diagnostics remain in qualification evidence.
