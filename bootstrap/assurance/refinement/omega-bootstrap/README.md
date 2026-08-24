# Bridge refinement reconstruction

This directory owns cross-rung reconstruction and checking of claims that join
the bridge's Omega meaning or produced artifacts to lower-rung evidence:

- `gamma2claim.py`, `meaning-tv.sh`, and `input-tv.sh` reconstruct meaning-route
  claims and safety obligations from elaborated Gamma;
- `tv-encode.py` and `translation-validation.sh` reconstruct a claim relating a
  native Delta-produced result to the Rust-free meaning route;
- `meaning_cert_diamond.py` and `meaning-cert-diamond.sh` replay those generated
  claims across the independent checker implementations.

The CKIR1 artifact tranche uses one exact raw per-compilation envelope for its
direct Beta refinement checkers. Its little-endian wire is:

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
Beta compiler and Alpha seed. Envelope acceptance alone does not close §10.6.

The next source-side layer is
`ckir-refinement-source-input.beta`. It independently decodes the exact
one-unit `OMG0BNDL` input retained by the envelope, validates the canonical
label and exact content extent, and lexes the complete source with nested
comments, bounded identifiers, checked decimal integers, and exact EOF.
`checked-ir-refinement-source-input.sh` carries positive comment/trivia forms
and isolated bundle, label, lexical, and exhaustion negatives. Lexical custody
alone is not a source→CKIR claim.

`ckir-refinement-source-tables.beta` is the next source-side layer. Starting
from those retained source bytes, it independently parses and resolves data,
field, type, machine, entry, state, and parameter declarations; reconstructs
copyability and recursive layout; interns the canonical CKIR1 types; and joins
the resulting record, field, machine, parameter, and block signatures to the
claimed CKIR. `checked-ir-refinement-source-tables.sh` carries the product
library, renamed/reordered sources, a valid copy-owner alternative, and
cross-pair controls in which valid source or CKIR semantics are changed alone.
This closes declaration/signature/layout correspondence. Source bodies,
operations, terminators, state facts, and the source-derived result remain the
next source-side layer.

The first artifact-side layer is `ckir-refinement-artifact.beta`. It decodes
the exact CKIR bytes directly, validates the complete CKIR1 declaration,
layout, ID/span/visibility, operation, terminator, root, and resource relations,
and independently evaluates the selected closed scalar entry from a zeroed
owner. `checked-ir-refinement-artifact.sh` runs that checker through persisted
Beta on the real all-operation fixture, the product library, valid structural
and self-aliasing-copy controls, a wrong-result claim, and the complete 142-row
schema mutation inventory. This establishes CKIR custody and recomputes the
claimed full result.

`ckir-refinement-elf.beta` independently reconstructs the selected private
layout, frame, copy leaves, shim, trap, every operation and terminator template,
rel32 fixup, ELF header and segment, padding, and EOF directly from validated
CKIR1. It also joins the CKIR evaluator's full scalar result to the claimed
result and process-status projection. `checked-ir-refinement-elf.sh` carries
the real fixture and library, a valid self-aliasing control, CKIR/ELF cross-pairs,
and isolated entry, field, branch, syscall, padding, truncation, trailing-byte,
and wrong-result controls. This closes the lower-rooted CKIR1→limited-ELF
relation for the selected finite closed entry profile; composing it with the
still-open source-body relation is the remaining source→artifact work.

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
