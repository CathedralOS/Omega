# Structural comparison gate

Run `sh tests/gamma/derivation-comparison/run.sh` from the repository root on
macOS arm64 or Windows x64 in Git Bash. Python 3 and the selected checked-in
Alpha seed are required; macOS also requires `codesign`. Windows runtime
validation is not implied by this portable entrypoint.

The contract is [COMPARISON.md](../../../bootstrap/gamma/derivation_checker/COMPARISON.md).
The gate materializes the complete canonical checker implementation once. Each
source is the exact concatenation of `diagnostic.gamma`, one whole named file
in `entries/`, and that implementation. `source.tsv` pins every composition in
the explicit `fixtures.py` entry order. No functions are extracted. Host code
only authors literal wire fields, frames inputs, and compares exact observations;
it never traverses terms to decide equality or produces a semantic proof.

Each entry first admits Grounded and forwards earlier failures unchanged.
Compared is tag 6 plus Boolean and cumulative steps as two little-endian u64
words: 17 bytes. A failure is tag 1/2 plus four u64 words: 33 bytes. Process zero
and empty stderr are required. A false Compared result is ordinary structural
difference, not a rejection. Definitions are not evaluated: `identity(zero)`
and `zero` differ here even though a later unfolding proof could equate them.

The source-owned entries are:

| Entry | Comparison sequence |
| --- | --- |
| `root` | Checked owner left/right roots; one observation. |
| `session` | Roots, repeated ordered pair, reversed pair; one threaded session. |
| `witness` | Left owner root versus last global identity, repeated and reversed. |
| `retention` | Fixed `(6,7)`, `(3,4)`, `(6,7)`, `(4,3)`; cumulative 6/8/12/16 steps prove child memo retention and no false-parent memo. |
| `invalid` | Checked root values 1/2/3 select test operands 0/N+1/1; one failing call at literal caller coordinates 701/709. |
| `budget` | 131,072 same-ID calls reach 262,144; right root1 selects invalid IDs, root2 the next valid call. Publishes exact-bound result, then failure. |
| `resume` | 131,071 same-ID calls and one head mismatch reach 262,143; the next visit fits but terminal resume refuses. |
| `pending` | 131,071 same-ID calls, then two distinct unary parents with one shared child; parent and child visits fit, pending-parent resume refuses. |

Every returned session is threaded forward. No call continues after failure or
restarts a session within a request. Invalid identities are checked before
identity shortcuts and exhausted counters. The last two controls publish no
Boolean or session for the interrupted comparison; `resume` publishes only its
earlier mismatch before that failure. Test-selector roots add no request fields
and are not part of the production comparison interface.

There are 39 literal vectors and 72 observations: 33 small vectors run twice
under 60-second host watchdogs; six larger controls run once under 600 seconds.
Large inputs include two distinct 46,484-node chains (92,968 transitions) and
separately encoded 1,024-level shared DAGs (4,094 transitions). These counts
follow the documented visit/resume schedule: two per unary node; the shared
nullary base costs two and each additional binary level costs four because its
second child is memoized. No recursive host expansion or comparison is used.

Coverage includes ordered children, same-head false suffixes, tag/symbol
mismatch, duplicate structure, cross-owner/witness keys, reversed ordered memo
keys, failed-parent custody, and earlier physical/formation/ground failures.
Host timeouts and outer evaluator failures are not comparison judgments. This
gate does not implement substitution, validate proof rows, or accept the full
Beta encoding certificate.
