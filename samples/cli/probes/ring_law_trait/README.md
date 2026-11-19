# ring_law_trait

Free-machine trait requirements: `machine add(a: T,
b: T) -> T` (no subject), `satisfies
RingOps<Peano>::add` per-requirement binding, `as
Standard` aliased satisfier, `terminates by a`
recursion measure, and `i32 in Wrapping` as a trait
carrier (Self binds a domain-qualified primitive).

## Gaps found while authoring (filed)

A law requirement `machine add_comm(a, b) ensures
add(a, b) == add(b, a)` — verbatim from corpus
`traits/ring_requirement_satisfies_exit` — is rejected
here: "specification call `add` is not denotational:
the selected machine is not unconditionally
terminating". The ensures spec-call resolves
differently under sample-package admission than under
the corpus's; the requirement was dropped.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
