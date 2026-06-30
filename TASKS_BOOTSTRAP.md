# Bootstrap Lattice — Status & Onboarding

> Live status of the **self-building lattice** only — decoupled from the main
> compiler's [TASKS.md](TASKS.md). If you are a fresh agent, read this top to
> bottom, then the architecture overview, and you are caught up.

## 30-second orientation

Omega is being built **from a tiny hand-audited seed, rung by rung**, so the
trusted toolchain has no Rust/LLVM in its lineage. The principle is *trust by
checking, not by pedigree*. The full architecture (trust model, the rungs, the
honest edges) lives in
**[wiki/architecture/bootstrap_lattice/bootstrap_lattice.md](wiki/architecture/bootstrap_lattice/bootstrap_lattice.md)
— read it first.**

## Read-first

| Doc | What |
| --- | --- |
| [bootstrap_lattice.md](wiki/architecture/bootstrap_lattice/bootstrap_lattice.md) | The overview: trust-by-checking, two stacks, rung table, honest edges, the two-roles-for-Rust cut |
| [rungs/*.md](wiki/architecture/bootstrap_lattice/rungs/) | One doc per rung (alpha … omega) |
| [compiler/beta/CALLING_CONVENTION.md](compiler/beta/CALLING_CONVENTION.md) | The calling convention (proven on the seed) |
| [compiler/beta/LANGUAGE.md](compiler/beta/LANGUAGE.md) | The Beta language surface (v0) |
| [compiler/beta-lang-rs/README.md](compiler/beta-lang-rs/README.md) | The Beta compiler on-ramp + slice status |

## The map (what's on disk)

| Path | What it is | Status |
| --- | --- | --- |
| `compiler/alpha/` | **The seed**: a 21-opcode register tape VM. **Two independent hand-authored realizations** — x64 Windows PE (`.exe` + audited `.hex`) and arm64 macOS Mach-O (`alpha_arm64_macos` + `.s` + `.lst`). The provenance root, and the first lattice diamond (same source → byte-identical tapes on both). | DONE (x64 + arm64-macOS) |
| `compiler/beta/` | **The assembler** (`assembler.alpha`, written in Alpha assembly) + the Beta-language docs/examples. | DONE — self-hosts byte-identically |
| `compiler/beta-rs/` | Throwaway Rust on-ramp for the *assembler* (cold-start only). | parked |
| `compiler/beta-lang-rs/` | Throwaway Rust on-ramp for the **Beta-language compiler** (`.beta` → Alpha asm). | slices 1–6 done; self-check passed |
| `compiler/beta-lang/` | The Beta compiler **written in Beta** (`bc.beta`) — slice 7. | **DONE — self-hosts** (byte-for-byte fixed point) |
| `compiler/delta/` | **The certificate checker** (`check.beta`) — the trust anchor: full intuitionistic prop logic + equality/conversion + ∀∃ + induction + `Mem`/`ProdIs`/`Perm` (three list inductive predicates). Compiled by `bc`, run on the seed. Plus a 212-proof corpus (sqrt2-irrational, infinitude-of-primes, and **the Fundamental Theorem of Arithmetic — BOTH halves: existence AND uniqueness up to permutation**), a soundness battery (no false proof accepted), a 43-theorem soundness sweep (proved by check.beta **and** computed-true in the interpreter), and **FOUR random fuzzers** that cross-check the trust anchor across *every* subsystem (`seam-fuzz` reducer, `checker-diamond-fuzz` conversion, `logic-diamond-fuzz` first-order logic, `predicate-diamond-fuzz` Mem/ProdIs/Perm). The `Perm` permutation relation that FTA uniqueness needs was added to all three checkers (`check.beta`, `checker.gamma`, `checker_typed.gamma`) and is diamond-cross-validated. | **WORKING** — also rewritten in gamma (`checker.gamma`, type-checked + diamond-tested); **no soundness-bridge THEOREM** to execution yet (the deep open problem) — but comprehensive fuzzed evidence for it |
| `compiler/gamma/` | `gamma.alpha` = parked v13 imperative compiler. **`interp.beta`** = interpreter-first reference interpreter (functional, ADTs + pattern matching, fuel-bounded) + **`typeck.beta`** = a static type checker + **`checker.gamma`** = the Delta checker in gamma, all in Beta. | interpreter + type system done |
| `compiler/epsilon-rs/` | The **Epsilon on-ramp** (throwaway Rust): compiles a machines / `data` / `transition` / `enum` systems language — Omega's executable surface — to x64 PE **and** arm64 Mach-O. Self-hosts (`lowermachine.alp` emits itself byte-identically). Hosts the **convergence** (`certify-*.alp` programs emit delta certificates — including over the `Perm` predicate) **and a proof-carrying contract system** (`assert`/`requires`/`ensures`, compiler-emitted static discharge, call-site composition). | **GATED** — slices 1–9 + operators/enums/state-params; 191 aarch64 tests; 168-cert convergence (arithmetic, structure, **permutation**, **computed factorization**); contract discharge (`contracts.sh` 30 verified, `discharge-soundness.sh` 29 ok: the full arithmetic-rewrite equality family + the order family incl. same-base offset gaps + bare-parameter gaps under `requires b >= 0` + implicit array-bounds with weakening (via `lt-le-trans`) + call-site composition incl. expression args `B(a+k)` — 11 increments). **Epsilon-meaning diamond** (`epsilon-meaning-diamond.sh`, 35 cases): `EPS_EMIT=gamma` translates the FULL effectful epsilon language (arithmetic + all comparisons, state machines, mutable locals/fields/arrays, cross-machine calls + recursion, stdin/stdout, and self-method calls — only seed-blocked bitwise excepted) to a gamma program the Rust-FREE reference interpreter (`interp.beta`) runs, and its output/exit code must match native execution. It even validates REAL certifiers — `certify-add`/`certify-mul` reproduce their byte-exact delta-certificate output through the lattice — defining epsilon's meaning IN the lattice (rungs/epsilon.md: "Written in Delta/Gamma"). NB the translator itself is still Rust (a spec/cross-check); the epsilon *compiler* is already Rust-free via the self-hosting `lowermachine.alp` |
| `compiler/omega-rs/` | The real Omega compiler, in Rust. Separate concern: the *producer*, not the lattice. | (other workstream) |

## Where we are RIGHT NOW

**The lattice has a complete vertical slice, seed → checker, with no Rust in the
runtime lineage.** One command verifies it all:

```sh
sh compiler/verify-lattice.sh        # -> "LATTICE VERIFIED ✓"
```

It walks every rung in dependency order:

- **alpha** — seed re-derives from source + conforms to `SEMANTICS.md` + diamond
  (`verify.sh`); 256 KB tape hole; both seeds in lockstep.
- **beta** — the assembler self-hosts byte-identically; gained a `db` data directive.
- **Beta language** — `beta-lang-rs` (throwaway Rust on-ramp) compiles the corpus.
- **bc** — the Beta compiler **written in Beta** (`beta-lang/bc.beta`) **self-hosts**:
  it compiles its own source to a compiler that reproduces that compilation
  byte-for-byte. From `bc` on, Rust is out of the lineage.
- **delta** — the **certificate checker** (the trust anchor): `check.beta` does full
  intuitionistic propositional logic (`-> & + ⊥`) **plus equality with the
  conversion rule** (`2+2=4` proved by computation), and `eq.beta` does definitional
  equality by fuel-bounded normalization. Both compiled by `bc`, run on the seed.
- **gamma** — `interp.beta`, the interpreter-first reference interpreter (stage 1:
  functional, fuel-bounded).

The lattice's thesis — *trust by checking, not pedigree* — is now a working stack.

## Roadmap (next)

7. **Self-hosting Beta compiler — DONE.** Rather than hand-write the compiler in
   assembly (the old framing), it is written *in Beta* (`compiler/beta-lang/bc.beta`)
   and cold-started through the on-ramp — the same on-ramp-then-discard pattern the
   assembler uses. **`bc` self-hosts:** it compiles its own source to a compiler
   that reproduces that compilation byte-for-byte (`sh compiler/beta-lang/selfhost.sh`),
   so from that compiler on, Rust is out of the lineage. bc implements the whole
   language — arithmetic, locals + symbol table, `if`/`else`/`while` + comparisons,
   procedures/params/calls/recursion, `byte[]`/`word[]` memory, char literals +
   `read_byte`/`write_byte` + call statements, and string literals via `emit("...")`.
   30/30 per-feature gate + the self-host fixed point. Enabled by the assembler
   `db` data directive (#46) and growing the arm64 tape hole 32 KB → 256 KB (bc's
   self-tape is ~45 KB; logic-dominated, not emit-dominated). The Rust on-ramp
   (`beta-lang-rs`) is now discardable from the steady state.
8. **Gamma — STAGE 1 DONE (interpreter-first).** Rather than grow the parked
   imperative `gamma.alpha`, the target gamma is the *interpreter-first, functional,
   ADT + pattern-matching* language the Delta checker is meant to be written in
   (rungs/gamma.md). [`compiler/gamma/interp.beta`](compiler/gamma/interp.beta) is
   the reference interpreter — *meaning is the interpreter*, fuel-bounded (totality)
   — stage 1: a pure functional core (ints, top-level recursive functions, `if`,
   `let`, arithmetic/comparisons; `fac 5→120`, `fib 10→55`, `gcd→12`), compiled
   Rust-free by `bc`. 11/11 in `test-interp.sh`. Stage 2 (ADTs + pattern matching) DONE; a static type system (`typeck.beta`,
   Int + ADTs, catches Int-vs-List etc.) DONE; the Delta checker rewritten in gamma
   (`checker.gamma`, full parity, ~12 functions vs ~350 lines) DONE — runs on the
   reference route and agrees with check.beta (`checker-diamond.sh`). The fully
   type-annotated `checker_typed.gamma` (which `typeck.beta` accepts) is DONE too,
   and is now mechanically tied to the trusted checker: `erase_types.py` type-erases
   it and the checker diamond runs it as a THIRD oracle agreeing with `checker.gamma`
   on ALL 83 cases (`frule_to_flat.py` rewrites the user-function proofs from the
   wrapper rule form to the typed flat form, so even `fdisp` — the one helper where the
   two representations diverge — is cross-checked). So "the checker is statically
   type-safe" and "the checker is behaviorally correct" are now claims about the SAME
   artifact, enforced every lattice run.
9. **Delta — the checker / evidence rung (where trust actually starts): PROTOTYPE
   DONE + GROWN.** [`compiler/delta/check.beta`](compiler/delta/check.beta) is now a
   **full intuitionistic propositional** proof checker (`-> & + ⊥`; Curry-Howard =
   simply-typed-lambda type checker) **plus equality with the conversion rule** —
   `(= t1 t2)` over Peano terms, with `refl` discharged by definitional computation,
   so `2+2=4` is proved *by reduction* and equations compose with the connectives.
   [`eq.beta`](compiler/delta/eq.beta) is a standalone definitional-equality checker
   (fuel-bounded normalization). Both compiled by `bc`, run on the seed; 32/32 in
   `compiler/delta/test.sh`. Demonstrates the architecture (tiny trusted checker,
   unbounded untrusted producer) AND the logic↔computation seam. Caught a real
   calling-convention bug (the prologue clobbered argument 3 — the checker's first
   3-arg `alloc`). Still a *Beta* prototype (target: a Gamma program once stage 2
   lands) and with **no soundness bridge** to actual program execution — the deep
   open problem. What exists is bounded *evidence* for that bridge, at the seam where
   a checker bug is most dangerous: `semantics-diamond.sh` (definitional `=` vs the
   interpreter's operational eval), `induction-soundness.sh` (inductive universals
   confirmed at concrete instances), and `soundness-sweep.sh` (curated corpus
   theorems that must be BOTH proved by check.beta AND computed true by the gamma
   interpreter — sourced straight from `proofs/*.elab`, so widening it is one line).
10. **Epsilon — the systems-language on-ramp + the convergence: WORKING + GATED.**
    [`compiler/epsilon-rs/`](compiler/epsilon-rs/) compiles a machines / `data` /
    `transition` / `enum` language (Omega's executable surface) to **both** x64 PE and
    arm64 Mach-O, and self-hosts (`lowermachine.alp` emits itself byte-identically). Its
    language grew well past the original slices 1–9: arithmetic/bitwise/shift operators
    (`% & | ^ << >>`, unary `-`), tag-only **and** payload `enum`s (single- and
    multi-field, the shape of omega's `shape_area`) with exhaustiveness checking, and
    **state parameters**. 134/134 on the aarch64 gate. The big payoff is the
    **convergence** (`convergence.sh`, 168 confirmed): `certify-*.alp` programs *compute*
    a result and *emit a delta certificate* the trust anchor independently checks —
    proof-carrying computation, the Omega idea in miniature, across the full intuitionistic
    logic (∃ `divides`/`mod`, ∧ `safety`, ∨ `max`, ¬ `distinct`), both inductive predicates
    (`member`=Mem, `product`=ProdIs), builtin arithmetic, AND user-defined-function
    reduction (`sum`). The capstone is `certify-factor`: it READS n, COMPUTES a factor list by
    trial division, and certifies `ProdIs(factors, n)` — the FTA's existence half, executed and
    machine-checked (where `certify-product` only asserts a fixed list). The suite has a NEGATIVE
    CONTROL too — `certify-wrong` is a deliberately-buggy computation (claims `a+b = a+b+1`) whose
    certificate the anchor must REJECT, so the thesis "the anchor checks the computation, not the
    compiler" is a falsifiable test in every run (without it, all accept-tests would pass vacuously
    if the checker ever degenerated to accept-everything). And `convergence-reference.sh` runs the SAME
    loop down the **reference / meaning route**: the certifier is translated to gamma and EXECUTED by the
    Rust-free reference interpreter `interp.beta` (the rung's "meaning"), which emits the certificate, and
    `check.beta` accepts it — so COMPUTE-then-CHECK runs on lattice artifacts via the reference interpreter
    (not the fast native route), answering the architecture's "checker run down the reference route" open
    question. 4 diverse cert kinds (refl, ProdIs, Mem, divisibility) + the `certify-wrong` reject control.
11. **Epsilon contracts — the verification-compiler, BUILT.** What the frontier once called
    "the open question" is now a working proof-carrying contract system (`src/discharge.rs`,
    `contracts.sh`, `discharge-soundness.sh`). Surface: `assert` (runtime trap),
    `requires`/`ensures` (pre/postconditions, desugared to runtime asserts at entry / every
    return). **Static discharge** (`EPS_EMIT=contracts`): the COMPILER emits a delta certificate
    that a contract holds for ALL inputs (a closed `∀`-proof) which the trust anchor checks at
    BUILD time — `refl` for definitional equalities, existential witnesses for the order family
    (`< > <= >=`, constant *and* parameter gaps), and **lemma citation** (`(use N)` against
    `gen-contract-lib.py`) for obligations that hold only up to a banked theorem (add-zero,
    commutativity, le-trans). Conservative (never a false cert) and cross-checked against
    execution (`discharge-soundness.sh`: discharged contracts never trap). **Call-site
    composition** (omega's BoundedCallArgumentObligation): a wrapper discharges a callee's
    precondition from its own — forwarding (identical, any arity) via `(lam (hyp 0))`, and
    weakening (caller's bound strictly stronger) via an le-trans citation. This is omega-rs's
    entailment model — cite a proven lemma at the site terms — in miniature.

## The frontier (what's left)

The lattice is comprehensively built from the seed through a certifying systems language **with
a working proof-carrying contract system** (item 11). What remains:

- **Richer contract composition — ESSENTIALLY MINED OUT (10 increments landed).** All on the proven
  mechanism (cite the right banked lemma at the site terms, no new design). DONE: `<=` upper-bound
  weakening (call-site, via le-trans); additive AND multiplicative commutativity (`result == b+a` /
  `b*a`) and associativity (`(a+b)+c == a+(b+c)`, `(a*b)*c == a*(b*c)`); reflexive order bounds
  (`result <= X` / `>= X` via add-zero-right); MULTIPLE postconditions per machine; **IMPLICIT
  ARRAY-BOUNDS** (every `self.arr[i]` emits `i < len` — literal index by a ground witness, parameter
  index by forwarding `requires i < len`); **same-base offset gaps** (`result < a+5` from `a+2`, via
  add-assoc — the inner ground sum reduces under the stuck `(p a _)`, so no congruence lemma needed);
  **ARRAY-BOUNDS WEAKENING** (a tighter `requires i < M`, M ≤ len, discharges `i < len` via the banked
  `lt-le-trans` lemma — memory safety from a stronger-than-needed contract); and **EXPRESSION
  CALL ARGS** (`B(a+k)` against `requires param >= k`, discharged conditionally on the caller's own
  `requires a >= 0` via add-commutes); and **BARE-PARAMETER GAPS** (`result <= a+b` with the witness a
  PARAMETER `b` — discharged CONDITIONALLY under `requires b >= 0`: the cert is `∀a∀b. (b>=0) -> result
  <= a+b`, the postcondition mirror of the `a+k` call-site slice, sound because the entry assert of
  `requires b >= 0` guards the postcondition; without the requires, still refused). The value-level
  `requires b >= 0` IS the non-negative integer domain — domain-type *syntax* (`i32 in NonNeg`) would be
  sugar over it. `contracts.sh` 30 verified / `discharge-soundness.sh` 29 ok.
  GENUINELY REMAINING (each blocked, not merely undone): overflow obligations (infeasible — i32 MAX is an
  unrepresentable unary numeral in Delta); `Trapping`/`Wrapping` overflow-domain semantics (a distinct
  axis from non-negativity); an all-accesses-must-discharge policy.
- **Epsilon out of Rust — STARTED (the structural lever).** `epsilon-rs` is a throwaway Rust on-ramp,
  so epsilon is the first rung OUTSIDE the Rust-free lineage `bc` established. rungs/epsilon.md says
  epsilon's meaning is "Written in Delta/Gamma" — defined by the reference interpreter, not the native
  backend. The **epsilon-meaning diamond** is the seed: `EPS_EMIT=gamma` (`src/gamma_emit.rs`) translates
  the supported subset to a gamma expression `interp.beta` runs, and the exit code must match native
  execution. DONE: the full operator set (`+ - * / %`, all six comparisons faithfully encoded from
  `interp.beta`'s `lt`/`eq`); and **STATE MACHINES** — a machine becomes mutually-recursive gamma `def`s
  over its locals frame (`me` = entry, `s_k` = each state, all sharing the `l0..l_{n-1}` signature),
  mutation is threaded SSA (each write rebinds via a fresh `let`), and transitions are guarded tail-calls;
  loops/conditionals/`gcd`/`factorial` all agree; and **SELF DATA FIELDS** (zero-initialised, threaded
  through the same state vector as locals via a unified Env — `self.f = …` rebinds SSA like a local
  write; field-based loops agree); and **CROSS-MACHINE CALLS** (each machine reachable from the entry via
  a `Call` becomes its own `m{idx}_*` gamma defs; a free-machine call returns a value, args filling the
  callee's parameter locals and 0 for the rest — recursion and callees-with-states included: `fact(5)` via
  a recursive helper agrees); and **SELF ARRAYS** (`self.arr[i]` modeled as a threaded gamma LIST with
  emitted `nth`/`setl` helpers — read → `(nth a i)`, write → functional `(setl a i v)`, zero-initialised
  to its element count; sum-of-squares over a buffer agrees); and **`read_byte`** (the input stream is a
  threaded LIST slot; `x = read_byte()` consumes the head — or `-1` at EOF — and advances to the tail, as
  two functional `match`es; the compiler bakes the bytes via `EPS_GAMMA_INPUT` and the diamond feeds the
  SAME bytes to native stdin); and **STDOUT** (`write_byte`/`write_line` modeled as an accumulated output
  LIST the program RETURNS — interp prints it, the diamond decodes it back to bytes and compares to native's
  raw stdout; needs NO interp change since interp already renders its result value). 29 diamond cases
  (echo+1 filter, write_line, count-up); and **SELF-METHOD CALLS** (`self.m()` mutates the shared `self`,
  so the program-wide UNIFIED self-state — all fields/arrays/stdin/stdout any `&mut self` machine touches —
  is threaded into the callee and bundled back out as a right-nested `Pair` tuple the caller unbundles via
  `match`; a method terminates by returning that bundle). 33 diamond cases (emit-pair method, method inside
  a read loop). This is the LAST language feature: epsilon's full effectful surface — arithmetic, state
  machines, locals/fields/arrays, calls + recursion, stdin/stdout, and methods — is now lattice-defined and
  cross-checked against native execution. Only bitwise/shift remain BLOCKED (absent from Beta and the frozen
  21-opcode seed). **REAL PROGRAMS NOW VALIDATE**: `certify-add` and `certify-mul` (actual production
  certifiers — they read stdin, parse decimals, compute, and emit a delta certificate via `emit_nat`
  methods + `write_line`) reproduce their BYTE-EXACT certificate output through the gamma route (35 diamond
  cases). The diamond can check an actual program's meaning against a Rust-free lattice semantics, not just
  constructed tests. (Larger certifiers — `certify-lt`/`mod` — exhaust interp's arena on big unary numerals;
  a fuel/arena limit, not a translation gap.) Next strategic step: port the translator itself off Rust.
- **Toward Omega's proof surface — STARTED.** rungs/omega.md: Omega = the lower rungs PLUS contracts +
  refinement types + **proof automation as an untrusted front line that emits certificates the Delta
  checker validates** ("automation discharges the easy 95% with zero hand-proving, a tiny kernel checks
  the hard 5%"). The lattice already had the kernel (`check.beta`), the certificate format, and
  certificate-*emitting computation* (the convergence) — now it has the first genuine proof AUTOMATION:
  `delta/prover.py`, an untrusted proof-SEARCH front line that now covers the FULL intuitionistic
  propositional fragment (`->`, `&`, `+`, `(bot)`), the FIRST-ORDER fragment (predicates/relations over
  terms, ∀/∃ with intro AND elim), AND EQUALITY (`(= a b)` over Peano terms z/s/p/m). Given a goal it searches
  a sound natural-deduction calculus and EMITS a `check.beta` certificate the kernel re-checks. Propositional
  rules: lam/app, pair/fst/snd, inl/inr/case, absurd. Quantifier rules: gen (∀-intro), inst (∀-elim), wit
  (∃-intro), unpack (∃-elim). Equality: refl (an equality goal whose two sides share a normal form -- a local
  `nf` mirrors check.beta's own term `normalize` for z/s/p/m EXACTLY, so every refl emitted is accepted),
  plus a conversion-aware axiom (a hypothesis equal to the goal up to term conversion discharges it -- e.g.
  P(1+1) ⊢ P(2)). First-order needed a
  uniform EIGENVARIABLE scheme — gen/unpack mint a fresh opaque individual, substitute it for the bound var,
  and recover de Bruijn from the eigenvar stack at emit time (so nested quantifiers index correctly and an
  outer individual never collides with a prop's own inner binder). unpack runs as an invertible left rule
  before the non-invertible wit, witnesses try in-scope eigenvars first, and a per-branch `_opened` guard
  stops a parent conjunction from regenerating an already-opened existential. The propositional search is
  MEMOISED on (context proposition-set, goal) and polynomial; a depth cap + node budget backstop the
  (now-infinite, eigenvar-rich) first-order space (sound-but-incomplete: too-deep yields "unprovable", never a
  crash, never a false proof). SOUND BY CONSTRUCTION (every rule is a valid kernel typing rule, so check.beta
  accepts every proof emitted). `prover-test.sh` (652 ok): propositional tautologies (or-comm, distribution,
  or-elim-to-common, ex-falso); first-order (forall-id, forall-elim, exists-intro, forall→exists, nested gen,
  unpack tautologies incl. ∃x.P,∀x.(P→Q) ⊢ ∃x.Q); equality (1+1=2, 2*2=4, symbolic 0+x=x, the conversion axiom
  P(1+1)⊢P(2)) — all proved + kernel-accepted; non-tautologies correctly unprovable incl. eigenvariable-escape
  (⊬ ∃x.P→P(sz)) and false arithmetic (⊬ 1=0, ⊬ 1+1=1); THREE randomized fuzzes — propositional, first-order
  (provable schemas, hardens eigenvar emission), and arithmetic (closed z/s/p/m equalities, validates nf vs
  the kernel's normalize) — where every proof found is kernel-accepted. The "cleverness on the untrusted side,
  authority in the kernel" split. Widen next: equality REWRITING (eqelim/transport, sym/trans, congruence —
  reasoning FROM equality hypotheses), then inequality `<` toward the contract-discharge obligations; the long
  arc is SMT-class procedures emitting kernel-checkable certificates (the proof-engine north star).
- **The soundness bridge** (`provable-in-Delta ⟹ true-about-execution`) — the one genuinely
  research-grade step: the meta-theorem connecting the checker's logic to the reference interpreter's
  semantics. The theorem is not done, but its **bounded evidence is now COMPREHENSIVE** — FOUR
  deterministic random fuzzers cross-check the trust anchor across *every* checker subsystem, plus the
  curated diamonds, the 43-theorem proved-and-computed sweep, and discharge-soundness:
  - `seam-fuzz` — the REDUCER: definitional eq (`eq.beta`) vs operational eval (`interp.beta`), across
    built-in arithmetic/lists AND user-function recursion over user-Nats and user-lists.
  - `checker-diamond-fuzz` — equality CONVERSION across all three checkers (check.beta, checker.gamma,
    type-erased checker_typed.gamma) on random Peano/List equations.
  - `logic-diamond-fuzz` — PROPOSITIONAL + FIRST-ORDER logic (→/&/+/⊥ intro+elim and ∀/∃ over Pred via
    gen/inst/wit/unpack) across all three checkers.
  - `predicate-diamond-fuzz` — the INDUCTIVE PREDICATES (Mem / ProdIs / Perm — the FTA's foundation)
    across all three checkers.

  And FOUR OPERATIONAL seams now bridge ALL FOUR checker subsystems to an independent notion of truth
  (kernel derivation vs an independent executable/oracle, not checker-vs-checker): `semantics-diamond`
  (EQUALITY / conversion), `induction-soundness` (inductive UNIVERSALS), `predicate-soundness` (the
  inductive PREDICATES — for each accepted Mem/ProdIs/Perm proof the interpreter independently DECIDES it
  via `member`/`prod`/`isperm`; perturbed goal rejected AND decided false), and `logic-soundness` (the
  propositional LOGIC — check.beta's intuitionistic provability implies CLASSICAL validity, so every
  proposition it proves is confirmed a TAUTOLOGY by a truth-table oracle, and a perturbed genuine
  non-tautology is rejected — it would catch the checker ever accepting a classically-INVALID prop).
  `predicate-soundness` also has a random FUZZER twin (`predicate-soundness-fuzz`, 80+ random goals/run)
  — the kernel-vs-operational-decision bridge under broad coverage, the way the curated diamonds each
  grew a fuzzed twin.

## How to build & verify (repo root; Git Bash on Windows, plain `sh` on macOS; `cargo` needed for `beta-lang-rs`)

The build scripts are **platform-aware** (`compiler/alpha/seed_env.sh` picks the
seed + stamping per host), so the same commands work on Windows (x64 PE seed) and
macOS arm64 (Mach-O seed, auto re-signed after stamping). On macOS the self-host
fixed point is asserted on the program bytes, not the OS-imposed code signature.

```sh
# verify the WHOLE lattice in one command (seed -> assembler -> Beta -> bc -> checker):
sh compiler/verify-lattice.sh                                  # -> "LATTICE VERIFIED ✓"

# beta self-hosts (byte-identical program-byte fixed point):
sh compiler/beta/selfhost.sh                                   # -> "self-host ✓"

# gamma still rebuilds + runs from .gamma source:
sh compiler/gamma/rebuild.sh
sh compiler/gamma/build.sh && ./compiler/gamma/build/answer.exe ; echo $?   # -> 42

# the Beta compiler, end to end (Beta -> asm -> tape -> seed):
sh compiler/beta-lang-rs/build.sh examples/factorial.beta
./compiler/beta-lang-rs/build/factorial.exe ; echo $?          # -> 120

# the hand-written calling-convention proofs:
sh compiler/beta/build.sh examples/fib.alpha
./compiler/beta/build/fib.exe ; echo $?                        # -> 55
```

A program is a *tape* memcpy'd into the seed's hole; exit code = the low byte of the
result, so examples return small numbers on purpose.

## Things a fresh agent MUST know (gotchas)

- **Two graphs — don't conflate.** The *implementation* graph (what each tool is
  written in: the assembler in Alpha asm, the Beta compiler prototyped in Rust) is
  separate from the *lowering* graph (how a program becomes machine code). Delta /
  Epsilon are **not** IRs that `omega-rs` emits; they are rungs whose *tools* are
  written in the rung below.
- **On-ramp-then-discard.** The `*-rs` crates (`alpha-rs`, `beta-rs`, `beta-lang-rs`)
  are **throwaway Rust** used to design each rung ergonomically. The *trusted*
  version is later transcribed into the rung below and the Rust is discarded. Rust
  is the drafting table, never the foundation.
- **Naming.** "beta" currently means **two** things: the *assembler*
  (`compiler/beta/`, `beta-rs`) and the *Beta language* (`LANGUAGE.md`,
  `beta-lang-rs`). Long-view cleanup (the assembler is really *Alpha's* assembler,
  freeing "beta" for the language) is deferred. Names across the lattice are
  emergent, not fixed.
- **Extensions** (renamed this session): `.alpha` = Alpha assembly, `.gamma` =
  Gamma source, `.beta` = Beta language, `.omg` = Omega.
- **The 32 KB hole.** The seed memcpy's a program tape into a *fixed 32 KB hole*.
  gamma's tape is ~21 KB; the Beta compiler's own tape will eventually outgrow it.
  The fix (make the arena a parameter) is a small VM change — flagged, not done.
  Watch for `exceeds the seed's 32 KB hole` build failures.
- **The calling convention** (`CALLING_CONVENTION.md`): two stacks — control on the
  VM's hidden `call`/`ret` stack, data on an explicit stack via `r15` (sp), `r14` =
  frame pointer; args `r0`–`r3`, result `r0`.
- **Determinism is load-bearing.** Builds are byte-identical/reproducible (the
  basis of the provenance story). `selfhost.sh` asserts beta reproduces itself
  exactly; don't introduce nondeterminism (clocks, hash-order, timestamps).
- **Bash cwd persists** between tool calls — use absolute paths or `git -C` to
  avoid drift.

## Why all this (trust-story north star)

The seed is the **provenance** root (lineage + determinism), **not** a correctness
oracle — a self-hosting fixed point proves *consistency*, not *faithfulness*. Real
trust eventually comes from a small **checker** (the Delta rung) that validates
*evidence*, not from self-hosting. Rust leaves the trusted base entirely (it may
remain an untrusted *producer* in the interim). The endpoint and the TCB ledger
({seed, checker, specs, hardware}) are in
[proof_engine_north_star.md](wiki/design_briefs/proof_engine_north_star.md) and
[cathedral_alignment.md](wiki/cathedral_alignment.md); the full reasoning is in the
[architecture overview](wiki/architecture/bootstrap_lattice/bootstrap_lattice.md).
