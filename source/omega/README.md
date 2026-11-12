# Omega product compiler

This package is the Omega-owned half of the final self-hosted compiler. The
sibling [`../psi/`](../psi) package owns target-neutral source processing,
checking, proof, and Terminal Psi. This package consumes Terminal Psi and owns
provider selection, target realization, ABI, artifact emission, and the product
entrypoint.

[`build.omg`](build.omg) and [`main.omg`](main.omg) are the current roots of the
Omega-written compiler closure. The build binds the ordinary
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
`coverage_request_semantics`, as the bootstrap compiler does, until package identities,
the dependency graph, snapshot materialization, admissions and the subject
commitment (phases 3, 4, 5, 7 and 8) are implemented and hand the root
source to Psi.

[`tests/product/request/test-request.sh`](../../tests/product/request/test-request.sh)
builds this root natively and checks its frames against the bootstrap compiler's request-gate
vectors and against structured variants whose anchors are computed from the
fields they name. Set `OMEGA_CLI` and `OMEGA_TARGET` as for the Psi gates.

The current VM route's bootstrap compiler is separately written in Epsilon
under [`../../bootstrap/interpreted/5_omega/`](../../bootstrap/interpreted/5_omega). Competing construction
routes are peers; each must construct this compiler closure from its documented
seed, then use the resulting compiler to rebuild the same closure reproducibly.
Rust under [`../../omega-rust/`](../../omega-rust) remains a nonauthoritative
development comparator.

```text
bootstrap compiler + source/{psi,omega,library} -> omega0
omega0 + source/{psi,omega,library}             -> omega
```

The source-owned [`alpha/emitter.omg`](alpha/emitter.omg) realizes the initial
checked scalar lowering slice from Psi. Its focused
[execution gate](../../tests/product/alpha/README.md) uses an explicit source-unit
and selected-callable invocation, then executes the actual emitted Alpha bytes.
It preserves the complete frontend checking path and refuses unsupported forms.
The gate uses the Rust development compiler to build its Omega-written compiler
artifact; it does not claim a source-only bootstrap, full portable publication,
public OCREQ semantic admission or self-compilation.

Implementation work is tracked in [`../../TASKS.md`](../../TASKS.md), while the
trust-chain edge is tracked in
[`../../TASKS_BOOTSTRAP.md`](../../TASKS_BOOTSTRAP.md).
