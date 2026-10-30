# Indexed private ranks

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.

The `indexed_value/` production in each of the nine parent places uses
`remaining.values[slot]` as its invocation rank. The helper renames and reorders
both the borrowed record and its scalar selector. Each program selects element
0 and element 1, runs seven iterations from counter 3 to 10, then checks an
empty invocation preserves 10. It checks the selected element is zero, the other
element retains its original amount, and canary 641 is unchanged. The field
place also checks its surrounding record's prefix 207.

Each place's `invalid/main.omg` changes only the decrement from `- 1` to `- 0`.
It must reject the cycle's missing strict edge. The row and twins are pinned as
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_indexed_value_*`.

- Establishment: each member independently supplies `selector < 2` before its
  indexed rank can be formed. The helper additionally supplies positive payload
  and the same upper bound 7. Typed array geometry alone establishes no bound.
- Identity and lifetime: selector values, including compiler-generated copies,
  identify an element. Equivalent captured scalar recipes use the existing
  scoped arithmetic equality judgment. The stored value is captured before
  overlapping writes invalidate observations. A source handle or equal type is
  insufficient to identify a value or loaded reference.
- Composition: each edge substitutes the entire actual telescope, including the
  structural argument and selector. Every complete cycle owes strict descent;
  nesting supplies no ambient parent fact. `copied_selector/` and
  `equivalent_selector/` preserve the selected coordinate through a copied name
  and an exact `selected + 0` recipe. `selector_actual_result/` produces the
  selector with a counter-mutating helper and checks it executes exactly once.
- Soundness: entry selectors must be available in that invocation. An unexecuted
  producer, a folded expression hiding such a producer, and the not-yet-produced
  result do not supply an entry snapshot. Stored-reference loads retain their
  independent exact frame-source requirements. Atomic, placed, write-only and
  unproved case observations acquire no ordinary read authority.
- Psi 05–07: admitted entry bounds remain on the selected rank coordinate.
  Pure read templates bind its selectors to captured scalar formals and its
  array to the full structural formal. They emit ordinary operation-owned
  runtime-index obligations without inventing an authored indexing occurrence
  or placing control before the entry rank exists.
- Omega 00–02: all nine positives declare all four hosted `ProgramEntry` rows.
  Original borrowed backing, complete record geometry and scalar argument lanes
  use the ordinary layout and call plans.
- Omega 03–09: acceptance is `omega run --both main.omg`, with native and Terminal
  exit 0. Wrong branches, repeated or missing increments, wrong element offsets
  and neighbor corruption trap. Host observations and producer identities live
  in the qualification/claim record; other-host execution and portable WCSU are
  separate remaining requirements.

The fixed base `3770944139e502df2b0674e3484f77a36456fd8d` rejects all nine unchanged
positive programs: it loses the indexed value when applying the helper's call
contract and cannot admit their joint rank. The frozen local source and its
baseline diagnostic were recorded before the compiler repair.

Controls in the parent `controls/indexed_*` folders remove an entry selector
bound, switch the arriving selector, or write only the neighbor instead of the
ranked element. Missing arrival contracts remain independent obligations; their
refusal is not reported as a strict-descent proof. Descriptor-indexed coordinates,
case coordinates, further reference loads, richer joint views, portable stack
accounting, source migration and the product compiler remain parent acceptance.
