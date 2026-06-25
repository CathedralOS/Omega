# `compiler/` — the bootstrap lattice

A tower of small languages rising from a tiny, hand-audited seed. Its thesis is
**trust by checking, not by pedigree**: nothing is trusted because of where it came
from (a vendor, a binary, a previous compiler); each rung's output is *checked* —
re-derived, conformance-tested, or diverse-double-checked — so trust is earned, not
assumed. The whole tower verifies in one command:

```sh
sh verify-lattice.sh        # seed → assembler → bc → checker, every rung's gate in order
```

## The rungs

Each rung is built by the one below it and pinned by its own gate. The lower you go,
the smaller and more auditable; the higher you go, the more expressive.

| rung | what it is | trust mechanism |
| --- | --- | --- |
| **alpha** | a ~300-line seed VM (21 opcodes), the root of trust | hand audit + a 25-case conformance suite (`SEMANTICS.md`) + a **diamond**: two independent seeds (x64, arm64) emit byte-identical bytecode |
| **beta** | the assembler, written in Alpha | **self-hosts** — reproduces its own bytecode byte-for-byte |
| **beta-lang-rs** | a throwaway Rust on-ramp for the Beta language | exists only to bootstrap `bc`; then leaves the lineage |
| **beta-lang** (`bc`) | the Beta compiler **written in Beta** | self-hosts byte-for-byte — Rust is out of the trust path |
| **gamma** | a safe functional language (ADTs + pattern matching): a reference interpreter (`interp.beta`) and a static type checker (`typeck.beta`) | interpreter + 22-case type-checker gate; the type checker is what makes the checker *safe to write* |
| **delta** | the **certificate checker** — the trust anchor | see below |
| epsilon, omega | higher rungs (systems language; full dependent types) | design-stage |

Trust flows bottom-up: the hand-audited seed runs the assembler, which lowers `bc`,
which compiles the checker, which validates a proof. No rung trusts its builder
blindly — every step is re-checkable.

## The trust anchor (`delta/`)

This is the rung the whole architecture exists to produce: a small, hand-auditable
checker with **sole authority over what is true**. An untrusted, arbitrarily clever
proof-*search* engine may produce certificates; this checker decides — a false
proposition cannot get past it, however the certificate was found.

`check.beta` now decides **first-order intuitionistic predicate logic with
induction**:

- propositional logic (`→ & + ⊥`), so `¬A = A → ⊥`, by Curry-Howard a simply-typed
  λ-calculus type checker;
- **equality** with the conversion rule (`refl` discharged by computation) — an
  equivalence relation (symmetry/transitivity via Leibniz `eqelim`);
- `∀`/`∃` with **capture-avoiding** instantiation (de Bruijn shifting), unary and
  binary predicates;
- induction over the two built-in inductive types (Peano naturals, Lists) **and over
  user-declared types** (`data` + `rec` — general structural induction, e.g. a binary
  `Tree`), plus Peano no-confusion (`disj`, `sinj`);
- **named lemmas** (`def`/`use`) so proofs factor instead of forming one monolith;
- real theorems, all pinned in the gate: `n+0=n`, `n≠s n`, every nat is `0` or a
  successor, `l++nil=l`, append associativity, `len(a++b)=len(a)+len(b)`, and — via
  lemmas — **addition commutativity** and **right distributivity** `(a+b)*c=a*c+b*c`,
  so the naturals satisfy the core semiring axioms inside the checker.

### Why you can believe it

The trust anchor is defended five independent ways (all under `verify-lattice.sh`):

- **95-case gate** (`test.sh`) — valid certificates accepted, invalid rejected.
- **27-case soundness battery** (`soundness.sh`) — invalid certificates that must
  *all* be rejected, including classical-but-non-constructive tautologies (excluded
  middle, Peirce, the drinker paradox).
- **38-case checker diamond** (`checker-diamond.sh`) — *diversity = security* applied
  to the checker itself: `check.beta` (Beta, tagged-memory + CFG guard-state dispatch) and
  `gamma/checker.gamma` (Gamma, ADTs + pattern matching) must return identical
  verdicts on every proof. It has caught real divergences.
- **type-safety** — `gamma/checker_typed.gamma` is the checker fully annotated, and
  gamma's own type checker accepts it: the trust anchor's *code* is statically safe.
- **soundness seams** — `semantics-diamond.sh` (definitional `=` vs the interpreter's
  operational eval) and `induction-soundness.sh` (inductively-proved universals
  confirmed against the interpreter at concrete instances). These are *evidence* for
  the open soundness theorem `provable ⟹ true-about-the-reference-interpreter`, not a
  proof of it.

## Honest frontiers

- The soundness theorem itself is the deep open problem.
- User-declared types are inert data with structural equality; a **user-defined
  recursive-function layer** (functions over those constructors, with reduction rules
  feeding the conversion rule) is the next frontier — it would make *theorems* over
  user types provable, not just their induction principles.
- epsilon (systems language) and omega (full dependent types) are design-stage.
