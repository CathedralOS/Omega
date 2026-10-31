# Array element default domains

Every element of Tallies.values satisfies 0..16, so a runtime-indexed observation supplies both bounds before it indexes the 17-element mapping.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. This is the remaining
whole-storage predicate requirement behind the historical bounded-element
increment: all four signed elements retain both their zero floor and ceiling
16. A precondition on one later increment does not replace those restrictions.

`Tallies` declares the complete predicate on its containing data. The public
`observe` machine accepts an arbitrary valid `Tallies` and any index below
four, then publishes the selected element's complete bounds. `lookup` consumes
those bounds before its exact signed-to-unsigned conversion and mapping read.
The runtime checks follow that operation; they supply no proof premise.

| Place | Transport |
| --- | --- |
| local | An ordinary immutable local owns the complete array. |
| field | `Envelope.tallies` retains the predicate; an independent marker is checked after use. |
| parameter | An ordinary call receives the complete owner explicitly. |
| call_result | A completed call returns the owner with its declared default domain. |
| match_join | Both joined owners retain the predicate. Both branches execute with distinct orders and results. |
| const_name | The named immutable constant validates the same complete predicate. |
| other_state | A nested machine explicitly receives the owner at a terminal transfer. |
| other_machine | An ordinary forwarding machine returns the owner. |
| target_body | Build selects the checked named provider for all four hosted targets. |

Each positive expects exit zero from `omega run --both <place>/main.omg`.
The four observations are 0, 15, 16 and 8, mapping to `0x40`, `0x4F`, `0x50`
and `0x48`. The other match branch reverses that order. A wrong element,
offset, selected branch or forwarded owner traps. Every invalid twin removes
only element two's upper-bound declaration. Its public `observe` postcondition
is then false for some admitted inputs and must reject there; convenient
initializers cannot establish a public guarantee for arbitrary callers.

The producer and consumer use ordinary carriers and propositions. No interval
nominal, initializer replay or array capacity supplies the missing value fact.
Index formation, access, initialization and ownership remain independent.
Writes invalidate current storage observations; completed scalar snapshots keep
their original identity.

The checked projection, result bounds and ordinary calls must survive Terminal
Psi lowering, provider selection, layout/ABI planning, instruction selection,
frame realization, encoding and native execution. Actual engine observations
identify their producer and host; authored four-target bindings do not claim
execution on the other hosts.

## Baseline and acceptance

On fixed base `5b4628fb00863b76e4a1a33a5108f2646db32096`, the exact optimized
producer rejects all nine valid places at `observe`'s `result >= 0 && result
<= 16` guarantee. Their twins reject there too. An otherwise identical
fixed-index observation checks successfully. The sources, original outcomes,
producer profile and oracles are frozen in the qualification evidence.

The repair reads the complete declared projection path and combines every
possible selected element's established floor and ceiling. Completed reads
retain their own value identity; return and arithmetic consumers use that
observation rather than rereading its source storage. Declaration predicates
use the existing bounded predicate explorer, preserving resolved operator
identity and independent call admission.

Three additional productions exercise this same owner requirement once:

- [writeback](writeback/main.omg) stores 5 through a public runtime-indexed
  mutable parameter, rereads it, and stores 6. All four final cells are checked.
  Its [twin](writeback/invalid/main.omg) changes only 5 to 16; 17 cannot restore
  the owner at handback.
- [saved_read](saved_read/main.omg) saves 15, writes 16 into the same cell,
  and checks both the returned snapshot and all current cells. Its
  [twin](saved_read/invalid/main.omg) returns a fresh read under the same
  result-below-16 obligation.
- [zero_owner](zero_owner/main.omg) consumes the whole zero-initialized owner
  before checking all four cells. Its [twin](zero_owner/invalid/main.omg)
  changes only the first element's floor to one; representation zero alone
  cannot establish that owner.

The exact programs and reject twins are pinned under
`tests/omega/{pass,fail}/facts/array_element_default_domain/`. Additional
single controls retain the signed floor, independent predicate-index formation,
and selected user index operation; existing write-only and recast controls
exercise the unchanged access and carrier boundaries.

Qualification requires all nine places and these three productions to agree
under native and Terminal execution on macOS ARM64, their intended rejects,
the restored historical increment's authored exit one, focused affected-domain
checks, fresh authored-project replay and independent exact-candidate review.
Run receipts retain actual producer and target identities. Other hosted targets
and the full parent migration remain open.
