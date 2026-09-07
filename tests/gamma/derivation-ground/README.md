# Sorted ground terms gate

Run `sh tests/gamma/derivation-ground/run.sh` from the repository root on macOS
arm64 or Windows x64 in Git Bash. Python 3 and the selected checked-in Alpha seed
are required; macOS additionally requires `codesign`. A portable entrypoint does
not establish Windows runtime validation.

The contract is [GROUND.md](../../../bootstrap/gamma/derivation_checker/GROUND.md),
following [formation](../../../bootstrap/gamma/derivation_checker/FORMATION.md)
and [physical layout](../../../bootstrap/gamma/derivation_checker/LAYOUT.md).
The gate materializes the complete canonical checker implementation with its
explicit diagnostic entry, checks the `source.tsv` identity, and calls the real
`check_derivation_ground()` through the selected Gamma evaluator. It never
extracts production functions or substitutes a host checker. The host reuses
only the layout gate's literal field encoder, frames bytes, and compares exact
observations. Expected coordinates are sums of authored record prefixes, not
decoded requests or production results.

Tag 5 publishes eight little-endian u64 fields: three section ends, owner count,
witness count, left root, right root, and the proof-count field's offset. The
result is exactly 65 bytes. Failure tags 1/2 publish their original four u64
fields in exactly 33 bytes. Every owned diagnostic must return process status
zero and empty stderr. Grounded does not assert equality, check a proof row,
authenticate the theory/proposition, or establish the full Beta encoding root.

The 113 vectors configure 223 observations: 110 small vectors run twice with a
60-second host watchdog, and three larger vectors run once with 600 seconds.
An outer evaluator failure or timeout is never a ground-term judgment.

- `positive.py` covers the FORMAT example, calls of any formed function,
  constructor/function namespaces, multiple sorts, duplicate rows, repeated
  children, and mixed references across separate owner/witness indexes. False
  same-sort roots, invalid proof semantics, and an empty proof table remain
  Grounded because proof checking is later.
- `references.py` covers zero/self/forward/cyclic references, invalid unused
  rows, and owner references that witnesses or clause templates cannot supply.
- `applications.py` checks symbol before arity, each of three argument positions,
  and reference/sort precedence for both namespaces in both tables.
- `roots.py` checks left/right fields, root sorts, empty owners, no witness
  rescue, and owner-before-root-before-witness diagnostic order.
- `forwarding.py` retains physical, formation, and capacity failures unchanged.
  The previously admitted formation fixture with unknown owner function 999 now
  rejects at that symbol's request coordinate 140, before its other defects.
- `large.py` constructs a 46,484-owner-row chain, a 46,484-witness-row chain,
  and a 2,048-row shared DAG whose recursively expanded tree is exponential.
  These are literal reference tables, not host-expanded terms or certificates.

The large requests are respectively 929,772, 929,792, and 49,260 bytes. The owner
chain roots name row 46,484. The witness chain retains owner1 as global1; witness
row j names predecessor global j. The shared DAG repeats its preceding child
twice per row. No new term/depth provision is introduced: the physical request,
formation provision, indexed storage, and tail scans own their existing bounds.
Exact outer/formation boundaries remain covered by their owning gates; this
gate additionally checks selected forwarded refusals.
