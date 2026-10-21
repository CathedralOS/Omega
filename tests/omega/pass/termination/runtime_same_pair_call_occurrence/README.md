This bounded executable preserves a weak `first -> second` tail edge and a
strict `second -> first` edge. Every cycle decreases the natural rank; both
Terminal Psi and native execution must finish with exit zero.

The check-only fail sibling uses Unit-returning members and adds an ordinary
statement call to `second` before the same `first -> second` tail edge. That separate occurrence has no tail classification
and must reject even though the same machine-pair graph is also covered by a tail transition. Runtime call
ranking cannot authorize an induction hypothesis for that returning invocation.
The invalid control is checking only and must never be executed.

Governing obligations: [termination](../../../../../wiki/spec/language/termination.md)
and [proof contracts](../../../../../wiki/spec/proofs/contracts.md), which distinguish
eventual cycle termination from an exact importing edge's induction authority.
