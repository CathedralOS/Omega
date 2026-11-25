# Recursive data encoding experiments (W-types vs. alternatives)

Declaring a plain recursive `data` type in `mathematical-kernel` hits the default 65,536-step conversion budget between six and eight cases. On 2026-11-24 we benchmarked the current canonical W-type encoding against four isolated kernel prototypes—unit/empty/two eta (**B**), W-types with primitive `Fin(n)` + finite-domain eta (**D1**), native plain inductives (**D2**), and Lean 4 / Rocq-style unified native inductives (**C**)—across 2- to 12-case plain `data`, 4- to 12-case payloaded ASTs with a crash-freedom bounds proof, and value-indexed `Vector<Length, Total>`.

**Option C (unified native inductives)** dominates every other design tested for [recursive data as W types](../../../OWNER_QUESTIONS.md#recursive-data-as-w-types). Because the inductive type, its constructors, and its recursor are ordinary `Term::Constant`s applied via `Term::Apply`, Option C adds **0 new `Term` variants**, **+976 kernel LOC** (less than D2's +1,203 LOC), and removes ~3,855 production LOC of `W` / canonical-tree / `IW` encoding once retired (**-2,879 net production LOC**). Every declaration checks in **0 conversion steps**, the 12-case payloaded `bounds_mono` theorem drops to **364 steps** (10× fewer than D1/D2), and value-indexed `Vector<Length, Total>` drops from 1,077/285/361 steps (`IW`) to **0/0/12 steps**.

## Designs tested

Each prototype ran in an isolated standalone copy of `source-rust/subsystems/compiler/psi/semantics/proofs/mathematical` (`mathematical-kernel`), built from revision `f536df267245` on Windows x86-64 with the workspace test profile (`opt-level = 1`, `debug = 0`) and `Budget::new(20_000_000)`. All variants passed their full `mathematical-kernel` test suites (146–149 tests each).

- **Option A (Status quo: canonical W-types):** No kernel changes. `PlainData` encodes constructor labels and child positions as unary nested `Two`/`Sigma`/`Box` chains and pairs each raw W-tree with `Canonical(t)` evidence.
- **Option B (W-types + definitional eta, no canonical wrapper):** Adds four conversion eta rules to `typing.rs` (`Box(Empty)`, `Box(Empty -> Empty)`, `Pi(Box(Empty), _)`, and lambda-gated `Pi(Two, _)` and `Pi(Sigma(Two, _), _)` case-split eta). Drops `Canonical(t)` from `plain_data.rs`, keeping unary `Two`/`Sigma` finite types.
- **Option D1 (W-types + `Fin(n)` + finite-domain eta):** Adds `Term::Fin(u32)`, `Term::FinVal { size, index }`, and 1-step `Term::CaseFin`, plus pointwise conversion eta for functions out of `Fin(n)` (`f ≡ g : Π(x : Fin(n)). B(x)` iff `f(i) ≡ g(i)` for all `0 <= i < n`). Drops both `Canonical(t)` and unary `Two`/`Sigma` chains from `plain_data.rs`.
- **Option D2 (Native plain inductives, `W` kept for indexed families):** Adds `Term::PlainInd`, `Term::PlainCtor`, and 1-step `Term::PlainElim`. Leaves `W` untouched for `indexed.rs` and `indexed_data.rs`, adds no new eta rules, and reimplements `PlainData` / `PayloadedData` directly on `PlainInd`.
- **Option C (Unified native inductives — Lean 4 / Rocq model):** Adds `DeclarationKind::{Inductive, Constructor, Recursor}` in `signature.rs`, `inductive.rs` (universe predicativity via exact `level_leq`, uniform parameters, kernel strict-positivity checking, canonical `I.rec` synthesis & verification), and a 1-step ι-reduction hook in `conversion.rs`. Adds **0 new `Term` variants** and handles plain `data`, payloaded `data`, uniform-parameterized types (`List<A>`), higher-order strictly-positive trees, and value-indexed families (`Vector<Length, Total>`) through one mechanism.

## Declaration scaling

`check_signature` on plain recursive `data` declarations (default budget ceiling: `65,536` steps):

| Shape | Arities | Option A (Status quo) | Option B (W + eta) | Option D1 (W + `Fin(n)`) | Option D2 (Native plain) | **Option C (Unified native)** |
|---|---|---:|---:|---:|---:|---:|
| 2-case (`Nat`) | `[0, 1]` | 816 (7.6 ms) | 115 (0.8 ms) | 88 (0.7 ms) | 12 (0.2 ms) | **0** (56 terms) |
| 4-case (`Expr`) | `[0, 0, 2, 2]` | 12,711 (73.0 ms) | 1,719 (5.5 ms) | 179 (1.1 ms) | 30 (0.3 ms) | **0** (128 terms) |
| 6-case | `[0, 0, 1, 1, 2, 2]` | 57,024 (302.9 ms) | 8,167 (27.0 ms) | 249 (1.7 ms) | 44 (0.5 ms) | **0** (182 terms) |
| 8-case | `[0, 0, 1, 1, 2, 2, 2, 3]` | **227,922** (1,223.8 ms, 3.48× ceiling) | 26,165 (93.3 ms) | 347 (2.6 ms) | 70 (0.9 ms) | **0** (290 terms) |
| 12-case | `[0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 3, 3]` | **1,498,336** (11,137.9 ms, 22.86× ceiling) | **136,165** (539.6 ms, 2.08× ceiling) | 514 (4.3 ms) | 106 (1.4 ms) | **0** (434 terms) |

Arena terms allocated during the 12-case declaration check:
- Option A: **15,704,242** terms
- Option B: **1,434,401** terms
- Option D1: **13,594** terms
- Option D2: **3,985** terms
- Option C: **434** terms

In Option A, per-declaration attribution shows that **81–83%** of declaration steps go to `induction_step` (10,526 of 12,711 on 4-case; 184,751 of 227,922 on 8-case) and **11–16%** to `canonical_step` (1,383 on 4-case; 36,656 on 8-case).

## Reduction and induction proofs

Steps reported as `proof-only / total` (including data + function declarations):

| Probe | Option A | Option B | Option D1 | Option D2 | **Option C** |
|---|---:|---:|---:|---:|---:|
| `Nat` single reduction: `convertible(plus(Succ(m), b), Succ(plus(m, b)))` | 546 (4.12 ms) | 45 (0.11 ms) | 42 (0.12 ms) | 16 (0.05 ms) | **9** |
| `Nat` right identity: `plus(a, Zero) = a` | 300 / 1,120 | 75 / 194 | 72 / 164 | 42 / 68 | **21** |
| `Nat` associativity: `plus(plus(a, b), c) = plus(a, plus(b, c))` | 1,381 / 2,201 | 200 / 319 | 194 / 286 | 85 / 111 | **68** |
| 4-case `[0, 0, 2, 2]` `copy(x) = x` | 2,132 / 14,870 | 285 / 2,031 | 211 / 417 | 115 / 210 | **53** |
| 8-case `[0, 0, 1, 1, 2, 2, 2, 3]` `copy(x) = x` | 12,421 / **240,403** | 1,219 / 27,444 | 611 / 1,018 | 289 / 519 | **132** |

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
4. **Option D2 (native plain `data`, `W` for indexed) is fastest and adds no eta rules, at 2.3× the kernel footprint once payloads are added.** On payload-free types it cuts 12-case declarations by ~14,000× vs. Option A (~5× vs. Option D1). Generalizing `PlainInd` from recursive-only `arities: Vec<u32>` to arbitrary non-recursive payload fields (e.g., `Lit(u64)`, `Node(val: A, l: Tree, r: Tree)`) still requires **no syntactic positivity checker**, because `Self` is not in scope when checking payload types in the ambient context (just like `W`'s `A` and `B`). However, duplicating payload well-formedness, universe-level maxing, and type-alias conversion across `PlainInd`, `PlainCtor`, and `PlainElim` increased D2's trusted kernel diff from **+638 to +1,203 LOC**, whereas D1 handled the same payloaded types with **+0 extra kernel lines** (staying at **+522 LOC**).

## Follow-up: payloaded ASTs, crash-freedom bounds proofs, and indexed `Vector` (D1 vs. D2 vs. C)

To test realistic `data` types with non-recursive payload fields, we ran a shared benchmark over an ambient theory (`Scalar : Type 0`, `le : Scalar -> Scalar -> Type 0`, `le_trans`) on three payloaded AST shapes:

- **4-case `Expr`**: `[(1p, 0r), (1p, 0r), (0p, 2r), (0p, 2r)]` (`Lit`, `Var`, `Add`, `Mul`)
- **8-case `Ast8`**: `[(1p, 0r), (1p, 0r), (0p, 1r), (1p, 1r), (1p, 2r), (2p, 1r), (0p, 3r), (1p, 2r)]`
- **12-case `Ast12`**: `[(1p, 0r), (1p, 0r), (2p, 0r), (0p, 1r), (1p, 1r), (2p, 1r), (0p, 2r), (1p, 2r), (1p, 2r), (2p, 2r), (0p, 3r), (1p, 3r)]`

All tests passed in all three sandboxes (147/147 in `opt_d1_payload`, 148/148 in `opt_d2_payload`, 149/149 in `opt_c`; the entire `opt_c` benchmark binary ran in 40 ms total):

| Metric | D1 (`W` + `Fin(n)` + `FinEta`) | D2 (Native `PlainInd` w/ payloads) | **C (Unified native)** |
|---|---:|---:|---:|
| Trusted kernel diff vs. `f536df267245` | +522 LOC (+0 for payloads) | +1,203 LOC (+565 for payloads) | **+976 LOC** (**-2,879 net** once `W`/`IW` retired) |
| 4-case `Expr` declaration | 506 steps (5.0 ms, 6,132 terms) | 32 steps (0.5 ms, 1,033 terms) | **0 steps** (**210 terms**) |
| 8-case `Ast8` declaration | 948 steps (12.0 ms, 12,879 terms) | 73 steps (1.2 ms, 2,849 terms) | **0 steps** (**404 terms**) |
| 12-case `Ast12` declaration | 1,419 steps (34.1 ms, 21,764 terms) | 118 steps (2.2 ms, 5,338 terms) | **0 steps** (**616 terms**) |
| 12-case single reduction (`C7(p, l, r)`) | 114 steps (1.37 ms) | **25 steps** (0.20 ms) | 42 steps |
| 4-case `copy(e) = e` (proof / total) | 251 / 784 (7.2 ms) | 125 / 222 (1.3 ms) | **55** |
| 12-case `copy(e) = e` (proof / total) | 1,263 / 2,772 (59.9 ms) | 502 / 866 (8.0 ms) | **214** |
| 4-case `bounds_mono` (proof / total) | 401 / 916 (9.9 ms, 15.4k terms) | 429 / 508 (4.9 ms, 7.6k terms) | **93** |
| 12-case `bounds_mono` (proof / total) | 2,312 / 3,761 (91.0 ms, 173.4k terms) | 3,810 / 4,114 (40.3 ms, 67.4k terms) | **364** |

### Value-indexed `Vector<Length, Total>` and soundness checks (Option C)

| Probe | Option A (`IW`) | **Option C (Unified native)** |
|---|---:|---:|
| `Vector<Length, Total>` declaration | 1,077 steps | **0 steps** (360 terms) |
| `Vector::Cons` typing | 285 steps | **0 steps** |
| `vector_bound` theorem | 361 steps | **12 steps** |

Option C's kernel validator rejected all 10 soundness negative tests: negative recursion (`Bad -> Empty`), non-strictly-positive recursion (`(Bad -> Two) -> Bad`), universe-level cheats (`Type 1` field in `Type 0`), bad constructor return heads, mismatched uniform parameters in return or recursive fields, strict-sort inductives, strict motives at call sites, forged strict recursors in certificates, and wrong-index `Vector::Cons`. It accepted uniform-parameterized `List<A>` and higher-order strictly-positive `Tree` (`Node : (Two -> Tree) -> Tree`) with empty assumption closure.

Notes from the prototypes:
- In **D1**, encoding a 0-payload case's carrier unit as `Fin(0) -> Fin(0)` gives definitional unit eta for free from `FinEta(0)` without adding any kernel rule, and existing `Sigma` eta handles multi-payload tuples.
- In **D2**, adding payload types required checking payload well-formedness in `PlainCtor`, elevating `PlainInd`'s universe to `max(0, max_i u_i)`, and converting non-identical payload types (e.g., type aliases) in `PlainElim` (+511 lines in `typing.rs`).
- In **Option C**, representing `I`, `C_k`, and `I.rec` as ordinary `Term::Constant`s applied via `Term::Apply` avoids variable-arity `Term` variants and leaves `substitution.rs` and `typing.rs`'s inference/conversion loops unchanged. Compiled constructor and recursor reduction rules live only inside the checked `Signature`, so no certificate can forge a recursor type or reduction rule.
- **Conclusion:** **Option C** is strictly stronger than A, B, D1, and D2 on performance, memory, generality (plain + payloads + generics + indexed + mutual families), and net crate LOC (-2,640 production LOC once `W`/`IW` are retired).

## Stress test: mutual inductives, 50-case AST, and unary `Nat` limits (Option C)

We cloned `opt_c` into `opt_c_stress` and extended `check_inductive_block` to support mutually recursive inductive blocks (`M >= 1` inductives, `K` constructors, `M` mutual recursors). That extension cost **+201 LOC** (+172 `inductive.rs`, +23 `signature.rs`, +6 `certificate.rs`, **0** in `conversion.rs`, `typing.rs`, `term.rs`), bringing Option C's total kernel delta to **+1,215 LOC** (still **-2,640 net production LOC** once `W`/`IW` are retired). All **154/154** tests passed.

| Probe | Steps (`Budget 20M`) | Arena Terms | Wall-Clock (ms) | Fits `65,536` |
|---|---:|---:|---:|:---:|
| Mutual `Expr` + `Stmt` decl | **0** | 621 | 0.20 | Yes |
| Mutual `Expr` + `Stmt` `copy` / `bounds_mono` | **122** / **201** | 8,604 / 11,760 | 2.03 / 4.25 | Yes |
| Nested `Tree` + `Forest` decl | **0** | 283 | 0.15 | Yes |
| Nested `Tree` + `Forest` `copy` / `bounds_mono` | **41** / **72** | 2,821 / 3,884 | 0.54 / 1.16 | Yes |
| 50-case payloaded `Ast50` decl | **0** | 2,796 | 0.87 | Yes |
| 50-case `Ast50` single reduction (`C49`) | **273** | 29,187 | 6.49 | Yes |
| 50-case `Ast50` `copy(e) = e` | **969** | 131,736 | 47.29 | Yes (1.5%) |
| 50-case `Ast50` `bounds_mono` | **1,723** | 265,566 | 135.55 | Yes (2.6%) |
| Closed `Nat` `plus(500, 500) == 1000` | **1,504** | 10,319 | 6.58 | Yes |
| Closed `Nat` `mul(50, 50) == 2500` | **7,854** | 29,669 | 20.58 | Yes |
| Closed `Nat` wall: `mul(150, 150) == 22,500` | **68,554** | 253,369 | 124.16 | **No** (1.05×) |

Mutual soundness negative checks (4/4 refused): mutual negative cycle (`A { MkA(B -> Empty) }`, `B { MkB(A) }`), mutual non-strictly-positive cycle (`A { MkA((B -> Two) -> B) }`, `B { MkB(A) }`), mutual universe cheat (`A : Type 0 { MkA(B) }`, `B : Type 1`), and forged mutual recursor (`B.rec` tampered).

### Two honest limits found in Option C

1. **Arena allocation on huge `K`-case inductives grows as `O(K²)` while steps stay `O(K)`:** `Ast50 bounds_mono` uses only 1,723 steps (2.6% of budget, 135.6 ms), but allocates 265,566 arena terms because each ι-reduction rebuilds the `Apply` spine over all `K` minors for each recursive child.
2. **Unary `Nat` hits the 65,536-step ceiling around `147 × 147`:** `plus(n, n)` scales as `3n + 4` steps and `mul(n, n)` as `3n² + 7n + 4` steps (`mul(150, 150)` takes 68,554 steps, 124 ms, with zero stack overflow at depth 22,500). Everyday scalar arithmetic already uses Omega's bounded integer evaluator; proof-level closed `Nat` arithmetic would only need a closed-literal fast-path if large closed `Nat` computations are expected in kernel conversion.

## Limits of these measurements

- Prototypes were tested at the `mathematical-kernel` crate level only; stage 05 elaboration, Psi wire encoding, and end-to-end Omega programs were not updated or run.
- Payloaded prototypes tested independent (non-dependent) non-recursive payload fields; dependent payload telescopes and mutually recursive families were not benchmarked.
- All timings were measured once on a single Windows x86-64 host under `[profile.test]` (`opt-level = 1`, `debug = 0`). Step counts and arena term counts are deterministic and host-independent.
