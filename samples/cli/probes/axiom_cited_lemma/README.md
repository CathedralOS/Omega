# axiom_cited_lemma

Axiom cited by a lemma: `boundary machine mul_comm_axiom(a: Nat, b: Nat) -> Nat ensures mul(a,b)==mul(b,a)` declares an accepted unproven fact; `uses_axiom` cites it inside `state body` to discharge its own identical ensures. `use omega::language::core::nat` + bare `Nat`/`Nat::Zero`/`mul` denotational terms.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
