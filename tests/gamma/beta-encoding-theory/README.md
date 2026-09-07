# Finite Beta encoding theory diagnostics

Run `sh tests/gamma/beta-encoding-theory/run.sh` from the repository root on
macOS arm64 or Windows x64 Git Bash, with Python 3 available as `python3`.
The same command applies on both hosts; PowerShell is not required.
Missing Python explicitly skips; unsupported hosts fail with status 2.
The gate prints the executing host. A result on one host does not establish
execution on the other.

The shell entry resolves bootstrap roles, materializes both complete source
closures, and invokes `materialize_gamma_evaluator` for the selected
Beta-authored evaluator. All test logic uses Python's standard library.
For a prepared directory containing `producer.gamma`, `checker.gamma`, and the
materialized `evaluator`, use
`python3 -B tests/gamma/beta-encoding-theory/gate.py PREPARED_DIRECTORY`
(or `python -B` on Windows when that names Python 3). The Python runner does
not reconstruct or replace the evaluator. Both entrypoints propagate failures.

The [producer entry](main.gamma) calls ordinary Gamma
[`beta_encoding_theory`](../../../bootstrap/gamma/beta_encoding/theory/theory.gamma)
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

The finite equations transcribe [Beta's language contract](../../../bootstrap/beta/LANGUAGE.md):

| Fixture family | Positive equations | Explicit proof rows | Expected work |
| --- | ---: | ---: | ---: |
| [Lexical](lexical.py) | 1,024 | 1,024 | 137,781 |
| [Nibble joins](nibbles.py) | 256 | 768 | 10,757 |
| [Byte nibble splits](nibbles.py) | 512 | 512 | 68,869 |
| [Split/join composition](roundtrip.py) | 256 | 1,792 | 84,741 |
| [Eight-byte word emission](words.py) | 13 | 13 | 928 |

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

Negative controls preserve the lexical wrong answers, wrong clauses, altered
formed body, and wrong owner root. New controls corrupt join answers and
clauses, swap high/low results, omit the helper step, corrupt transitivity,
swap congruence premises, and attempt case selection before normalization.
Word controls reverse byte order, shorten and lengthen the output list, lose
the high bit, truncate the maximum, corrupt each byte position, and select an
invalid clause. Expected rejection coordinates come from physical record
sizes, never decoded or learned checker observations.

The 51 vectors also include seven-field and nine-field Word constructor
applications. These are physically complete records that must reject during
ground admission with code 8 at the argument-count field, independently of
the separate seven-byte and nine-byte output-list mutations.

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

Rejections require the exact 33-byte owned diagnostic, process zero, and empty
stderr. Timeouts and process failures never count as proof results. Request
sizes and elapsed times are printed per vector; none establishes full-certificate
size or runtime. This gate proves only these finite equations under the fixed
partial theory. Token scanning, word parsing, counters, opcodes, limits, full
Beta reconstruction, and accepted artifact custody remain outside its claim.
It does not close the complete obligation in the
[derivation calculus](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md).
