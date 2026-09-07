# First complete Beta encoder

**Status: consolidated engineering candidate; not implementation approval or a
demonstrated resource fit.** This replaces the sequence of isolated encoder
experiments as the proposed next integration unit. The customer is P1's entire
[selected Gamma evaluator source and tape](../../bootstrap/2_gamma/EVALUATOR_PROFILE.md),
not another literal instruction proof. Governing contracts are
[minimization](bootstrap_minimization.md), the
[encoding proposition](../architecture/bootstrap_chain/derivation_calculus.md#first-complete-subject),
and the complete [Beta language](../../bootstrap/1_beta/LANGUAGE.md).

## One entrance and complete flow

The owner fixes `encode_Beta(raw_source, source_limit, output_limit) = Success(tape)`.
The proposed entrance coordinates these ordinary, total definitions:

1. Check the complete raw source envelope and source capacity.
2. Scan source in order, completing and encoding one token at a time.
3. Flush EOF once, require no missing operands, and propagate failure.
4. Flatten the returned output fragments and return `Success` with those bytes.

The certificate producer supplies explicit equalities for this computation;
the generic checker does not execute it. Root reconstruction reads the owner's
exact raw source, tape, limits, and theory independently of those witnesses.
Keep source/tape identity, framing, and the evaluator-correctness admission
separate from proof production and checking.

## State and output ownership

Use the existing candidate raw-source tree `Empty | Leaf(Byte) | Join(Source, Source)`.
The owner's partition is mechanical over raw bytes, not token or instruction
boundaries. Every recursive scan call consumes an unchanged immediate child.

Incoming scan state contains comment mode, the current reversed token,
operand expectation, checked output count, and sticky failure status. It has
**no completed-token list and no accumulated output bytes**. A successful token
is at most 19 bytes, including a possible assertion colon. The baseline need
not introduce another token-length counter: complete-token parsing rejects
overlong forms. This does not invent a token-capacity exhaustion outcome.
Without early length rejection, malformed pending tokens and their reversals
can grow to the entire admitted source extent. Nineteen bytes bounds valid
tokens only; producer/checker depth, storage, and refusal accounting must cover
the larger malformed-input case, not assume successful-input sizes universally.

Each scan result contains its outgoing state and an output fragment. The proposed
fragment sort is only `Empty | Chunk(ByteList) | Join(Fragment, Fragment)`.
It is not a new source language, compiler IR, or checker primitive. A completed
token emits at most eight bytes; most source bytes emit an empty fragment.
Source Joins combine fragments in source order while threading only the state.
This costs a small constructor vocabulary and explicit projections/composition,
but avoids repeatedly copying output lists or carrying output history in every
closed scan equation. No empty-Join simplifier is required for the baseline.

Flatten only at the final successful result:

```text
flatten(Empty, tail) = tail
flatten(Chunk(bytes), tail) = append(bytes, tail)
flatten(Join(left, right), tail) = flatten(left, flatten(right, tail))
```

`append` recurses on its unchanged ByteList tail; `flatten` recurses on unchanged
Fragment children. No computed cursor or higher-order difference list is needed.
This makes each output byte participate in one chunk append, not one append per
source-tree ancestor. It does not make flattening or output equality free.

## Token and instruction transitions

Keep the complete-token register/HEXWORD parsers; do not introduce a separate
character-level recognizer framework. Reversing the current token is explicit
proof work. Leading zeroes, lowercase spelling, width, and complete exhaustion
remain parser obligations, including cases absent from the current subject.

Operand expectation is one of `Ready`, `R`, `X`, `RR`, `RX`, or `RRX`:

| Expectation | Required next token | Next expectation |
| --- | --- | --- |
| `R` | Complete register | `Ready` |
| `X` | Complete HEXWORD | `Ready` |
| `RR` | Complete register | `R` |
| `RX` | Complete register | `X` |
| `RRX` | Complete register | `RX` |

In `Ready`, a mnemonic selects its exact opcode and operand expectation from
all 21 rows of Beta's instruction table. Emit the opcode immediately, including
`ret` with no operands. `dw` emits no opcode and selects `X`. An address assertion
is legal only in `Ready`, requires its word to equal the current output count,
and emits nothing. Unknown mnemonics and inappropriate token forms are Invalid.

A register emits one byte; a word emits its existing eight-byte little-endian
serialization. Intermediate output is not publication: a later missing or wrong
operand still prevents `Success`, so no whole-instruction buffer is necessary.
Use checked, nonwrapping growth and output-limit checking at emission, with
sticky exhaustion. Checking the limit only at EOF could otherwise change an
earlier capacity refusal into a later syntax failure.

Separators flush pending tokens. Semicolon flushes then enters comment mode;
CR/LF end that mode. EOF flushes once and requires `Ready`, even after a comment.
The separate envelope check covers every raw byte, including a late invalid
comment byte after a parser failure. Invalid source, encoding exhaustion, and
the generic checker's own Incomplete verdict remain distinct. Successful
capacity equality is permitted; overflow or adjacent exhaustion cannot wrap.
The exact total failure-selection equations still need audit against the
selected profile; this design does not change existing compiler diagnostics.

## One cost ledger, with missing quantities visible

Existing observations are anchored at `c727cd624a` on macOS arm64 and described
in the [cost review](bootstrap_cost_review.md#whole-route-comparison-real-state-and-coherent-provisions).
They identify 46,484 raw bytes, 3,686 completed tokens, 582 distinct tokens,
and 8,355 tape bytes. They are not a trace of the encoder proposed here.

| Cost owner | Complete-candidate accounting obligation |
| --- | --- |
| Theory and owner root | All declarations/clauses, full raw source and tape terms, both limits, and exact encoding root. Formation checks unused clauses too. |
| Source admission | Complete ASCII and capacity proofs. Compare the measured 547,817-work shape route with direct counting at 641,773; fusion has no established saving. |
| Stateful scan | At most 46,483 Join occurrences and 46,484 Leaf occurrences before sharing, plus EOF. Cost the actual result/projection recipe, not the old 55-work state-only Join. |
| Token completion | Reversal, exact mnemonic dispatch, whole-token register/word/assertion parsing, and finite expectation transitions. Local token facts may share; state-dependent composition cannot assume that sharing. |
| Emission | Exact serialization, every checked count increment, limit comparisons, assertion comparisons, failure propagation, and returned fragments. |
| Output and finalization | Without empty-Join simplification, at most 92,967 fragment nodes from the source scan, plus bounded EOF composition; chunk appends cover 8,355 bytes. Price every flatten/projection/congruence and final root comparison. |
| Request and execution | Theory + owner/witness terms + proof rows + framing; formation/ground allocation + cumulative checking allocation; source-owned producer and root-reconstruction resources, depth, and time. |

One additional **algebraic recipe scenario**, not a runtime measurement, exposes
the missing emission cost. Repeating the existing Word-successor proof for
counts 0 through 8,354, with no cross-fact reuse, gives:

```text
noncarry increment: 285 + 2*low_byte work; 21 proof rows
one-byte carry:     687 + 2*next_byte work; 30 proof rows
8,323 noncarries + 32 carries: 4,494,077 work; 175,743 proof rows
one proof-index sentinel and final root comparisons: +5 work
```

These charges follow the existing `successor_rows` recipe in the retained
`/private/tmp/omega-beta-hexword.qiJde0/tests/gamma/beta-encoding-theory/counters.py`.
The file is local continuation evidence, not a portable production command.
Independent source/algebra review checked the sum. The five final units do not
connect the individual successor facts into an encoder or output-count proof.
This scenario excludes count-state composition and limit checks. It is neither
a lower bound nor an estimate after sharing, and must not be added to a different
recipe's partial costs as if that produced a measured complete certificate.

The ledger deliberately has **no invented full-work, request-size, memory, or
time total**. The state-only fold measurements do not price returned fragments;
the lexical census does not contain actual encoder states; and no complete
definition package/recipe exists from which to count those records. Two or four
million work is therefore not a justified profile. A larger profile also needs
the framing, allocation-ledger, and native-backing review already identified in
the cost review; a scalar limit edit would not establish containment.

## Integration boundary and scope decision

If the user elects to continue this approach, use **one complete Beta definition
package and integrated recipe** as the next implementation unit, not another
permanent helper experiment. Within `bootstrap/2_gamma/beta_encoding/`, keep the
existing theory entrance; the encoder entrance should sequence admission, scan,
and finalization. Subordinate ownership is source admission, token recognition,
instruction/emission transitions, and output composition. Existing arithmetic
remains shared; create files only when those actual definitions require them.
Owner-root reconstruction and untrusted production remain separate from generic
checking. No new framework, language, proof rule, or host semantic producer is
part of this candidate.

That unit must supply total equations for every Beta case, an explicit integrated
recipe, and one full-subject ledger with sharing assumptions and all missing
costs above. Only then select and validate a coherent execution profile. The
eventual full source-owned certificate and mutation/resource controls still
determine P1 acceptance; finishing the package alone does not close the edge.

**Disposition:** the algorithm is now concrete, but feasibility remains unproved.
Pause implementation and request user scope direction: continue with that
complete integration unit, or defer this proof strategy while preserving the
open P1 obligation and disclosed trust assumptions. Do not delete existing
machinery, substitute execution for proof closure, or silently move to another
language. This is a prioritization checkpoint, not an unresolved language
decision for `OWNER_QUESTIONS.md`.
