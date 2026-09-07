# Checked template substitution gate

Run `sh tests/gamma/derivation-substitution/run.sh` from the repository root on
macOS arm64 or Windows x64 in Git Bash. Python 3 and the selected checked-in Alpha
seed are required; macOS additionally needs `codesign`. Windows runtime validation
is not implied by this portable entrypoint.

The contract is [SUBSTITUTION.md](../../../bootstrap/gamma/derivation_checker/SUBSTITUTION.md),
following [COMPARISON.md](../../../bootstrap/gamma/derivation_checker/COMPARISON.md).
The gate materializes the complete production closure once. Each diagnostic is
the exact concatenation of `diagnostic.gamma`, a whole `entries/<name>.gamma`,
and that closure. `source.tsv` pins each composition in `fixtures.py` order.
Host code reuses only the layout gate's literal field encoder; it does not
decode terms, construct binding maps, perform substitution, or decide equality.

All entries first call the real ground-admission stage and forward failures.
One session is created and threaded forward; no failed session is resumed.
Compared observations are tag 6 followed by Boolean and cumulative work as two
little-endian u64 values (17 bytes). Rejected/Incomplete observations are tag 1/2
and four u64 fields (33 bytes). Process zero and empty stderr are required.
False means the substituted template differs structurally, not malformed proof
data. Proof semantics remain unchecked, but physical proof errors still reject.

The explicit entries and caller coordinates are:

| Entry | Source-owned calls |
| --- | --- |
| `root`, `case` | Unfold checked owner roots using ordinal 1 or ordinal 2. Coordinates are 901/902/903 for left/right/clause. |
| `clause` | Same roots; ordinal is right-root identity minus 1, testing invalid local ordinals. |
| `invalid` | Checked root selectors 1/2/3 choose operands 0/N+1/1, then make one unfolding call. |
| `witness` | Unfold global N against N−1, selecting the final witness function and preceding witness target. |
| `session` | Unfold 2→1; structurally compare 2/1; unfold 4→1 then 4→3. Expected cumulative outcomes true 7 / false 8 / false 13 / true 20. |
| `retention` | Unfold 6→7, compare ground 3/4, unfold 6→7 again. Expected false 14 / true 16 / false 28. |
| `bulk` | Right-root selector 1 starts empty, 2 reserves 262144, 3 reserves 1; left-root selector chooses the next amount at coordinate 907. |
| `budget` | Reserve a fixed amount, then unfold identity 2→1; selectors 5/6 replace the left/right operand with 0. Successful exact unfolding is followed by a one-unit reserve at 907. |

Bulk left-root selectors 1..8 choose respectively `0`, `-1`, `2147483648`,
`2147483647`, `262144`, `262145`, `1`, and `INT64_MIN`. Invalid amounts reject
with code 11 even after exhaustion. A maximum valid amount after one consumed
unit requests 2,147,483,648 without truncation. These selectors are explicit test
coordination over already checked root identities, not a new request format.

Step derivations are independent of the implementation:

- A mode-zero identity with one variable row costs ordinal 1 + index 2 + template
  visit/resume 2 + ground identity comparison 2 = 7. A constant costs 5.
- A mode-one ordinal 2 variable costs 8; its constructor identity may instead be 3.
  A two-row table whose body uses an unselected parameter costs 9.
- A false template retains completed ground-child memo entries, but each new
  unfolding has fresh clause/binding-local memo state. Definitional equality
  must never populate the structural `(left,right)` memo.
- Pre-reservation 262137 permits identity unfolding to finish at 262144. At 262138,
  the variable's ground comparison finishes at the limit but template terminal
  resume refuses. At 262143, the index bulk request is exactly 262146; at 262144,
  clause scanning refuses at clause coordinate 903. Invalid IDs still reject first.
- A 46,484-row unary template with variable base costs `3T+4 = 139456`; the request
  is 1,859,508 bytes. A 1,024-row shared binary DAG costs `5T+2 = 5122`; its request
  is 49,312 bytes. Neither path expands a logical tree on the host or native stack.

There are 57 vectors and 112 observations: 55 small vectors run twice under a
60-second host watchdog; the two larger vectors run once under 600 seconds.
These are watchdogs, not language bounds or owned refusals. Coverage includes
mode 0 / mode 1, local clause ordinals, unchanged child and other-parameter bindings,
incorrect target children, constructor/function heads, cross-space row-number
collisions, environment changes, witness custody, physical-error forwarding,
and semantically invalid but physically complete proof rows. This gate does not
validate derivations or accept the complete Beta encoding certificate.
