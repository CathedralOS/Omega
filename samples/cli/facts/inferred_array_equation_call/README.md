# Inferred array equation call

Fact: an omitted Capacity is inferred as 2 from the actual [u8; 2] type and Length == [u8; Capacity], independently of element values.

The frozen root calls `capacity(items)` with a declared `[u8; 2]` actual. It
previously failed before ordinary argument inference could anchor `Length`.
Its SHA-256 is `a03068442c4021bdb12f1cf6e784f6b91d3167944159783d882cfc2dfb46c4b3`.
The compiler now combines closed declaration-backed actual/result types with
where equations in the existing structural solver. Explicit static arguments
remain fixed, and ordinary compatibility and specialization bounds still apply.

| Place | Establishment and transport |
| --- | --- |
| local | declared array local |
| field | selected nominal record field |
| parameter | declared ordinary parameter |
| call result | declared array return type |
| match join | both arms carry the same declared type; both branches execute |
| const name | declared array constant |
| another state | state arrival parameter |
| another machine | ordinary call forwarding |
| another target body | four hosted implementations perform the same call |

Each positive declares `Expected exit: 0` and binds ProgramEntry on
windows_x86_64, linux_x86_64, macos_arm64 and linux_arm64. Each `invalid/` twin
changes exactly one consuming call to `capacity<[u8; 2], 3>(items)` (using that
place's original actual expression). The required rejection is
`where equation binds Capacity to 2 but its explicit argument is 3`, with the
compiler's backticks. Package errors or unrelated typing errors do not count.
Mirrored fixtures live under `tests/omega/{pass,fail}/facts/` with prefix
`inferred_array_equation_call_through_`.

The authority is [structural equations and inference](../../../../wiki/spec/language/generics.md#structural-type-equations-and-inference),
canonical Git blob `e3d20c0c1f44d36ac6221811e46446975bea57e8`:

> For ordinary data and machine applications, omitted arguments can be recovered
> only when the combined structural equations and existing argument/result
> inference determine one substitution.

> All applicable occurrences must be consistent, not just the first one visited.

> Inference selects arguments before checking ordinary compatibility.

The implementation/review context fingerprint is
`9e16129bdad951e9ebf98419ff1ca28ef66c9f6ca656895fbc3ab28b8d299be8`.
Resolved declaration handles retain identity through deferred body completion;
receiver projections select fields only within the exact declared nominal owner.
Literals, element values, private flow facts and storage-size coincidences do
not select generic arguments. Focused controls cover differing element values,
same-named const/type declarations, partial explicit calls, repeated conflicts
in both orders, incompatible match arms/results, constructor identity, missing
anchors, occurs cycles, copy bounds, nested/generic owner fields and deferred
bodies. A typed result may anchor inference; its argument must still satisfy
the selected array extent.

Terminal Psi and native lowering consume the ordinary completed specialization
and its concrete array carrier. This repair changes no carrier, loan lifetime,
ABI, frame allocation, image emission or backend semantics. Native/Terminal
exit agreement exercises the existing call, state transport, branch layout,
frame and image path. The witnesses assert Capacity==2 rather than merely
compiling; the join executes both arms. Linux x86-64 supplies matching-host
execution evidence. Checks for the other hosted targets supply source evidence
only; they do not establish execution on absent hosts.

This closes only the declaration-backed closed array-call inference slice of
STRUCTURAL-GENERIC-MATCHING. Open schemas, late receiver/extension selection,
operator/conformance/machine/evidence/value binders, other const-index kinds,
unspecialized alternatives and computed structured equations remain on the
parent board. The Std-backed exit-92 and exit-7 customers retain their existing
Console/entry dependencies. No parent capability closure is claimed.
