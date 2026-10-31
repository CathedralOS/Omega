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

## Current failure and remaining acceptance

On fixed base `5b4628fb00863b76e4a1a33a5108f2646db32096`, the exact optimized
producer rejects all nine valid places at `observe`'s `result >= 0 && result
<= 16` guarantee. Their twins reject there too. An otherwise identical
fixed-index observation checks successfully. The sources, original outcomes,
producer profile and oracles are frozen in the qualification evidence.

Completion requires the general projection/fact repair, all nine native/Terminal
agreements, their intended reject twins and exact corpus pins, the related
historical indexed-storage restorations and independent exact-candidate review.
The full parent migration remains open.
