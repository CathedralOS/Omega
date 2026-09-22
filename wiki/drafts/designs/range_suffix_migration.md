# Range-suffix migration recipe

Companion recipe for **REMOVE-BRACKETED-RANGE-ANNOTATIONS** (TASKS.md :62).
The revoked spelling is the bracketed scalar range annotation — `u64 [1..=8]`,
`i32 [0..n)`-style `T [min..=max]` / `T [min..max]` on declared types — parsed at
`tokens-to-syntax-trees/src/type_syntax/parse_type.rs` (~:779) as
`TypeConstraintNode::Range`, both inclusive and exclusive forms. The owner
decision is settled in
[domains](../../spec/language/domains.md#declaration-and-membership): scalar types
have no bracketed range-annotation suffix; bounds come from `requires`,
`ensures`, guards and arithmetic as ordinary proof facts, and a published
reusable bound is declared as a predicate domain and spelled `T in Domain`.
There is no compatibility mode and no new compiler-provided range domain —
this document is the mechanical recipe for migrating the corpus. Delete this
recipe once REMOVE-BRACKETED-RANGE-ANNOTATIONS closes — no corpus file spells
the suffix and the parse path is gone.

## Decision table by position

| Old spelling | New spelling | When |
|---|---|---|
| `-> T [lo..=hi]` | `-> T in D` | The bound is part of the published contract other callers rely on; declare `domain T::D requires self >= lo && self <= hi;` beside the requirement |
| `-> T [lo..=hi]` | `-> T` + `ensures result >= lo && result <= hi` | The bound is a one-off result fact; no reusable name needed (Squalr's `MemoryAlignment::get_size_in_bytes` is the customer example) |
| `x: T [lo..=hi]` | `x: T in D` | The bound is a caller-facing admission obligation |
| `x: T [lo..=hi]` | `x: T` + `requires x >= lo && x <= hi` | The bound is signature-local; `requires` on the machine or signature already flows as an ordinary proof fact |
| field/local `v: T [lo..=hi]` | `v: T` | Locals need no qualification to use established facts — a checked call's postconditions apply to its exact result even when the receiving local is declared bare |
| field `f: T [lo..=hi]` | `f: T in D` | Storage whose bound must be re-established after every write; the declared qualification is enforced at consumption points |
| `T [lo..hi]` (exclusive) | `self >= lo && self < hi` | In the domain predicate or contract. For literal endpoints, `lo..=hi-1` is equivalent; prefer the predicate form so exclusive half-open intent stays readable |
| `x: T [0..=self.n]` (dependent endpoint) | `requires x <= self.n` on the machine, or a parameterized domain `x: T in Bounded<N>` | Endpoints can be expressions over receiver state. A domain with a const-position binder (`domain<T, const U: Unit> T::Quantity<U>`, `domain<P,T> Extent::Resident<P,T>`) takes a literal, `const` binder, or runtime `Value` binder; where the bound is per-call receiver state, a `requires`/`ensures` fact is the direct translation |

## Positions that reject instead of migrating

Layout-determining uses — array extents, `const` positions — were never
serviceable by the suffix's proof-fact machinery and stay rejected. They
migrate to ordinary const extents, not to `in D`.

## Procedure per file

1. Grep the file for `T [` … `..=` / `..` brackets inside type positions
   (`param:`, `->`, `field:`, `local:` declarations). Do not touch `for` /
   loop ranges or slice indices — those are expressions, not the revoked
   suffix.
2. For each occurrence ask whether the bound is *published* (consumed by
   other machines/signatures) or *local*. Published → predicate domain +
   `in D`. Local → drop the suffix and, if the bound is still load-bearing,
   carry it as `requires`/`ensures`.
3. Collapse duplicate per-file domains: the same `lo..=hi` pair over the same
   carrier type belongs to one named domain, not one per spelling site.
4. Re-check the package: `mbx run -p omega -- --check <root.omg>`. For corpus
   fixtures, the scoped canary filter is
   `OMEGA_PASS_CANARY_FILTER=<group>/<name> mbx nextest run -p compiler
   --test canary_suite entry_and_abi::pass_canary_coverage::pass_canaries_compile`.
5. The compiler-side deletion (parse path, `TypeConstraintNode::Range`,
   range-shell generic matching, range-recommending diagnostics) is the last
   step of REMOVE-BRACKETED-RANGE-ANNOTATIONS — do not pre-delete it while
   corpus still spells the suffix.

## Scale and ordering

Measured at `7a9a7b8287`: 1,598 occurrences across 544 `.omg` files —
456 under `tests/omega`, 48 `samples/cli`, 14 `source/library`, 11
`tests/native-differential`, 7 `omega-rust/psi`, 4 `samples/gui` — plus 41
Rust files naming a range type-constraint. The corpus migration dominates;
batch it by directory so each batch lands with its own scoped canary witness.
The Epsilon-written Omega parser and its fixtures migrate on the same recipe;
Epsilon's own surface is unchanged.

Re-verified at `2dbfecd98e49` (zergling-172): the recipe's anchors still
resolve — `TypeConstraintNode::Range` at `parse_type.rs:791/801`, and the
`domain T::D requires self …` + `T in D` spelling this prescribes is the one
the corpus already uses (`tests/omega/pass/modules/module_operator_home/
units.omg:6`).
