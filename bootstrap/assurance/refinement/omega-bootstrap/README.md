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

The two-unit `OMGCOMP` successor is pinned in
[`OMGCOMP_REFINEMENT_WITNESS.md`](OMGCOMP_REFINEMENT_WITNESS.md). Its version-2
frame carries one untrusted normalized-resolution witness so source resolution,
CKIR tables, body lowering, and result checking remain separate persisted-Beta
conjuncts under the 128-procedure ceiling. The witness contains no operations
and carries neither resolver-receipt nor digest authority.

Five focused `OMGRFN2` gates now close the selected public two-package,
finite, acyclic, returning source-to-limited-ELF relation. They independently
own frame/OMGCOMP/source custody, source→witness resolution, witness→CKIR
tables/layout/root, resolved bodies→CKIR plus an artifact-free full source
result, and complete CKIR/result→ELF reconstruction at the v2 offsets. The
lattice driver runs those five conjuncts after the native/self-built and
Rust-free producer composition. This modular conjunction is artifact
refinement, not compilation authority: accepted resolver receipt bytes and a
lower-rooted comparison of their expected envelope SHA-256 remain separate and
open.

For the earlier one-unit `OMGRFN1` tranche, the first source-side layer is
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
copyability and recursive layout; interns the canonical CKIR1 types; and
derives the source root and canonical signatures. Its CKIR join then compares
the resulting record, field, machine, parameter, and block signatures to the
claimed artifact. `checked-ir-refinement-source-tables.sh` carries the product
library, renamed/reordered sources, valid copy-owner and trapping alternatives,
exact layout exhaustion, and cross-pair controls in which valid source or CKIR
semantics are changed alone.

`ckir-refinement-source-lowering.beta` independently reconstructs source-body
operations and operands, value/place identities and types, terminators, edge
arguments, transition facts, invalidation, and canonical evaluation order.
`checked-ir-refinement-source-lowering.sh` joins those rows to CKIR in a
separate persisted-Beta conjunct and carries semantic cross-pairs plus distinct
semantic and resource failures.

`ckir-refinement-source-result.beta` evaluates the source-derived rows without
reading CKIR, ELF, or their evaluator caches. Its gate composes three
persisted-Beta conjuncts over the same exact envelope: CKIR validity and result,
exact CKIR-to-ELF reconstruction, and source reconstruction and result. Valid
source/artifact and CKIR/ELF cross-pairs isolate both joins, and a full-width
result mutation sharing the same exit byte prevents an exit-status-only claim.
This closes the first finite, acyclic, returning source-to-artifact tranche.
Cycles, traps, divergence, and later profile growth require separately stated
observations rather than being inferred from this bounded result contract.

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
source-derived body and result check above closes the current bounded
source→artifact relation.

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
