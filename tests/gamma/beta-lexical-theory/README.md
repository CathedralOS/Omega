# Finite Beta lexical theory diagnostics

Run `sh tests/gamma/beta-lexical-theory/run.sh` from the repository root on
macOS arm64 or Windows x64 Git Bash, with Python 3 available as `python3`.
The same shell command applies on both hosts; PowerShell is not required.
Missing Python explicitly skips; unsupported hosts fail with status 2.
The gate prints the host that actually executes the checks. A result on one
host does not establish execution on the other.

The shell entry resolves bootstrap roles, materializes both complete source
closures, and invokes `materialize_gamma_evaluator` for the selected
Beta-authored evaluator. All test logic uses Python's standard library.
For an already prepared directory containing `producer.gamma`, `checker.gamma`,
and the materialized `evaluator`, the direct Python route is
`python3 -B tests/gamma/beta-lexical-theory/gate.py PREPARED_DIRECTORY`
(or `python -B` on Windows when that names Python 3). The Python runner does
not reconstruct or replace the evaluator. Both entrypoints propagate failures.

The [producer entry](main.gamma) calls ordinary Gamma
[`beta_lexical_theory`](../../../bootstrap/gamma/beta_encoding/lexical_theory/theory.gamma)
only for empty input. It returns application success without appending a
terminator. Three nonempty inputs require status 1 and empty stdout/stderr.
The exact producer composition is pinned in [source.tsv](source.tsv).
The unmodified checking entry and its source identity are reused from
[derivation-checking](../derivation-checking/README.md).

The emitted GTH1 package must have the fixed byte length and SHA-256 in
[theory.tsv](theory.tsv), and a second source invocation must emit identical
bytes. This pin was independently calculated from the authored package layout.
Every positive checker request contains the actual emitter stdout. The host
does not construct, parse, evaluate, or repair its theory. The fixed digest
is a diagnostic custody comparison, not a checker rule or artifact admission.

[fixtures.py](fixtures.py) transcribes the finite tables in
[Beta's lexical contract](../../../bootstrap/beta/LANGUAGE.md#lexical-form).
Its literal wire examples state all 256 byte cases of each of four functions:
source-byte admission, separators, comment terminators, and lowercase hex
digits with their nibble values. All 1,024 Unfold rows occur in one request;
the owner root is the last equation. The source-owned generic checker must
check every row, including the 1,023 rows not used by that final equation.
These are authored finite diagnostic instances, not a general host proof
producer, source parser, or encoding certificate.

There are 17 checker vectors: the complete truth table, ten wrong-answer
mutations, four invalid or mismatched clauses, one altered but still formed
theory body, and a wrong owner root after an otherwise valid table. Controls
include uppercase hex, a wrong nibble, DEL, comma, semicolon, tab, CR, and an
invalid final byte-255 row after 1,023 valid rows. The theory mutation changes
the source-byte-zero clause from False to True; the unchanged stated truth
must reject. Expected failure coordinates are authored physical prefix sizes,
not decoded or learned checker output.

The independent work expectation is 137,781 units. Proof indexing costs 1,025;
four ordered clause walks cost `4 * (1 + ... + 256) = 131,584`. Each row adds
one rule reservation, two template-index units, and two nullary substitution
transitions: 5,120 total. The sixteen Hex bodies add one index unit and two
substitution transitions each: 48 total. Final root comparisons cost four.
The expected observation is tag 7 followed by count 1,024 and work 137,781 as
two little-endian u64 values. All rejections require the exact 33-byte owned
failure diagnostic, process zero, and empty stderr. Timeouts and process
failures do not count as proof results.

This gate establishes only the finite lexical equations under this fixed
theory. Comment scanning, complete tokens, words/registers, opcodes, encoding,
limits, full Beta reconstruction, and accepted artifact custody remain outside
its claim. It does not close the complete certificate obligation described in
the [derivation calculus](../../../wiki/architecture/bootstrap_chain/derivation_calculus.md).
