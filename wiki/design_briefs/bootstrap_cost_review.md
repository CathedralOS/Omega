# Bootstrap cost review

Measured against `59e73ffef81b550c0eac4946b0e40970dc90942b` on macOS arm64.
This is engineering evidence, not an owner ruling or admitted proof artifact.
The objective remains the [smallest human-auditable chain](bootstrap_minimization.md).

## Findings and disposition

| Area | Evidence | Recommendation |
| --- | --- | --- |
| Delta height normalization | Current Epsilon customer lowers to height 69; normalization adds no helpers. | Freeze expansion. Do not raise Gamma depth for this customer. Resolve deep-source conformance before deleting the transform. |
| Generic Gamma checker | Entire raw source/tape lists can be checked, but separately represented copies cost 229 seconds before any encoding derivation. | Keep the narrow checker; require a costed, shared certificate strategy before further encoder helper families. |
| Partial Beta theory | Lexical and word helpers exist; full encoder, independent owner root, and source-owned certificate producer do not. | Do not treat helper completion as full-proof feasibility or trust reduction. |
| Retained concatenative route | Eleven bootstrap files, 236,347 bytes; eleven remaining test entrypoints after completed-discriminator retirement below. | Candidate for further retirement, not dead code. Map unique coverage and preserve useful findings before coordinated removal. |

No production implementation, private capacity, language contract, or trusted
assumption changed in this review. It adds one optional diagnostic alongside the
existing theory gate, not a new framework, compiler stage, or accepted artifact.

## Metrics that determine the next step

1. Exact remaining trusted programs, definitions, and admissions; which named
   obligation a completed certificate would remove, and which it would not.
2. Human audit burden across the whole route: source, semantic cases, state,
   invariants, proof rules, resource arguments, and permanent validation.
3. Actual next-compiler demand, with exact input identities and owned outcomes.
4. Full-certificate bytes, retained state and cumulative allocation, depth,
   checking time, and producer cost. Distinguish measured peaks from bounds.

File/line totals are inventory, not a proxy for correctness or audit difficulty.
Passing small equations does not establish any of these whole-chain results.

## Delta: current customer versus conformance envelope

The existing source-owned
[normalization diagnostic](../../tests/delta/normalization/normalization_driver.gamma)
was composed with the selected Delta implementation manifest. Its 157,030 bytes
match the committed diagnostic pin. Input was the exact 87-member Epsilon closure
(610,428 bytes), one LF, and the existing 2,565-byte
[execution entry](../../tests/epsilon/interpreted-omega-experiment/execution_driver.delta).
The combined 612,994-byte subject has SHA-256
`251a97366c26c4356e4c573e353f8998d3c7d26985bc6052deafc4fe28a43f94`.
Standard Gamma framing and the unchanged selected evaluator produced status 0,
empty stderr, and these five u32 observations plus the scalar terminator:

```text
authored definitions / maximum lowered Gamma height: 724 / 69
normalized definitions / maximum Gamma height:       724 / 69
maximum generated-helper parameters:                0
stdout hex: d402000045000000d4020000450000000000000000
elapsed: 86.747 seconds
```

This establishes customer demand, not complete receipt equivalence or Epsilon
execution correctness. A separate host-only source census found 720 authored
Epsilon functions and maximum Delta expression depth 23; the source-owned lowered
height 69 is the relevant result. Neither measurement covers future customers.

The normalizer owns ten Gamma files: 303 lines/14,326 bytes. Its README and five
gate files add 408 lines/22,121 bytes. Its obligations include capture identity,
scope, fresh parameter mapping, evaluation/trap order, tail position, height
termination, refreshed emission extents, and generated-helper resources.
Those obligations, rather than 303 lines alone, are the cost of retaining it.

The selected Delta profile still admits 1,024-level expressions, and existing deep
controls use this transform. Deleting it now would leave that contract uncovered.
The smaller alternative is **not currently to enlarge Gamma**: its published
[containment argument](../../bootstrap/2_gamma/EVALUATOR_PROFILE.md#containment-argument)
allows at most 508 lists in the existing hidden-stack gap with unchanged context
bounds. A 511-list provision needs 16,846,848 bytes versus 16,777,220 available.
A larger coherent layout or tighter argument is possible, but has its own audit
cost and no demonstrated need from this customer.

Next useful simplification evidence: compare complete canonical emission with a
scratch source-owned path serializing the original plan for this exact customer.
Do not remove shared height/extent metadata: emission also consumes it. Any
contract narrowing remains an owner decision, not a local optimization.

## Checker: full-size representation is only a prerequisite

Run `sh tests/gamma/beta-encoding-theory/run.sh --subject-shape`.
It uses the unchanged 63,504-byte checker diagnostic and 93,140-byte source-emitted
partial theory. The subject is the entire 46,484-byte evaluator Beta source and
8,355-byte Alpha tape, with their existing profile identities printed by the gate.
Source and tape retain separate list roots. All byte values share 256 constructor
rows and one Nil; duplicate witness list spines force structural traversal rather
than same-reference shortcuts. Both supplied proof rows are checked, even though
only tape reflexivity is the final diagnostic root.

| Observed positive request | Result |
| --- | ---: |
| Request bytes | 2,729,608 |
| Owner / witness ground rows | 55,096 / 54,839 |
| Proof rows | 2 |
| Cumulative checking work | 219,369 of 262,144 |
| Wall time | 229.060 seconds |

The companion request changes the witness source's first byte and must reject
at the first proof row's right field. It returned tag 1, code 12, coordinate
2,729,588, zero limit/requested, in 125.442 seconds. Both invocations returned
process status 0 and empty stderr; the complete owned bytes determined the
verdict. Windows execution was unavailable. This is scoped diagnostic evidence,
not a fresh whole-checker conformance run.

This does **not** prove Beta encoding, authenticate the owner, or prove evaluator
correctness. It shows that the current representation fits this preliminary
whole-subject check and that duplicate endpoint traversal is expensive.
No runtime allocation peak was instrumented. The published ledger gives the
conservative bound `2*23284 + 3*109935 + 32*8 + 26 + 96*219369 + 128`
= 21,436,207 cumulative pairs for this request, not measured allocation or RSS.

There is a concrete sharing opportunity without new rules. Using the same
validated root identities would reduce these two reflexivity rows to 13 work
units, by the existing accounting, but ground admission would still traverse the
input. That is a derived comparison, not a measured full encoding certificate.
Similarly, the raw source has 76 distinct byte values across 46,484 occurrences.
Repeated `source_byte` unfolding per occurrence would cost 3,581,951 work under
the current literal recipe; one closed equation per distinct value costs 6,477.
These are source-derived recipe costs (`sum(byte+7)+5`), excluding traversal,
state transitions, and composition. Reuse helps; it does not prove the missing
encoder will fit. Raising the work provision alone also changes its cumulative
allocation argument and cannot be assessed from a timeout alone.

The missing dependency sequence remains:

```text
complete error-valued Beta definitions
  -> independently constructed exact source/tape/limits proposition
  -> source-owned untrusted producer with shared terms and closed equalities
  -> complete checked encoding certificate and artifact custody
```

Definitions must cover malformed input and limits as well as successful opcodes,
operands, trivia, assertions, and EOF. The owner must fix the theory and proposition
independently; the producer cannot choose them. All of this is additional to the
current generic checker (1,365 Gamma lines/62,349 bytes) and partial theory emitter
(435 Gamma lines/21,305 bytes), excluding their docs and tests.
Even a successful encoding certificate leaves the evaluator's implementation of
Gamma trusted, as the [checker contract](../architecture/bootstrap_chain/proof_kernel.md)
states. It is not a proof of the whole compiler chain.

Before another helper family, specify the state/recursion and proof-composition
route for a complete encoder and cost a representative contiguous sequence,
including a word operand, trivia, an assertion, and state/endpoint composition.
Reuse existing ground identities and closed facts first. Report extrapolation
separately from complete-subject measurement. If this cannot plausibly improve
the total audit burden, return for a design decision instead of expanding it.

## Counter strategy: sharing does not establish feasibility

A follow-up at `e128a74dbba34b651ac7c36e782e66ec4a0c3eeb` tests the concrete
proposal to maintain a Word source position by proving one existing
`word_successor` equation per byte. Run
`sh tests/gamma/beta-encoding-theory/run.sh --counter-cost`.
This reuses the existing diagnostic recipe, globally interns identical ground
terms, and shares identical proof rows after remapping their premises. It does
not search for shorter proofs, interpret the theory, or implement a Beta encoder.

| Constructed consecutive increments | Ground rows | Proof rows | Work | Request bytes |
| --- | ---: | ---: | ---: | ---: |
| 1 | 15 | 15 | 266 | 93,988 |
| 24 | 268 | 314 | 6,637 | 108,616 |
| 257 | 2,834 | 3,350 | 130,186 | 257,116 |
| 46,484: one per raw source byte | 373,550 | 513,467 | 11,204,359 | 25,285,216 |

The first three requests are checked by the unchanged Gamma implementation,
with exact row/work observations in 3.157, 4.863, and 25.661 seconds on macOS
arm64. Changing the final endpoint of the 257-increment request rejects with
tag 1, code 12, coordinate 257,104 and zero limit/requested in 25.932 seconds.
All four invocations return process status 0 and empty stderr; the complete
owned observation determines the verdict. Windows execution was unavailable.
The full-sized table is **constructed only**:
it exceeds both the checker's 8 MiB input and Gamma's 16 MiB enclosing frame.
Its work is derived from the existing recipe ledger, not an executed verdict.
Before sharing, the same full-sized recipe would retain 790,878 term rows,
977,793 proof rows, and 25,098,413 work. Sharing materially helps but still leaves
source counting alone at about 43 times the current checking-work provision.
Neither table includes a proof that these numbers count the source, a scanner,
operand parsing, output construction, assertions, or the final encoding root.
This is a cost for this recipe, not a lower bound on every possible certificate.

The 24-byte literal example is:

```text
imm r0 0x1 ; x
0xa:
ret
```

It includes a word operand, comment, exact address assertion, and final LF/EOF.
Its expected tape from the written Beta rules is opcode 1, register 0, the eight
little-endian bytes of 1, then opcode 20: eleven bytes. This is a hand-derived
example, not a checked encoding result. The probe establishes only the cost of
its source-position increments; token handling and their connection to those
eleven output bytes are deliberately still missing. Small-example success must
not conceal the full-subject failure of the proposed counter strategy.

### Candidate complete construction and remaining decision

The complete transparent encoder needs three ordinary-definition phases:

1. Validate the entire source envelope and its capacity, including comment bytes.
   This cannot be delegated to an unchecked producer-supplied length. The
   per-byte Word-successor recipe above is not a viable baseline under the current
   profile. A grouped/binary length derivation is an engineering alternative to
   cost before adding helpers; a larger coherent profile is another, not a
   change to source semantics. Neither alternative is established by this probe.
2. Consume the immediate source tail in one structurally decreasing scanner.
   Its state distinguishes trivia, comments, an unfinished token, expected item
   or operand, emitted-byte count, output accumulator, and failure. Completed
   tokens select only the closed mnemonic/operand table, `dw`, or an assertion.
   EOF must finish the last token and reject missing operands. Failure values
   remain explicit; no helper-returned cursor substitutes for structural decrease.
3. Finalize an error-or-success value, exact output ordering and provision, and
   full exhaustion. Assertions compare against emitted length at their precise
   position. The owner fixes the complete `encode_Beta(S, limits) = Success(T)`
   root from the exact raw bytes and independently fixed definitions.

For a forward state-fold formulation, one byte step would unfold the scanner,
use a closed state-transition equality, rewrite its recursive-call argument by
congruence, and compose with transitivity. Shared source suffix identities avoid
copying source lists; shared byte classifications and closed arithmetic facts
avoid repeating those derivations. Equalities still have to establish every
transition and finalization. This specifies the dependency/recursion route, not
an implemented or fully costed theory. Output ordering and resource/error
precedence must be audited against Beta before promoting particular equations.

**Disposition:** do not extend the encoder around per-byte Word counter proofs.
The next proof-feasibility decision is cheaper capacity accounting and its
composition with this complete scanner route, followed by a costed contiguous
example. An 8 MiB limit increase alone cannot fit the measured recipe; Gamma
framing and the cumulative-allocation argument also need consideration. No new
trusted arithmetic, assembler primitive, unchecked length, chunk-level budget
reset, or narrowed root follows from this engineering result. The complete
certificate remains open, and its auditability advantage remains unproven.

## Legacy route: retirement must follow its consumers

`bootstrap/2_gamma/bootstrap/concatenative/` and
`bootstrap/3_delta/bootstrap/concatenative-compiler/` retain four language-source/
receipt files totaling 6,658 lines/195,831 bytes, one 26,674-byte tape, and six docs.
These are outside the selected execution route; their size is maintenance and
review-discovery cost, not automatically additional trusted chain material.

The six remaining Gamma entrypoints are `evaluator-slice.sh`, `evaluator-reconstruction.sh`,
`compiler-fixed-point.sh`, `state-machine-customer.sh`, and the `run.sh` files in
`gamma1-augmentation-experiment/` and `gamma-to-beta-experiment/`.
The five remaining Delta entrypoints are the `run.sh` files in `compiler-slice/`,
`state-machine-experiment/`, `functional-compiler-experiment/`,
`forth-gamma-experiment/`, and `streaming-compiler-experiment/`.
The role registry, two artifact helpers, and
chain-hygiene gate also retain the old route. Searches must include helper calls
and role variables, not only literal directory names.

Their [retention inventories](../../tests/delta/README.md) include historical
architecture discriminators and conformance evidence. Some old comparisons
explicitly lack current typing, arithmetic, and Bytes coverage, so their line
counts do not justify replacing the selected implementation. Audit each unique
assertion against selected-route gates; keep useful findings in existing design
documentation and retire obsolete execution paths together with their adapters
and hygiene requirements. Do not preserve them merely to make inventory checks
pass, or delete them while leaving their consumers broken.

### Completed topology discriminators

The direct-Beta feasibility and direct-Delta evaluator experiments meet their
documented retirement conditions. D88 selected the implemented direct functional
Gamma evaluator; D92 rejected moving Delta's recursive-data machinery into its
Beta trust root. Those ratified records and the selected evaluator's
[measurement discussion](../../tests/gamma/evaluator-development/README.md)
preserve the reasons: generated-call expansion was expensive, but removing Gamma
by enlarging the low-level evaluator did not reduce matched audit cost.

The retired directories are `tests/delta/direct-beta-feasibility-experiment/`
and `tests/delta/direct-beta-evaluator-experiment/`: six files, 2,430 lines,
62,896 bytes, including the 2,019-line symbolic direct-Delta prototype.
Their source, measurement assertions, and reconstruction recipes remain
recoverable from Git at `0ac643b7237ec7d667dff4868c3fc8ebee542e83`.
They are not accepted artifacts or sources used by another executable gate.

| Former obligation | Retained evidence or selected-route coverage |
| --- | --- |
| Generated tokenizer/call/helper size comparison | D88 and the selected evaluator measurement discussion retain the topology finding; old generated-code sizes are not selected-language conformance. |
| Exact Nat, List, and recursive rope observations | `tests/delta/staged-compiler/{recursive_match,list_match,bytes_rope}.{delta,gamma}` and `run.sh` compile, compare exact receipts, and execute results. |
| 100,000-node proper-tail construction/traversal | `proper_tail` in that same selected gate exercises tail calls through `if`, `let`, and `match`. |
| 3,001-function witness | `stress` in that gate retains the same source and result 199. |
| Unknown nominal field and missing match arm | The selected gate's `malformed` cases reject without emitted bytes. |
| Rejection of reordered exhaustive arms | Not retained: it contradicts current Delta. D110 and the selected `reordered_match` case require acceptance and correct dispatch. |
| Prototype tape/source hashes and old evaluator size pins | Identify an unselected prototype, not a selected-chain obligation. They remain historical Git data. |

At the retirement review's base revision above, a focused selected-route run
compiled and executed all six positive programs in this mapping and rejected
the two malformed sources. The three stored receipts remained exact; no legacy
compiler, host translator, or direct-Delta prototype participated. This is not
a full Delta conformance claim. The existing selected Gamma augmentation gate
also reproduced its exact receipt and result 42. Checks ran on macOS arm64;
Windows execution was unavailable. The selected source, artifacts, and permanent
tests were not changed to make retirement pass.

No shared resolver, concatenative compiler, artifact helper, fixture, or hygiene
requirement was removed: their other consumers remain. This retires completed
comparison work without pretending that the complete Gamma-to-Delta edge has
closed or that every retained experiment is equally disposable.
