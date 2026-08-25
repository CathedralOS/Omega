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

The CKIR2 exact-root/call tranche uses the distinct
[`OMGRFN3`](OMGCOMP_REFINEMENT_WITNESS_V3.md) frame. Its first persisted-Beta
responsibility closes version-3 framing and complete OMGCOMP/source custody;
the second independently reconstructs the complete source-to-role-3 witness;
the third reconstructs witness-to-CKIR2 declarations, layout, types, root, and
tables. The fifth responsibility is split into focused CKIR/result validation
and CKIR2-to-ELF gates. The fourth independently reconstructs body/call lowering
and computes the result in a companion executable from which CKIR and ELF
readers are physically absent. A final composite runs all seven executables over
one identical canonical role-3 frame and carries source/artifact cross-pairs plus
witness-, CKIR-, and ELF-local mutations. These five responsibilities now close
the selected finite-call source-to-artifact relation below Delta. No OMGRFN2
checker is relabeled or widened in place.

The CKIR3 constant-aggregate successor is separately specified as
[`OMGRFN4`](OMGCOMP_REFINEMENT_WITNESS_V4.md). Its 4,497,544-byte simultaneous
ceiling follows directly from the already-published CKIR3 component maxima.
The contract keeps frame/source custody, resolution, intrinsic constant-table
structure, source-derived roots and meaning, and complete CKIR/result/artifact
reconstruction as five independent responsibilities over one exact carrier.
Nine persisted-Beta executables implement the five responsibilities. The first
three close exact frame/source custody, complete source-to-`OMGRSW1`
reconstruction, and the witness-to-CKIR3 declaration/layout/selected-entry/
intrinsic-DAG join. Responsibility 4 is split into source-body/operation
lowering, constant-root correspondence, cyclic interval fixed point, and a
physically artifact-free source-result evaluator. Responsibility 5 is split
into complete CKIR3/result validation and independent exact ELF reconstruction.
The final composite gives all nine executables one unchanged exact
Unicode+harness carrier and exercises source/witness/CKIR3/ELF/result
cross-pairs, phase-local opacity, and local mutations. Its source-only and
CKIR-only evaluators also close their owned `16/17` and `64/65` active-frame
pairs and the shared `65,536/65,537` dynamic-block-entry boundary. This closes
the selected constant-aggregate source-to-artifact relation below Delta; it
does not widen an earlier OMGRFN frame or admit the family to final `Ωself`.

The CKIR4 runtime-record successor is separately specified as
[`OMGRFN5`](OMGCOMP_REFINEMENT_WITNESS_V5.md). It retains the same component
ceilings and 4,497,544-byte simultaneous maximum while assigning runtime
constructor field binding/canonicalization, artifact-free snapshot meaning,
constructor-object frame extents, structural Call/Copy transport, and exact
opcode-13 ELF reconstruction to independent responsibilities. The contract is
frozen. Eight bounded persisted-Beta executables now close all five
responsibilities: frame/source custody; source-to-`OMGRSW1` resolution; the
resolution-to-CKIR4 declaration, layout, type, root, intrinsic-constant, and
opcode-13 nominal-envelope join; exact source-body lowering; physically
artifact-free source lowering and source-result meaning; complete CKIR4/result
validation; and exact ELF reconstruction. The same-frame composite feeds every
executable immutable 16,274-byte runtime-record-opener and 16,417-byte complete-
`SourceUnit`-API carriers. The latter varies resolution and body censuses while
retaining the same schemas, and has its own source/witness, witness/CKIR4,
CKIR4/ELF, and result cross-pairs. Original-carrier opacity, local mutations,
resources, and all native/self 0/251/252 controls remain green. No earlier
OMGRFN checker is widened or relabeled, and this assurance hardening neither
adds a CKIR4 source form nor decides final `Ωself` admission.

The direct field-receiver successor is separately specified as
[`OMGRFN6`](OMGCOMP_REFINEMENT_WITNESS_V6.md). It pairs only with OMGRSW2 and
retains CKIR4 unchanged. The existing five owners and eight executables are
shared rather than copied: R1 and R5 validate the exact outer version while
keeping the witness opaque; R2 reconstructs `self.field.machine(...)`; R3 owns
the OMGRFN/OMGRSW identity pair; and R4 reconstructs
`SelfPlace -> FieldPlace -> Call` plus the per-call receiver base in its
artifact-free evaluator. One immutable 16,817-byte exact `SourceUnit` plus
`SourceHost` carrier passes every executable native/self with result 70,
version cross-pairs, phase opacity, mutations, and the inherited resource
ceilings. OMGRFN5 remains byte-for-byte valid, and no CKIR5 is introduced.

The next payload-bearing pure-sum successor is specified as
[`OMGRFN7`](OMGCOMP_REFINEMENT_WITNESS_V7.md). It pairs exact OMGRSW3 with
CKIR5 while preserving the five responsibility owners and the existing outer
component ceilings. Its explicit case-arm/payload-argument relation prevents
inactive-payload reads and keeps layout reconstruction private. The contract is
fixed; responsibility-local implementations and same-frame evidence remain
open and are not claimed by this entry.

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
