# Bridge refinement reconstruction (transitional `omega0` path)

This directory owns cross-rung reconstruction and checking of claims that join
the bridge's Omega meaning or produced artifacts to lower-rung evidence:

- `gamma2claim.py`, `meaning-tv.sh`, and `input-tv.sh` reconstruct meaning-route
  claims and safety obligations from elaborated Gamma;
- `tv-encode.py` and `translation-validation.sh` reconstruct a claim relating a
  native Delta-produced result to the Rust-free meaning route;
- `meaning_cert_diamond.py` and `meaning-cert-diamond.sh` replay those generated
  claims across the independent checker implementations.

All encoders are untrusted. They gain no authority from this location; accepted
claims still require the lower-rooted meaning and proof-kernel checks described
by the standing bootstrap decisions.

Every Beta support binary is built with the persisted lattice artifact through
`bootstrap/rungs/beta/artifact_env.sh`. `translation-validation.sh` deliberately
retains one Rust dependency only for its diagnostic native leg: it exercises
the current disposable Delta producer whose result is being checked, not a Rust
Beta compiler or an authority over the accepted claim.

The bridge's source profiles, meaning elaborator, bundle/artifact gates, and
local compiler conformance remain under the transitional `bootstrap/omega0/`
path. In particular,
`omega-meaning.sh`, `kernel-diamond.sh`, compiler emission tests, and convergence
gates are not relocated merely because they consume more than one rung: they
test the bridge product/meaning route rather than own obligation
reconstruction.
