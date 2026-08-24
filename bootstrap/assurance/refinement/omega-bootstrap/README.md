# Bridge refinement reconstruction

This directory owns cross-rung reconstruction and checking of claims that join
the bridge's Omega meaning or produced artifacts to lower-rung evidence:

- `gamma2claim.py`, `meaning-tv.sh`, and `input-tv.sh` reconstruct meaning-route
  claims and safety obligations from elaborated Gamma;
- `tv-encode.py` and `translation-validation.sh` reconstruct a claim relating a
  native Delta-produced result to the Rust-free meaning route;
- `meaning_cert_diamond.py` and `meaning-cert-diamond.sh` replay those generated
  claims across the independent checker implementations.

The CKIR1 artifact tranche will use one exact raw per-compilation envelope for
its two direct Beta refinement checkers. Its little-endian wire is:

```text
"OMGRFN1\\0" | u32 version=1 | u32 flags
u32 source-bundle-length | u32 CKIR-length | u32 ELF-length
u32 claimed-full-result | u32 claimed-exit-projection
exact source bundle | exact CKIR | exact ELF | EOF
```

Flag zero is a library compilation and requires empty ELF plus both claims set
to `0xffffffff`. Flag one is entry-bearing and requires a nonempty ELF, a full
`u32` result, and an exit projection equal to its low byte. The claims remain
untrusted until the source and artifact checkers recompute them. The Python
packer only frames exact bytes. `ckir-refinement-envelope.beta` is the common
custody fragment: it independently parses the header, enforces every extent and
EOF, and retains direct access to every raw byte.
`checked-ir-refinement-envelope.sh` gates that fragment through the persisted
Beta compiler and Alpha seed. Source-table reconstruction and artifact-template
semantics remain the next two implementation layers; envelope acceptance alone
does not close §10.6.

All encoders are untrusted. They gain no authority from this location; accepted
claims still require the lower-rooted meaning and proof-kernel checks described
by the standing bootstrap decisions.

Every Beta support binary is built with the persisted lattice artifact through
`bootstrap/rungs/beta/artifact_env.sh`. `translation-validation.sh` deliberately
retains one Rust dependency only for its diagnostic native leg: it exercises
the current disposable Delta producer whose result is being checked, not a Rust
Beta compiler or an authority over the accepted claim.

The bridge's source profiles, meaning elaborator, bundle/artifact gates, and
local compiler conformance remain under `bootstrap/omega-bootstrap/`. In
particular,
`omega-meaning.sh`, `kernel-diamond.sh`, compiler emission tests, and convergence
gates are not relocated merely because they consume more than one rung: they
test the bridge product/meaning route rather than own obligation
reconstruction.
