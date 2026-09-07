# Finite Beta encoding theory diagnostics

Run `sh tests/gamma/beta-encoding-theory/run.sh` from the repository root on
macOS arm64 or Windows x64 Git Bash, with Python 3 available as `python3`.
The same command applies on both hosts; PowerShell is not required.
Missing Python explicitly skips; unsupported hosts fail with status 2.
The gate prints the executing host. A result on one host does not establish
execution on the other.

For the bounded full-subject representation probe, run
`sh tests/gamma/beta-encoding-theory/run.sh --subject-shape` on either supported
host. This selects [subject_shape.py](subject_shape.py), not the finite-equation
suite. It checks separately encoded copies of the entire current evaluator
source and tape as ByteLists, then requires an exact rejection after changing
the first source byte in the witness. Both rows are literal reflexivity claims;
the final root is tape-list reflexivity, **not** `encode_Beta(source) = tape`.
Host code constructs diagnostic records only. Theory bytes still come from the
pinned Gamma emitter, and all owner/witness rows pass the ordinary checker.
The 300-second per-invocation watchdog grants no verdict on timeout.

The runner prints exact subject identities, request bytes, term counts, checking
work, and elapsed time. At total subject length N, two separately represented
spines require `4*N+13` work: index3, two row checks, two comparisons costing
`4*N+4`, and four final-root transitions. These expectations are derived before
execution, not learned from output. This measures one full-size prerequisite,
not certificate production, Beta semantics, or full encoding-proof feasibility.
See the [cost review](../../../wiki/design_briefs/bootstrap_cost_review.md).

`sh tests/gamma/beta-encoding-theory/run.sh --counter-cost` selects the separate
[counter recipe probe](counter_cost.py). It reuses the existing literal successor
derivations, interns identical ground records across them, and reuses identical
proof rows after remapping their backward references. It performs no theory
interpretation, Beta parsing, or search for alternative proofs. Small combined
requests check 1, 24, and 257 consecutive increments; the last crosses the byte
carry boundary. A corrupted final endpoint must reject after the valid prefix.
The work ledger uses the existing counter recipe's row costs, charging one
combined proof index/root and charging each retained row once.

The final measurement constructs the same shared recipe for one increment per
raw evaluator source byte. It prints term/row/byte counts and the derived work,
but does **not** invoke the checker on that oversized request. This tests one
proposed source-counter strategy, not a lower bound for all encoders. Neither
the small checked examples nor the large constructed table proves source length,
source traversal, or Beta encoding. Retain this probe only while deciding that
strategy; it is not another compulsory component gate or production producer.

The shell entry resolves bootstrap roles, materializes both complete source
closures, and invokes `materialize_gamma_evaluator` for the selected
Beta-authored evaluator. All test logic uses Python's standard library.
For a prepared directory containing `producer.gamma`, `checker.gamma`, and the
materialized `evaluator`, use
`python3 -B tests/gamma/beta-encoding-theory/gate.py PREPARED_DIRECTORY`
(or `python -B` on Windows when that names Python 3). The Python runner does
not reconstruct or replace the evaluator. Both entrypoints propagate failures.

The [producer entry](main.gamma) calls ordinary Gamma
[`beta_encoding_theory`](../../../bootstrap/2_gamma/beta_encoding/theory/theory.gamma)
only for empty input. Its marked application result publishes bytes without a
scalar terminator. Three nonempty inputs require status 1 and empty stdout and
stderr. The exact producer composition is pinned in [source.tsv](source.tsv).
The unmodified checking entry and its identity are reused from
[derivation-checking](../derivation-checking/README.md).

The emitted complete GTH1 section contains a partial Beta theory. Its byte
length and SHA-256 must match [theory.tsv](theory.tsv) and the independently
authored fixed layout in [identity.py](identity.py); a second Gamma invocation
must emit identical bytes. The host layout computes only a diagnostic length
and digest. Every checker request uses actual Gamma stdout as its theory,
except the explicitly corrupted copy in one negative control. No host-produced
package is admitted as checker input. Identity is not a proof rule or artifact
admission, and these fixtures contain no general Beta parser or proof producer.

The finite equations transcribe [Beta's language contract](../../../bootstrap/1_beta/LANGUAGE.md):

| Fixture family | Positive equations | Explicit proof rows | Expected work |
| --- | ---: | ---: | ---: |
| [Lexical](lexical.py) | 1,024 | 1,024 | 137,781 |
| [Nibble joins](nibbles.py) | 256 | 768 | 10,757 |
| [Byte nibble splits](nibbles.py) | 512 | 512 | 68,869 |
| [Split/join composition](roundtrip.py) | 256 | 1,792 | 84,741 |
| [Eight-byte word emission](words.py) | 13 | 13 | 928 |
| [Byte counter helpers](counters.py), four batches | 512 | 128 per batch | 9,029 or 25,413 |
| [Checked word successor](counters.py), nineteen requests | 19 | 21–84 per request | 290–3,341 |
| [Unsigned nibble comparison](ordering/nibbles.py) | 256 | 768 | 10,757 |
| [Unsigned byte comparison](ordering/bytes.py), twelve requests | 12 | 19 per request | 204–1,284 |
| [Unsigned word comparison](ordering/words.py), thirty requests | 30 | 177 per request | 2,004–10,644 |

Each family uses a separate checker request. All rows, including those unused
by the final owner equation, must check. Lexical coverage retains every byte of
source admission, separators, comment endings, and lowercase hex classification.
Every nibble pair joins through an explicit public-to-helper Unfold,
helper-to-byte Unfold, and Transitivity. Every byte is split both ways. Composed
roundtrips add two split Unfolds, Congruence using their two ordered premises,
and a final Transitivity; the checker never implicitly normalizes arguments.
Word cases include zero, maximum, the highest bit, two varied byte patterns,
and a single nonzero byte in each of all eight positions. The expected byte
lists are authored least-significant byte first.

Counter cases explicitly state both eight-byte input and expected result.
They cover zero, a varied no-carry value, two sets of carry lengths one through
seven with varied unaffected bytes, crossing the highest bit, maximum minus one
to maximum, and maximum to `Overflow`. A successful result is `WordValue(Word)`;
the maximum must never wrap to `WordValue(0)`. Four requests of 128 equations
cover every clause of the internal byte increment and maximum predicate. The
internal byte increment wraps 255 to zero; the public word successor selects
Overflow before an eight-byte wrapped value can become its result.

The fixed carry derivation writes each helper Unfold, predicate Unfold, two
Reflexivity rows for the choice branches, ordered choice Congruence, choice
Unfold, two Transitivity rows, and a Transitivity connecting that helper to the
public root. The selected successful branch then supplies byte increment
Unfold, seven byte Reflexivity rows, eight-argument Word Congruence, unary
WordValue Congruence, and final Transitivity. These are finite authored proof
families over the listed literals; no code reads theory clauses to infer a
rewrite, recursively normalizes terms, or constructs a proof of arbitrary input.

Ordering has three results: `Less`, `Equal`, and `Greater`. The fixed nibble
table covers all 256 pairs. Twelve byte examples include equal values, zero,
maximum, a higher nibble opposing the lower nibble, a matching higher nibble
requiring lower-nibble comparison, and values on either side of the unsigned
high bit. Thirty word examples include equal zero/maximum/varied words, both
directions across the high bit, and each highest differing byte position in
both directions. Lower bytes deliberately oppose the highest difference while
higher bytes agree. Separate literal rows compare the exact Beta source limit
`0x4000000` and output limit `0xfffffc`, and the adjacent values on each side,
against those limit words. These are numeric comparisons, not source or output
accounting judgments.

The [fixed ordering recipes](ordering/proofs.py) explicitly unfold both nibble
splits, both public/helper nibble comparisons, and every choice. They supply
ordered Congruence and Transitivity rows for each argument rewrite. A byte
proof always normalizes both nibble comparisons. A word proof always supplies
all eight byte proofs, then seven fixed bottom-up ordering choices, then the
two word entrance unfoldings and their composition. Nothing inspects emitted
definitions to select a rewrite, searches for proofs, or recursively normalizes
arbitrary terms. Word and byte owner outcomes are separately stated literals.

Negative controls preserve the lexical wrong answers, wrong clauses, altered
formed body, and wrong owner root. New controls corrupt join answers and
clauses, swap high/low results, omit the helper step, corrupt transitivity,
swap congruence premises, and attempt case selection before normalization.
Word controls reverse byte order, shorten and lengthen the output list, lose
the high bit, truncate the maximum, corrupt each byte position, and select an
invalid clause. Expected rejection coordinates come from physical record
sizes, never decoded or learned checker observations.

The 181 vectors also include seven-field and nine-field Word constructor
applications. These are physically complete records that must reject during
ground admission with code 8 at the argument-count field, independently of
the separate seven-byte and nine-byte output-list mutations.

Counter mutations corrupt the clause in every byte-helper batch, attempt choice
selection before normalizing its predicate, omit choice Congruence, substitute
the wrong condition premise, omit increment normalization, and corrupt each
result byte in a carry-three and highest-bit-crossing derivation. The maximum
case explicitly rejects a proposed wrapped zero result. Their exact failure
coordinates use the authored record lengths, including earlier valid rows.

Ordering adds 32 mutation controls. They reverse a nibble answer, attempt to
skip its helper, select wrong nibble and word clauses, reverse high-nibble
priority, interpret the high bit as a sign bit, ignore the highest differing
word byte, omit normalization or Congruence, swap ordered premises, and change
the owner root after an otherwise valid proof. All 106 earlier vectors remain.

The work expectations follow the generic checker accounting. Every batch pays
`P+1` for its proof index and four for its final root. Lexical clause walks cost
`4*(1+...+256)`; row/index/nullary substitution overhead is five per row, with
three extra units for each of sixteen Hex results. Splits use the same nullary
overhead and two complete byte clause walks. Each join costs `high+low+24`:
public unfolding costs `high+11`, helper unfolding `low+6`, and Transitivity
seven. Each composed roundtrip costs `2*byte+high+low+54`, including eleven
units for its two-premise Congruence and seven for final Transitivity. Each
word Unfold costs 70: one row reservation, one clause walk, eighteen template
index units, 34 substitution transitions, and sixteen variable ground-comparison
transitions. Shared closed ground references make each structural endpoint
comparison exactly two units. These are independently derived expectations;
the runner requires the exact tag-7 count/work observation.

For counter byte helpers, each row costs `byte+6`; a 128-row request adds 129
for indexing and four for the root. Public word unfolding costs 46: one row,
one clause, ten index units, eighteen substitution transitions, and sixteen
variable comparison transitions. Each lower carry-helper unfolding costs 80:
one row, one clause, sixteen index units, 46 substitution transitions across
22 child edges, and sixteen variable comparison transitions. Its highest-byte
counterpart costs 63, with fifteen index units and thirty transitions across
fourteen edges. Repeated variable templates under the same ground reference
use the invocation-local memo; each of eight distinct variable templates still
requires its first two-unit ground comparison.

A predicate row costs `byte+6`; the two branch Reflexivity rows cost six,
choice Congruence sixteen, choice unfolding eight for False or nine for True,
and the three Transitivity rows cost 21. Successful final normalization costs
`byte+81`: byte unfolding, seven Reflexivity rows (21), Word Congruence (41),
WordValue Congruence (six), and final Transitivity (seven). Including the proof
index and root, an input carrying `c=0..6` bytes and then incrementing byte `b`
uses `21+9*c` proof rows and `290+402*c+2*b` work. Carrying seven bytes uses
84 rows and `3087+2*b` work. Maximum overflow uses 73 rows and 3,251 work.
These equations were fixed from checker/source accounting before execution.

For ordering, each nibble pair `(l,r)` uses three rows costing `l+r+24` before
index/root work. A normalized high- or low-nibble comparison of bytes `(a,b)`
uses seven rows costing `a+b+l+r+54`. The byte definition Unfold costs 38:
one row, one clause, ten template-index units, 22 substitution transitions
across ten child edges, and four variable comparison transitions. Its two
variable templates each incur one ground comparison; later occurrences reuse
the invocation-local memo. The ordering choice costs six for Less, nine for
Equal, or eight for Greater, including its row and clause walk. Its Equal
clause compares the bound lower result; the other two have constant bodies.

Let `q(L)=6`, `q(E)=9`, and `q(G)=8`. A nineteen-row byte proof has row work
`B(a,b)=2*a+2*b+ah+al+bh+bl+171+q(high_order)`, where the four nibble values
come from the finite split cases. Its exact checker work adds 24 for its index
and final root. Word public unfolding costs 51: one row, one clause, eleven
index units, twenty substitution transitions, and eighteen variable comparison
transitions. Right-word unfolding costs 128: one row, one clause, 32 index
units, 62 substitution transitions across thirty edges, and 32 variable
comparison transitions. All sixteen variable templates are distinct even when
their ground bytes coincide.

Each of the seven word choice levels costs `18+q(byte_order)`: binary
Congruence eleven, choice unfolding, and Transitivity seven. Two entrance
Transitivity rows cost fourteen. Thus every word request has 177 proof rows
and exact work `375 + sum(B(left[i],right[i]), i=0..7) +
sum(18+q(byte_order[i]), i=1..7)`. The constant 375 includes both entrance
unfoldings, their Transitivity rows, proof indexing, and final root comparison.
This gives 2,004 work for equal zero words and 10,644 for equal maximum words.
Ordering requests contain at most 531 ground terms, 768 proof rows, 10,757 work,
and 121,144 request bytes. All formulas and expected failures were fixed from
the checker rules and source templates before runtime observations.

Rejections require the exact 33-byte owned diagnostic, process zero, and empty
stderr. Timeouts and process failures never count as proof results. Request
sizes and elapsed times are printed per vector; none establishes full-certificate
size or runtime. This gate proves only these finite equations under the fixed
partial theory. Token scanning, word parsing, opcodes, source/output accounting, full
Beta reconstruction, and accepted artifact custody remain outside its claim.
It does not close the complete obligation in the
[derivation calculus](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md).
