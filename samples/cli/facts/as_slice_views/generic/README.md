# Generic whole-array production

Owning requirement: **STRUCTURAL-BORROW-IDENTITY / GENERIC-FIXED-ARRAY-VIEWS**
in [TASKS.md](../../../../../TASKS.md). This is another production of the
[whole-array view fact](../README.md), retaining its four-entry extent and
`require_four(view.len)` consuming operation across the same nine places.

`whole_view<Element, const Count: u64>` forms the ordinary shared range view
from `&[Element; Count]` and promises `result.len == Count`. Element and extent
parameters retain resolved declaration identities. The caller must consume the
returned extent without adding an equality guard as proof. Every short-extent
twin changes the backing extent and corresponding transport contract to three;
the consuming requirement remains four.

At fixed main `178897a3f89a7158387d7df5d4c14ee42e772701`, the frozen root
positive is wrongly refused at `whole_view`'s ensures clause. An optimized
Linux x86-64 producer with SHA-256
`1a7a8b4950900035d021861c09817bf49753337606c84af373d6d1958a2ff0e8`
also refuses a const-generic scalar `items.len` result while accepting the
same helper with literal extent four, element-only generics, and a direct
return of `Count`. The retained claim record holds exact source hashes,
producer inputs and raw checking diagnostics. These are baseline observations,
not acceptance of the new production.

The same frozen root also refuses on current main
`e4d9c74577f9ed221697b73113b045220364f9de` with its independently built,
optimized Windows producer (`9c54bc1e2ce01743cb51ae02e84b86fc74a5c5fd961f6923130d9e4e75f97c3b`).
The original source and authored oracle remain unchanged.

The repair retains the fixed-array extent under the completed actual descriptor
before a slice formal attenuates its type, and imports open operator type/const
arguments under the same exact telescope as ordinary calls. Address formation
consumes physical path and selector geometry without requiring an unrelated
standalone scalar declaration for an inline field. Typed loan presentation and
reference access checks remain separate owners.

Linux development observations establish native exit 0 and Terminal Psi exit 0
for the root, all nine places, a shadowed const binder, a nonzero-offset selected
field, and selectors on both sides of a stored fixed-array reference. Reject
controls cover short extents, an unrelated const binder, stale descriptors,
backing writes, narrowed descriptors, both selector bounds, a write-only stored
pointer crossing, and mismatched static argument categories. Raw commands,
producer/source identities and actual engine outputs remain in the claim's
qualification records. These development observations are not final candidate
qualification. The stored-reference case uses explicit intermediate values;
it does not claim support for a nested-call lowering route.

An exploratory array of returned slice descriptors still loses its initializer's
length fact before the consuming call. Its frozen source and refusal remain in
the claim evidence as remaining parent acceptance; this repair does not claim to
close that separate production. No matching macOS ARM or Linux ARM execution
host is verified for this wave. All four hosted entry bindings are authored;
Linux and Windows execution observations remain distinct from that binding
coverage.

Final exact-candidate validation is pending. Extent establishment, exact backing and lifetime,
call/join composition, invalid extent and backing controls, checked-to-Terminal
lowering, target realization and ABI, instruction/frame realization, and final
image execution must retain the governing row's obligations. All four hosted
entry bindings are authored; matching-host `omega run --both`, corpus pins and
independent exact-candidate review remain required. This production does not
close the parent's other indexed borrow or cross-host recast requirements.
