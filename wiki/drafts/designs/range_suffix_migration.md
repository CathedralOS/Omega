# Scalar range contract migration

Companion recipe for **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. Commit
`669925b8b9` originally declared the suffix revoked without owner authority; the
owner has since selected the no-suffix design explicitly. The decision is now
valid, but prior migrations remain unaudited because many generated nominal
domains where a contextual proposition was intended.

The removed spelling is `T [min..=max]` / `T [min..max]` on scalar type
positions. One proposition surface replaces it: data/common fields use data
`where`, case payloads use case `where`, machine/state parameters use
`requires`, results use `ensures`, and locals retain facts established by their
initializer, guards, and calls. `T in Domain` is not generic range sugar; use it
only when that exact named domain identity is independently part of the API.

Delete this recipe once every historical migration is audited, all source uses
the canonical clauses, and the parser/representation path is removed.

## Decision table by position

| Old spelling | New spelling | When |
|---|---|---|
| `-> T [lo..=hi]` | `-> T` + `ensures result >= lo && result <= hi` | Result bounds are published postconditions. Use `-> T in D` only when the exact named domain is independently required. |
| parameter/state parameter `x: T [lo..=hi]` | `x: T` + `requires x >= lo && x <= hi` | Callers prove callable preconditions; ranges are not parameter type identity. |
| common field `f: T [lo..=hi]` | `f: T` plus data `where f >= lo && f <= hi` | The relation is part of the containing value's default domain. |
| case payload `case X(v: T [lo..=hi])` | `case X(v: T) where v >= lo && v <= hi` | The relation exists only for the active case. |
| local `let v: T [lo..=hi] = source` | `let v: T = source` | The initializer must already establish the facts. Preserve a needed bound through the producer's contract or a checked guard; the annotation cannot assert it. |
| `T [lo..hi]` (exclusive) | `value >= lo && value < hi` | Use the owning `where`/`requires`/`ensures` clause. |
| parameter `x: T [0..=self.n]` | `x: T` + `requires x <= self.n` | Runtime-dependent endpoints are ordinary call propositions. |
| generic range shell used to infer `N` | explicit binder/equation, or intentional indexed domain | Anonymous interval propositions are not structural type identity. Supply `N` explicitly when no exact declaration structure determines it. |

## Positions that reject instead of migrating

Layout-determining uses — array extents, `const` positions, specialization keys —
require ordinary static values. A runtime range proposition does not make its
endpoint static and does not authorize maximum-sized storage.

## Procedure per file

1. Grep the file for `T [` … `..=` / `..` brackets inside type positions
   (`param:`, `->`, `field:`, `local:` declarations). Do not touch `for` /
   loop ranges or slice indices — those are expressions, not the revoked
   suffix.
2. Classify the owning position: common field, case payload, parameter, result,
   local, generic shell, or static/layout use. Apply the decision table exactly.
3. Remove migration-generated interval domains unless their nominal identity or
   establishment route has an independent customer. Equal predicates do not
   justify keeping a named domain.
4. Re-check the package: `mbx run -p omega -- --check <root.omg>`. For corpus
   fixtures, the scoped canary filter is
   `OMEGA_PASS_CANARY_FILTER=<group>/<name> mbx nextest run -p compiler
   --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile`.
5. The compiler-side deletion (parse path, `TypeConstraintNode::Range`,
   range-shell generic matching, range-recommending diagnostics) is the last
   step — do not pre-delete it while source still spells the suffix.

## Scale and ordering

The original inventory at `7a9a7b8287` measured 1,598 occurrences across 544 `.omg` files —
456 under `tests/omega`, 48 `samples/cli`, 14 `source/library`, 11
`tests/native-differential`, 7 `omega-rust/psi`, 4 `samples/gui` — plus 41
Rust files naming a range type-constraint. The corpus migration dominates;
batch the audit by customer so each batch lands with its own scoped witness.
The Epsilon-written Omega parser and its fixtures migrate on the same recipe;
Epsilon's own surface is unchanged.

Do not use later reduced occurrence counts as completion evidence: they include
unaudited rewrites. Completion requires the history-derived inventory to record
each original declaration and its canonical replacement.
