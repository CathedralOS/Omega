# Omega product compiler

This package is the Omega-owned half of the final self-hosted compiler. The
sibling [`../psi/`](../psi) package owns target-neutral source processing,
checking, proof, and Terminal Psi. This package consumes Terminal Psi and owns
provider selection, target realization, ABI, artifact emission, and the product
entrypoint.

[`build.omg`](build.omg) and [`main.omg`](main.omg) are the current roots of the
Omega-written compiler closure C. The build binds the ordinary
`alpha_bootstrap` target alongside native targets and depends on Psi and the
standard library through ordinary package declarations.

The entry is the sealed
[standalone compiler request](../../wiki/spec/build/compiler_request.md):
[`main.omg`](main.omg) reads one OCREQ V1 request from stdin and publishes
exactly one OCOUT V1 outcome on stdout, exiting with the outcome's tag.
Nothing outside the request supplies meaning. [`slots/review.omg`](slots/review.omg) re-admits the public slot records Psi
checking publishes (returned-borrow origins and parameter qualification
preconditions). [`request/`](request) holds the
boundary: [`request.omg`](request/request.omg) stages the request (a private
65,536-byte provision) and runs the framing and extent phases (0 and 1) and
the subject and invocation field/tag phases (2 and 6);
[`outcome.omg`](request/outcome.omg) types the contract's Reject and
Incomplete code tables and encodes the frame. A well-formed request stops at
`coverage_request_semantics`, exactly as D does, until package identities,
the dependency graph, snapshot materialization, admissions and the subject
commitment (phases 3, 4, 5, 7 and 8) are implemented and hand the root
source to Psi.

[`tests/product/request/test-request.sh`](../../tests/product/request/test-request.sh)
builds this root natively and checks its frames against D's request-gate
vectors and against structured variants whose anchors are computed from the
fields they name. Set `OMEGA_CLI` and `OMEGA_TARGET` as for the Psi gates.

The first compiler D is a separate Epsilon-written bootstrap implementation
under [`../../bootstrap/5_omega/`](../../bootstrap/5_omega). Interpreted D compiles
this final closure to `omega0`; `omega0` then recompiles the same closure to
`omega`. Rust under [`../../omega-rust/`](../../omega-rust) remains a
nonauthoritative development comparator.

```text
interpreted bootstrap/5_omega D + source/{psi,omega,library} -> omega0
omega0 + source/{psi,omega,library}                        -> omega
```

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md), while the
trust-chain edge is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
