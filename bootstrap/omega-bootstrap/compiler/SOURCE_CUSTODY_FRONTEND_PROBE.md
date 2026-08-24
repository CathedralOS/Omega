# Source-custody frontend cost probe

This contract bounds the first checkpoint-driven implementation measurement for
`omega-bootstrap`. It is not an `Ωself` admission, an artifact format, or a
replacement for the canonical source bundle.

## Claim

`omega-bootstrap-source-custody-check.alp` reads one ordinary Omega source unit
from standard input and checks the compositional source families isolated by
`compiler/psi/source/source.omg`:

- named record data, `[copy]`, named fields, and forward nominal references;
- `u8`, `u32`, `bool`, fixed arrays, `in Trapping`, and literal inclusive scalar
  ranges;
- attached machines, mutable/shared `self`, named states, and typed parameters;
- field and indexed-field reads and assignments;
- integer and Boolean literals, `+`, `<`, member access, and indexing;
- exhaustive Boolean guarded transitions to named states; and
- scalar or Unit state results.

The checker resolves names and types, enforces receiver mutability and copyable
nominal assignment, checks transition target arity/result agreement, and carries
the true branch of a `<` guard into the target state as a bounded interval fact.
That fact must suffice for the actual unit's guarded array accesses and
`length + 1` assignment.

`in Trapping` is retained as a type constraint, not discarded as parser trivia.
It is valid only on `u8`, `u32`, and fixed-array types in this profile; applying
it to `bool` or a nominal record rejects. The canonical CKIR type row carries
the corresponding flag.

The rules are name- and order-independent. The implementation may retain source
spans in fixed tables, but it must not recognize the current declaration names,
counts, or syntax-tree permutation.

## Deliberate boundary

The probe consumes one raw unit so it measures the new grammar and static
semantics without duplicating the already-gated bundle decoder and UTF-8 source
custody machinery in another Delta program. The eventual admitted bridge must
compose these rules with the canonical bundle frontend.

Success exits 0 and publishes no bytes. Unsupported, malformed, or ill-typed
source exits 251 and publishes no bytes. Declared resource exhaustion exits 252
and publishes no bytes. No successful result is a Terminal-Psi or executable
artifact claim.

Delta's present integer carrier is signed `i32`. The probe recognizes Omega
`u32` type identity but admits only nonnegative literal/range endpoints through
2,147,483,647 and interval obligations expressible within that carrier. A
program requiring a larger authored `u32` value is unsupported status 251; it
is not silently wrapped or treated as proof of full-width arithmetic. The
actual unit's largest endpoint is 65,536. Full-width representation remains a
later bridge-cost row if product source requires it.

## Resource contract

The probe enforces the applicable public checkpoint ceilings rather than the
counts in the first fixture:

| Resource | Accepted ceiling | Adjacent rejection |
| --- | ---: | ---: |
| source bytes in the unit | 131,072 | 131,073 |
| root items | 128 | 129 |
| fields per data item | 64 | 65 |
| states per machine | 128 | 129 |
| parameters per state or entry | 8 | 9 |
| statements per state or entry | 32 | 33 |
| identifier bytes | 64 | 65 |
| expression depth | 8 | 9 |
| fixed-array length | 65,536 | 65,537 |

Internal tables must be large enough that source satisfying these limits and the
source-byte ceiling cannot hit an undocumented smaller cap. Exact-limit and
adjacent-over-limit fixtures gate each independently where the grammar permits.

The aggregate backing follows from those public bounds and the source-byte
ceiling:

- 8,192 field rows equal 128 roots times 64 fields;
- 16,384 state rows exceed 128 machines times 127 explicit states (the entry is
  the public machine's 128th state but needs no state-table row);
- 32,768 parameter rows exceed the number expressible in 131,072 bytes because
  every stored non-receiver parameter costs at least five disjoint source
  bytes; and
- 32,768 type rows use the same source charging. Even the densest two-row array
  type costs at least eight disjoint bytes in its field, parameter, or result
  context, while every scalar/nominal row and implicit machine-self row costs
  at least four.

These are proof obligations on the fixed backing, not permission to expose the
aggregate constants as additional public ceilings.

Checkpoint resource `path.components` counts identifiers in a qualified name
path. It does not count postfix member/index expression nodes. This first unit
does not require general qualified paths, so the probe does not claim or fake a
separate path-component tooth. Member and index nodes remain covered by the
normalized expression-depth ceiling: each suffix adds one AST level.

## Evidence required for this probe

1. The exact `compiler/psi/source/source.omg` unit checks successfully.
2. Renamed and declaration/state-reordered equivalents check through the same
   implementation.
3. Phase-isolated syntax, duplicate/unknown name, type, mutability, copy,
   transition, guard, result, and bounds failures return 251 with empty output.
4. Every resource row has exact-limit and adjacent-over-limit teeth returning
   252 with empty output.
5. Native Rust-on-ramp-built and Delta-`lowermachine`-built checkers agree.
6. The checker elaborates through the Beta-written Delta-to-Gamma route and the
   canonical Gamma interpreter agrees on the exact unit plus representative
   251 and 252 observations.

## Measured result

The implemented checker is 1,313 lines / 78,450 source bytes. Its fixed tables
reserve 5,395,760 zero-initialized bytes in the current ARM64 image. The focused
native gate carries 34 actual, renamed/reordered, phase-isolated, exact-limit,
and adjacent-limit observations in about 0.15 seconds; six representative
cases repeated through a lowermachine-built checker take about 0.01 seconds.

The Beta-written elaborator produces 626,059 bytes of canonical Gamma, below
the explicit 1 MiB ceiling, in about one second. Canonical Gamma interpretation
is the expensive evidence: approximately two minutes for the exact 1,614-byte
product unit, 12 seconds for an unguarded-index rejection, and three seconds for
declared array-length exhaustion. The meaning gate therefore runs the exact
unit and the two distinct failure classes once; the equivalent renamed positive
is already repeated through native and lowermachine-built checkers.

This proves that the isolated record/array/attached-machine frontend families
are implementable in current Delta with explicit fixed backing. It also prices
the result as a substantial checker (especially under canonical reference
interpretation). It does not retain those families in final `Ωself`, select a
bridge artifact representation, or admit the incomplete `u32` carrier above.

Artifact layout, native lowering, and runnable behavior belong to the
separately tracked artifact tranche, whose first finite, acyclic, returning
source→CKIR1→limited-ELF relation is now closed by independent persisted-Beta
checkers. Current Terminal-Psi vocabulary 28 lacks the general structural
scalar load/store/copy and runtime-index operations needed by this source
family; the tranche uses a private checked-IR handoff and does not silently
widen Terminal Psi or admit the source family to `Ωself`.
