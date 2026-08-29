# `source/alpha/checker/` — rooted certificate-checker service

This is a service beside the compiler lattice, not a language rung and not a
compiler edge. It answers one bounded question:

```text
Does certificate C derive proposition P under the declarations in the input?
```

Only the persisted Alpha tape built from `implementations/beta/check.beta` has
authority to answer that question. Proof search, artifact-obligation
reconstruction, compiler execution, and deployment policy remain outside the
checker. In particular, the checker does not decide that a valid derivation is
about the compiler artifact a caller intended; an artifact-aware owner must
bind the exact subject and construct the proposition independently.

## Rooted construction

```text
audited Alpha seed
  + Alpha-written assembler
  + source/beta/compiler/beta_compiler.alpha
      -> fresh Beta compiler tape
      -> implementations/beta/check.beta
      -> artifacts/proof_checker_bytecode.tape
      -> accept | reject
```

`reconstruct-artifact.sh` repeats this construction without trusting the
persisted Beta compiler tape and compares the result byte-for-byte. This closes
the checker's construction route; it does not by itself prove the Beta
compiler correct. Exact source-to-tape edge admission still requires the
artifact-bound derivation tracked in `TASKS_BOOTSTRAP.md`.

Input is a declaration prefix followed by one proposition and one proof term.
Output is `accept` with exit status 1 or `reject` with exit status 0. The
calculus implemented by `check.beta` includes constructive propositional and
first-order rules, computation-aware equality, natural/list/user-data
induction, equality transport, no-confusion, named lemmas, bounded user-function
reduction, list membership, and product witnesses. The compact gate suite owns
the executable rule inventory; a theorem library is deliberately not stored
under this service.

Function, constructor, and product declarations must precede the first checked
lemma. Their tables are bounded, IDs and rules are unique, and the theory freezes
at that first lemma; later declarations reject rather than retroactively changing
the meaning of an accepted proof. Lemma IDs are likewise bounded and unique, and
no non-whitespace form may follow the one goal/proof pair.

Artifact-bound callers use the binary frame below; legacy plain certificate
input remains available for generic judgments.

```text
"OMGCHK1\n"
u64le source_length | raw source bytes
u64le tape_length   | raw tape bytes
u64le cert_length   | certificate bytes
```

Lengths have zero high halves and checker-owned limits, the certificate must
end exactly at the frame boundary, and the whole input is bounded before it can
overlap checker tables. Within a valid frame, `source` and `tape` parse as
immutable balanced byte trees constructed by the checker itself. A byte is
constructor 60 applied to fixed high- and low-nibble constructors 0 through 15;
constructors 61, 62, and 63 are empty, leaf, and binary node. Byte terms are
interned, tree depth is logarithmic in the subject extent, and framed
certificate functions may dispatch on those fixed shapes without redeclaring
them. The constants and fixed shapes are unavailable to unframed input. This
binds a derivation proposition to exact bytes without trusting a hash or a
shell-generated literal, but a caller still needs a checked artifact-specific
ledger proving the intended relation.

## Retention inventory

Every retained owned file must strengthen the rooted checker service or one
exact compiler-edge consumer and must have an explicit deletion condition.
Material that merely demonstrates mathematics, generality, historical effort,
or another oracle is negative value here. Git history, not the live tree, owns
discarded possibilities.

| Retained child | Bounded role | Deletion condition |
| --- | --- | --- |
| `implementations/` | Authoritative Beta checker, its bounded equality seam, and one independent diagnostic reference. | Delete a diagnostic implementation when no retained gate consumes it; replace the Beta checker only atomically with the accepted checker tape. |
| `artifacts/` | One persisted platform-independent Alpha checker tape. | Delete when an equally low or lower accepted checker artifact replaces the service. |
| `gates/` | Compact rule discriminators, adversarial rejects, one complete independent diamond, and one operational equality seam. | Delete a gate when subsumed by a stronger formal check or when its implementation/seam is retired. |
| `corpus/` | The deterministic generator for the single independent checker diamond. | Delete with that diamond or replace atomically with its successor generator. |

Root scripts are retained only for exact construction and loading:

| File | Role | Deletion condition |
| --- | --- | --- |
| `artifact_env.sh` | Stamp the accepted tape into the selected audited Alpha seed. | Delete when the canonical Alpha executor accepts a tape without stamping. |
| `construct-artifact.sh` | Construct the checker tape through the exact below-Beta route. | Delete when the authoritative checker source or immediate construction edge changes. |
| `reconstruct-artifact.sh` | Compare a fresh construction to the persisted tape and run accept/reject controls. | Delete when a stronger exact construction gate fully subsumes it. |

## Principal checks

```sh
sh source/alpha/checker/reconstruct-artifact.sh
sh source/alpha/checker/gates/test.sh
sh source/alpha/checker/gates/soundness.sh
sh source/alpha/checker/gates/check-ref-diamond.sh
sh source/alpha/checker/gates/semantics-diamond.sh
```

- `test.sh` is a compact positive/negative discriminator per calculus boundary.
- `soundness.sh` attacks the checker with malformed, false, capture-unsafe, and
  non-constructive certificates.
- `check-ref-diamond.sh` compares the complete retained rule set with one
  independently written Python reference. The reference is diagnostic and has
  no runtime authority.
- `semantics-diamond.sh` is one bounded bridge between definitional equality
  and Gamma's operational evaluator. It is evidence, not a soundness theorem.

The deleted theorem corpus, proof-search stack, Gamma checker copies, format
adapters, and overlapping fuzz/gate entry points remain recoverable from Git.
They are not part of the live assurance surface because none was required to
construct or validate the direct Alpha-tape compiler chain.
