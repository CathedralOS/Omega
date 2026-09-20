# Omega D OCREQ request entry

This gate exercises D's canonical OCREQ request entry — the sealed-request
boundary of the
[standalone compiler request](../../../wiki/spec/build/compiler_request.md) —
inside the complete manifested Epsilon-written compiler D. The
[entry](main.epsilon) is appended to the packed closure and interpreted
through the selected chain:

```text
Gamma-written evaluator -> Delta compiler -> Epsilon evaluator -> D + request entry
```

Unlike the sibling gates, which drive D through ad-hoc per-gate customers,
this entry is the request boundary itself: it reads one sealed OCREQ V1
request from stdin, stages it in a bounded private buffer, runs the
implemented phases — envelope identity and declared extents (phases 0/1) and
the inner field/tag structure pass (phases 2/6) — and publishes exactly one
OCOUT V1 outcome frame on stdout before exiting with the outcome's tag as
its exit code.

It verifies, with exact bytes, that the sealed boundary holds end to end:

- the canonical request ([request.bin](request.bin)) — the smallest
  well-formed OCREQ V1 request, one external-local package with an empty
  snapshot, an `alpha_bootstrap` product invocation, and a 32-byte
  commitment — decodes complete and publishes the `coverage_request_semantics`
  `Incomplete` frame: the request's semantic phases (package-key
  recomputation, canonical ordering, graph, snapshot, admission, commitment)
  are unimplemented, so the refusal names the provision instead of producing
  a verdict;
- every refusal is computed from the assigned outcome tables rather than
  captured: an empty stream and a corrupt identity byte anchor
  `malformed_request` at request offset 0, a truncated invocation anchors at
  the invocation length field (offset 12), a trailing byte anchors at the
  declared end (offset 132), an unassigned product tag anchors at its field
  (offset 73), a hostile package-name length anchors at its count field
  (offset 20), a well-formed request with a zeroed commitment still reaches
  the coverage provision, and a request beyond the entry's 65,536-byte
  staging bound publishes `request_staging_bytes` with the full observed
  length as the requested amount.

The expected observation is [expected.hex](expected.hex), computed from the
assigned tables rather than captured output. The pinned request fixture is
input to the boundary, not output from it; mutating it is how the refusal
cases are built. This is the request edge's own observation — no raw-source
stdin convention, and no host compiler meaning is supplied.

From the repository root on macOS arm64, or Windows x64 with Git Bash:

```sh
sh tests/bootstrap/omega-request/run.sh
```

The gate requires Python 3, the selected checked-in Alpha seed, and the
existing shell tools; macOS also requires `codesign` for the materialized
evaluator. Outputs live in ignored `build/omega-request/`.
`OMEGA_REQUEST_OBSERVATION_SECONDS` overrides the default 14,400-second
customer watchdog, `OMEGA_REQUEST_RECEIPT_SECONDS` overrides the default
1,800-second receipt-reconstruction watchdog, and `OMEGA_REQUEST_BUILD_DIR`
selects a different output directory; these are host controls, not language
semantics.

## Bound request boundary subjects

The [entry](main.epsilon) is bound at 4,115 bytes, SHA-256
`0d612813e17cfbe2e755b7398d90bb3572f5ed32da249c8863b37f545d3822c0`, and packs
on top of the bound member closure to 571,394 bytes, SHA-256
`571d738bbc140cfff0de150f281aabd2fe7d048860320ee14def7cc7025762fa`. The
canonical request fixture is bound at 132 bytes, SHA-256
`ab2e980a89d20651b69782446cd8a8333313dce109636fd3e26cc7f52bc98062`.
`tools/bootstrap/omega/compiler_env.sh` checks both identities before every
packing and `tests/bootstrap/omega-identity.sh` covers the refusals. The same
pins stand inline in `gate.py`; they are records of these same subjects, not
independent identities.
