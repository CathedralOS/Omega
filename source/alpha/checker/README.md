# `source/alpha/checker/` — rooted certificate-checker service

This is a service beside the bootstrap chain, not a language rung and not a
compiler edge. It answers one bounded question:

```text
Does certificate C derive proposition P under the declarations in the input?
```

Only the persisted Alpha tape built from `implementations/gamma/check.gamma` has
authority to answer that question. Proof search, artifact-obligation
reconstruction, compiler execution, and deployment policy remain outside the
checker. In particular, the checker does not decide that a valid derivation is
about the compiler artifact a caller intended; an artifact-aware owner must
bind the exact subject and construct the proposition independently.

## Rooted construction

```text
audited Alpha seed
  + gamma_compiler_bytecode.tape
      -> implementations/gamma/check.gamma
      -> artifacts/proof_checker_bytecode.tape
      -> accept | reject
```

`reconstruct-artifact.sh` repeats this construction with the exact canonical
Gamma compiler tape and compares the result byte-for-byte. This closes
the checker's construction route; it does not by itself prove the Gamma
compiler correct. Exact source-to-tape edge admission still requires the
artifact-bound derivation tracked in `TASKS_BOOTSTRAP.md`.

Input is a declaration prefix followed by one proposition and one proof term.
Output is `accept` with exit status 1 or `reject` with exit status 0. The
calculus implemented by `check.gamma` includes constructive propositional and
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

### Closed FloatMeaning correspondence

D40's first rooted-checker slice adds one proof-only term and one proposition:

```text
(fm FORMAT OP DECL CATALOG SOURCE_KIND SOURCE_A SOURCE_B)
(FloatMeaningEqual LEFT RIGHT)
(fmrefl TERM)
```

The canonical Binary32 contract tuple is `(32, 1, 1, 1)` and the Binary64
tuple is `(64, 2, 2, 1)`: format, projection operation, exact recognized core
declaration, and numeric-catalog version. Any cross-format, lookalike
declaration, or catalog substitution rejects before proof checking. Source
kinds `0..4` identify a contract parameter, contract result, Terminal value,
structural float leaf, or exact-bit literal. The two source words retain the
owner/ordinal, value coordinate, root/leaf coordinate, or low/high literal
bits respectively; a Terminal value has a canonical zero second word and a
Binary32 literal has a canonical zero high word. All fields are unsigned
32-bit values.

The checker compares this complete tuple structurally. Equal independently
encoded tuples therefore have one semantic identity; a source coordinate,
signed-zero bit, or any other field mutation does not coalesce and requires an
explicit checked theorem. `fmrefl` proves reflexivity, including projected NaN.
Ordinary `=`, `Pred`, and `Rel` reject the term, preserving separation from
runtime IEEE equality and preventing the proof carrier from entering generic
runtime-shaped data. The term is closed, so quantifier shifting and
substitution preserve its identity without copying or rewriting it.

The checker does not decide that a source coordinate belongs to a Terminal
artifact. The artifact-aware owner must reconstruct this exact key from the
canonical subject, just as it reconstructs every other kernel proposition.

```text
"OMGCHK1\n"
u64le source_length | raw source bytes
u64le tape_length   | raw tape bytes
u64le cert_length   | certificate bytes
```

Lengths have zero high halves and checker-owned limits, the certificate must
end exactly at the frame boundary, and the whole input is bounded before it can
overlap checker tables. Within a valid frame, `source` and `tape` parse as
immutable, power-of-two-indexed byte trees constructed by the checker itself. A byte is
constructor 60 applied to fixed high- and low-nibble constructors 0 through 15;
constructors 61, 62, and 63 are empty, leaf, and binary node. Byte terms are
interned, every real leaf has one fixed-depth left/right address, a wholly
padded suffix is represented by the fixed empty constructor, tree depth is
logarithmic in the subject extent, and framed
certificate functions may dispatch on those fixed shapes without redeclaring
them. The constants and fixed shapes are unavailable to unframed input. This
binds a derivation proposition to exact bytes without trusting a hash or a
shell-generated literal, but a caller still needs a checked artifact-specific
ledger proving the intended relation.

The independent Python reference decodes this same frame and constructs the
same raw byte trees, allowing the eventual artifact-owned proof gate to require
logical agreement over identical `source` and `tape` constants. It remains a
diagnostic implementation: it does not reproduce the authoritative runtime
resource profile and cannot admit an artifact.

Definitional equality uses the permanent arena for parsed declarations and raw
subjects. The checker records the closed arena range occupied by its immutable,
already-normal subject trees; normalization preserves pointers in that range,
and substitution, shifting, and function-rule instantiation likewise preserve
only pointers in that exact range. Certificate-spelled CID `60..63` lookalikes
are allocated later and remain ordinary terms, so they cannot bypass
substitution. Selecting or transporting one indexed byte therefore does not
copy the selected subtree. Other normal
forms created solely for one conversion decision are scratch: the checker
restores the arena mark after their structural comparison. No returned proof or
term can reference those temporary nodes. Together these rules keep a balanced
artifact certificate with many independent computations from turning dead
normal forms into permanent memory pressure.

One admitted root proposition does not require one compiler-scale conversion.
An artifact-owned proof may discharge bounded subject-bound equalities as named
lemmas, reclaim conversion scratch after each decision, and compose those
lemmas through the existing checked proof rules into one root judgment. Cut
locations are untrusted proof witnesses: the artifact owner fixes the relation,
subjects, boundary-state schemas, composition theorem, canonical endpoints, and
root proposition, while checked adjacency, ownership, and exhaustion decide
whether a proposed partition is valid.

The canonical Gamma checker must publish an exact resource profile for its
arena, semantic stack, framed input, certificate, declaration, and lemma tables.
Artifact producers target that profile rather than the unbounded diagnostic
reference implementation. A future authoritative replacement must continue to
accept the live certificate under its recorded profile or perform an explicit
certificate migration; cross-implementation agreement remains diagnostic and
does not grant the reference checker authority.

The `AlphaBootstrapV2` authoritative checker profile is:

| Resource | Exact profile |
| --- | --- |
| Complete stdin | At most 2,810,748 bytes: the 32-byte `OMGCHK1` framing overhead plus all three maxima below. Any next byte rejects before parsing. |
| Framed source / tape / certificate | Source at most 262,144 bytes; tape at most 1,048,572 bytes; certificate 1..1,500,000 bytes; every declared extent must exactly exhaust stdin. |
| Permanent + conversion arena | Logical bytes `[16,777,216, 134,217,712)`: exactly 4,893,354 complete three-word nodes at 24 bytes each, with 16 unused bytes before the 128-MiB logical raw-memory boundary. Allocation beyond start address 134,217,688 marks the candidate invalid; per-equality conversion scratch is restored to its saved mark. |
| Proof context | 65,536 proposition slots plus 65,536 matching individual-binder-depth slots. The next push marks the candidate invalid. |
| Generated semantic stack | Guarded downward physical bytes `[1,048,576, 2,097,152)`, 1,048,576 bytes total. Exhaustion halts with contained Beta runtime status 250 and cannot accept. The V2 separation `[2,097,152, 4,194,304)` and 128-MiB biased raw region `[4,194,304, 138,412,032)` keep it disjoint from checker data and Alpha's hidden return stack. |
| Constructors / products | Constructor IDs `0..63`, arity `0..2`, one declaration per ID; product marks use the same 64 IDs. Framed subjects predeclare nibble IDs `0..15` and raw-byte/tree IDs `60..63`. |
| Ground functions | Function IDs `0..767`, with at most one rule for each of the 64 constructor IDs: 49,152 fixed rule slots. The first checked lemma freezes constructors, products, and functions. |
| Named lemmas | Sparse IDs `0..32767`, each defined and checked once before use. |
| Definitional equality | 100,000 reduction-fuel units per normalization request. Fuel exhaustion does not establish equality. |

The extent ceilings are conjunctive, not a promise that every simultaneous
maximum fits the arena. A candidate accepts only if its exact framed subjects,
declarations, proof, inference, and all retained normal forms fit every row.
Anything other than status 1 with exact `accept\n` is non-acceptance.

The V2 gate retains a real maximum-size Gamma-compiler output together with its
source, named subject lemmas, bounded raw-tree selection, normalization, and
conversion scratch. Separate exact-maximum and adjacent cases pin every frame
extent, while a balanced structural identity rebuild crosses the published
arena bound and must reject without trapping. A zero-filled or tape-only
allocation test is insufficient. The rebuilt checker tape, V2 seeds, and Beta
compiler now install this profile coherently.

D14's bounded chunk lemmas do not authorize paging the root subject. The V2
checker continues to construct one immutable power-of-two tree per subject;
chunk-addressable custody remains a separate future checker/input revision.

## Retention inventory

Every retained owned file must strengthen the rooted checker service or one
exact compiler-edge consumer and must have an explicit deletion condition.
Material that merely demonstrates mathematics, generality, historical effort,
or another oracle is negative value here. Git history, not the live tree, owns
discarded possibilities.

| Retained child | Bounded role | Deletion condition |
| --- | --- | --- |
| `implementations/` | Authoritative Gamma checker and its bounded equality seam. | Replace the Gamma checker only atomically with the accepted checker tape. |
| `artifacts/` | One persisted platform-independent Alpha checker tape. | Delete when an equally low or lower accepted checker artifact replaces the service. |

Construction and loading live under `tools/bootstrap/proof-checker/`. Tests,
the deterministic corpus, and the independent Python checker live under
`tests/proof-checker/`.

## Principal checks

```sh
sh tests/proof-checker/reconstruct-artifact.sh
sh tests/proof-checker/gates/test.sh
sh tests/proof-checker/gates/soundness.sh
sh tests/proof-checker/gates/check-ref-diamond.sh
sh tests/proof-checker/gates/semantics-diamond.sh
```

- `test.sh` is a compact positive/negative discriminator per calculus boundary.
- `soundness.sh` attacks the checker with malformed, false, capture-unsafe, and
  non-constructive certificates.
- `check-ref-diamond.sh` compares the complete retained rule set with one
  independently written Python reference. The reference is diagnostic and has
  no runtime authority and is deleted when the checked direct route subsumes
  this comparison; it is not part of the completed offline bootstrap.
- `semantics-diamond.sh` is one bounded bridge between definitional equality
  and Gamma's operational evaluator. It is evidence, not a soundness theorem.

The deleted theorem corpus, proof-search stack, Gamma checker copies, format
adapters, and overlapping fuzz/gate entry points remain recoverable from Git.
They are not part of the live assurance surface because none was required to
construct or validate the direct Alpha-tape compiler chain.
