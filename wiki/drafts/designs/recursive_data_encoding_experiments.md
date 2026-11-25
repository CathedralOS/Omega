# Recursive data encoding experiments (W-types vs. alternatives)

Declaring a plain recursive `data` type in `mathematical-kernel` hits the default 65,536-step conversion budget between six and eight cases. On 2026-11-24 we benchmarked the current canonical W-type encoding against three isolated kernel prototypes—adding unit/empty/two eta, adding a primitive finite type `Fin(n)` with finite-domain eta, and adding native plain inductive types—on the same 2-, 4-, 6-, 8-, and 12-case probes.

The measurements eliminate the status quo and pure unit/two eta, and narrow the decision for [recursive data as W types](../../../OWNER_QUESTIONS.md#recursive-data-as-w-types) to two clean contenders: **W-types with `Fin(n)` + finite-domain eta (D1)** or **native plain inductive types with `W` retained for indexed families (D2)**. Both add roughly 520–640 lines to the trusted kernel, shrink `plain_data.rs` by 537–857 lines, and reduce a 12-case declaration from 1,498,336 steps (11.1 s, 15.7M arena terms) to 514 steps (4.3 ms) or 106 steps (1.4 ms).

## Designs tested

Each prototype ran in an isolated standalone copy of `source-rust/subsystems/compiler/psi/semantics/proofs/mathematical` (`mathematical-kernel`), built from revision `f536df267245` on Windows x86-64 with the workspace test profile (`opt-level = 1`, `debug = 0`) and `Budget::new(20_000_000)`. All four variants passed all 146 tests (`mathematical-kernel` unit tests, `certificate_abstraction`, and `certificate_strict`, including `indexed_data`, `quotient`, and `certificate`).

- **Option A (Status quo: canonical W-types):** No kernel changes. `PlainData` encodes constructor labels and child positions as unary nested `Two`/`Sigma`/`Box` chains and pairs each raw W-tree with `Canonical(t)` evidence.
- **Option B (W-types + definitional eta, no canonical wrapper):** Adds four conversion eta rules to `typing.rs` (`Box(Empty)`, `Box(Empty -> Empty)`, `Pi(Box(Empty), _)`, and lambda-gated `Pi(Two, _)` and `Pi(Sigma(Two, _), _)` case-split eta). Drops `Canonical(t)` from `plain_data.rs`, keeping unary `Two`/`Sigma` finite types.
- **Option D1 (W-types + `Fin(n)` + finite-domain eta):** Adds `Term::Fin(u32)`, `Term::FinVal { size, index }`, and 1-step `Term::CaseFin`, plus pointwise conversion eta for functions out of `Fin(n)` (`f ≡ g : Π(x : Fin(n)). B(x)` iff `f(i) ≡ g(i)` for all `0 <= i < n`). Drops both `Canonical(t)` and unary `Two`/`Sigma` chains from `plain_data.rs`.
- **Option D2 (Native plain inductives, `W` kept for indexed families):** Adds `Term::PlainInd { arities }`, `Term::PlainCtor { arities, case, fields }`, and 1-step `Term::PlainElim`. Leaves `W` untouched for `indexed.rs` and `indexed_data.rs`, adds no new eta rules, and reimplements `PlainData` directly on `PlainInd`/`PlainCtor`/`PlainElim`.

## Declaration scaling

`check_signature` on `PlainData::new(arities).declarations(...)` (default budget ceiling: `65,536` steps):

| Shape | Arities | Option A (Status quo) | Option B (W + eta) | Option D1 (W + `Fin(n)`) | Option D2 (Native plain) |
|---|---|---:|---:|---:|---:|
| 2-case (`Nat`) | `[0, 1]` | 816 (7.6 ms) | 115 (0.8 ms) | 88 (0.7 ms) | **12** (0.2 ms) |
| 4-case (`Expr`) | `[0, 0, 2, 2]` | 12,711 (73.0 ms) | 1,719 (5.5 ms) | 179 (1.1 ms) | **30** (0.3 ms) |
| 6-case | `[0, 0, 1, 1, 2, 2]` | 57,024 (302.9 ms) | 8,167 (27.0 ms) | 249 (1.7 ms) | **44** (0.5 ms) |
| 8-case | `[0, 0, 1, 1, 2, 2, 2, 3]` | **227,922** (1,223.8 ms, 3.48× ceiling) | 26,165 (93.3 ms) | 347 (2.6 ms) | **70** (0.9 ms) |
| 12-case | `[0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3]` | **1,498,336** (11,137.9 ms, 22.86× ceiling) | **136,165** (539.6 ms, 2.08× ceiling) | 514 (4.3 ms) | **106** (1.4 ms) |

Arena terms allocated during the 12-case declaration check:
- Option A: **15,704,242** terms
- Option B: **1,434,401** terms
- Option D1: **13,594** terms
- Option D2: **3,985** terms

In Option A, per-declaration attribution shows that **81–83%** of declaration steps go to `induction_step` (10,526 of 12,711 on 4-case; 184,751 of 227,922 on 8-case) and **11–16%** to `canonical_step` (1,383 on 4-case; 36,656 on 8-case).

## Reduction and induction proofs

Steps reported as `proof-only / total` (including data + function declarations):

| Probe | Option A | Option B | Option D1 | Option D2 |
|---|---:|---:|---:|---:|
| `Nat` single reduction: `convertible(plus(Succ(m), b), Succ(plus(m, b)))` | 546 (4.12 ms) | 45 (0.11 ms) | 42 (0.12 ms) | **16** (0.05 ms) |
| `Nat` right identity: `plus(a, Zero) = a` | 300 / 1,120 | 75 / 194 | 72 / 164 | **42 / 68** |
| `Nat` associativity: `plus(plus(a, b), c) = plus(a, plus(b, c))` | 1,381 / 2,201 | 200 / 319 | 194 / 286 | **85 / 111** |
| 4-case `[0, 0, 2, 2]` `copy(x) = x` | 2,132 / 14,870 | 285 / 2,031 | 211 / 417 | **115 / 210** |
| 8-case `[0, 0, 1, 1, 2, 2, 2, 3]` `copy(x) = x` | 12,421 / **240,403** | 1,219 / 27,444 | 611 / 1,018 | **289 / 519** |

## Code footprint and metatheory trade-offs

| Metric | Option A | Option B | Option D1 | Option D2 |
|---|---:|---:|---:|---:|
| Trusted kernel diff (LOC net) | 0 | +183 (`typing.rs`) | +522 (`term`, `substitution`, `conversion`, `typing`) | +638 (`term`, `substitution`, `conversion`, `typing`) |
| `plain_data.rs` impl LOC | 1,199 | 830 (-369) | 662 (-537) | **342 (-857)** |
| Net crate LOC (excl. benchmarks) | 0 | -186 | -15 | **-219** |
| Variable-arity `Subterms` needed? | No | No | Yes (~130 LOC in `term.rs`) | Yes (~130 LOC in `term.rs`) |

### Key findings per option

1. **Option A hits a hard wall at 7–8 cases.** Even 6 cases consumes 87% of the default 65,536 budget. Larger real-world `data` types cannot be declared without raising budgets by orders of magnitude and paying multi-second compile times and tens of millions of arena nodes.
2. **Option B ("just add unit eta") does not solve the general problem.** Unit and empty eta (`Box(Empty)` and `Box(Empty -> Empty)`) only eliminate `Canonical(t)` for 0- and 1-field cases (`Nat`). Supporting 2+ fields required lambda-gated `Pi(Two, _)` and `Pi(Sigma(Two, _), _)` case-split eta, which **breaks transitivity of conversion** across a lambda bridge between two stuck neutral functions. Even with all four rules, unary `Two`/`Sigma` chains still exceed 65,536 steps at 12 cases (136,165 steps, 1.43M terms).
3. **Option D1 (`W` + `Fin(n)` + `FinEta`) keeps one recursive primitive.** Positivity remains automatic because `W` is still the only recursive type former. Pointwise eta at `Pi(Fin(n), B)` is sound, transitive, and terminating (each step strictly reduces `Pi` nesting depth, bounded by `n` checks). `Fin(n)` tags can also simplify `indexed_data` if desired. Worst-case note: comparing two non-equal functions out of a huge `Fin(N)` expands `N` checks (bounded by `Budget`).
4. **Option D2 (native plain `data`, `W` for indexed) is fastest and adds no eta rules.** It cuts 12-case declarations by ~14,000× vs. Option A (~5× vs. Option D1) and removes 857 lines from `plain_data.rs`. Generalizing `PlainInd` from recursive-only `arities: Vec<u32>` to arbitrary non-recursive payload fields (e.g., `Lit(u64)`, `Node(val: A, l: Tree, r: Tree)`) still requires **no syntactic positivity checker**, because `Self` is not in scope when checking payload types in the ambient context (just like `W`'s `A` and `B`). A syntactic strict-positivity checker is only needed if nested or mutually recursive occurrences inside payload types (e.g., `List Self`) are also made primitive.

## Limits of these measurements

- Prototypes were tested at the `mathematical-kernel` crate level only; stage 05 elaboration, Psi wire encoding, and end-to-end Omega programs were not updated or run.
- `PlainData` models recursive arities only (no non-recursive payload fields). Payload fields will add typing and substitution work in both D1 and D2, though the scaling difference between unary `Two`/`Sigma` vs. `Fin(n)` vs. native `PlainInd` holds.
- All timings were measured once on a single Windows x86-64 host under `[profile.test]` (`opt-level = 1`, `debug = 0`). Step counts and arena term counts are deterministic and host-independent.
