# Conservative theory formation gate

The governing contract is [FORMATION.md](../../../bootstrap/gamma/derivation_checker/FORMATION.md),
with literal fields defined by [FORMAT.md](../../../bootstrap/gamma/derivation_checker/FORMAT.md).

Run `sh tests/gamma/derivation-formation/run.sh` from the repository root on
macOS arm64 or Windows x64 in Git Bash. Both routes require Python 3 and the
selected checked-in Alpha seed; macOS also requires `codesign`. Windows runtime
validation is not implied by the portable entrypoint.

The gate materializes the complete canonical checker source with one explicit
diagnostic entry and verifies its `source.tsv` identity. The selected Gamma
evaluator calls the actual `form_derivation_theory()`. No production functions
are extracted and no host decoder, sort checker, termination checker, proof
producer, or replacement implementation is used. `theory_wire.py` reuses only
the neighboring layout gate's literal field encoders. Coordinates come from
explicit authored record prefixes, not inspection of decoded requests.

The diagnostic returns process status zero only to publish an owned observation:
tag 4 followed by six little-endian u64 fields (the three frame ends, S, C, F),
or failure tag 1/2 followed by four u64 fields (code, coordinate, limit,
requested). Success is exactly 49 bytes and failure exactly 33 bytes; stderr
must be empty. Formed does not mean an accepted proof or an authoritative Beta
definition package. Physically valid but invalid ground terms, proof premises,
and roots deliberately do not prevent this stage's success.

The concept-owned fixture files cover:

- `positive.py`: the FORMAT identity example, finite constructor inhabitants,
  reverse and productive cyclic dependencies, prior helpers, nested/shared
  templates, decreasing self-calls, and clause-local parameter/child slots.
- `signatures.py`: all signatures before inhabitation/bodies, field ordering,
  undeclared sorts, and uninhabited self/mutual cycles.
- `clauses.py`, `applications.py`, `decrease.py`: exact ordered coverage, scoped
  slots and references, every checked row including unused ones, application
  symbol/order/arity/sort precedence, and structural rather than row-number
  decrease. Reconstructed, computed, parent, and other-parameter values cannot
  justify recursion.
- `forwarding.py`: outer/layout failures precede semantic and resource checks.
- `resources.py`: exact/adjacent sort and work provisions, the deepest sort-mark
  path, the function/constructor work term, a refusal exceeding u32, and a
  46,484-row local template chain with backward children and no recursive host
  expansion.

The work boundary is the documented estimate
`E = (S+1)*(C+A) + F*C + 4W + S`, not runtime instructions or elapsed time.
Two small literal theories establish its exact boundary:

| S | C | A | W | E | Request bytes | Expected |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 64,030 | 2 | 128 | 137 | 8,388,608 | 604 | Reject 7 at first result-sort field 40 |
| 52,753 | 10 | 148 | 181 | 8,388,609 | 780 | Incomplete 3 at 28, requested 8,388,609 |

Both have an invalid first constructor result. Equality must reach that semantic
failure; one above must stop at preflight. The S=65,536/C=65,536 case requests
4,295,884,812 work units, proving the failure codec does not truncate to u32.
The final-sort mark control uses S=65,536 and one nullary constructor of sort
65,536; it traverses all 16 mark-tree levels before rejecting uninhabited sorts.
The template-chain case remains within the documented work and request budgets.

There are 104 vectors and 205 exact observations: 101 small vectors repeat twice
under a 60-second host watchdog; three larger vectors run once under 600 seconds.
A timeout or outer evaluator failure is not a formation judgment. The gate does
not accept a full encoding certificate or validate downstream proof rules.
