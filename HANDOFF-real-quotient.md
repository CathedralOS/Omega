# Handoff: core Real as the Cauchy quotient

Goal (TASKS.md, PROOF-KERNEL-CORE): replace the opaque `Real` in
`source/library/core/real.omg` (two axiom rows: `real_add_commutative`,
`real_multiply_commutative`) with the Cauchy quotient of regular sequences,
so its laws are derived. Ruling: #6596
(`wiki/spec/proofs/quotients.md#cauchy-sequences-and-the-real-quotient`).

## Done (merged to main)

- #6542: `Box` and `Squash` of a proposition read in source.
- #6547: a quotient by a boxed squash relation is mere (QUOTIENT_BOX_PROP).
- #6559: equivalence rows and lift congruences may prove a relation at every
  index (`LawProof::Pointwise`).
- #6618: `data Sequence<S, const At: Sampler<S>> { state: S; }` works in proofs
  at a closed selection (for example `Sequence<N, repeat>`). Tests are in
  `06_lowered-program-to-checked-program/src/tests/sequence.rs`.
- #6621: open owner question 1 in OWNER_QUESTIONS.md, "May core `Real` be the
  quotient of one regular function sequence type?" **This blocks the Real
  declaration.** It recommends
  `data Regular { samples: Sequence<Nat -> Rat, apply>; } requires regular(...)`,
  `data Real = Regular % converge_together`, and
  `project<S, const At>(s) requires regular(s)`.

## In flight (this branch, `claude/project-thread-3gsxb2`)

This commit carries only hand-made rustfmt fixes to code merged in #6618.
That code went in unformatted, because the fmt edits were made after staging.
The fixed files are `read/clause_terms.rs`, `read/source_records.rs` and
`tests/sequence.rs`. Open a PR for it, or fold it into the next code PR.

## Next steps

1. Mathematical function types as types. The parser accepts arrows
   (`C -> C -> |proof|::Type`) only in generic-parameter position, where they
   are proposition families (`01_tokens-to-syntax-trees/src/parameters/generics.rs`,
   `declarations/mathematical_bindings.rs::parse_mathematical_type`).
   `machine apply(state: N -> N, n: N)` fails to parse: "expected `)`, found `->`".
   This needs a new type form through syntax trees, symbol-resolved trees,
   typed trees, stage 05 (`types.rs::value_type`) and the kernel (`Term::Pi`).
   About 100 match sites name `TypeReferenceNode::Callable`; use them as the map.
   Spec: `wiki/spec/proofs/mathematical_bindings.md#universes-and-mathematical-function-types`.
   - An alternative was probed: a callable alias `data Fn = machine(n: N) -> N ...`.
   - It fails in two places. In typing, "runtime call callee has no complete
     declared value type in its exact lexical scope" for `f(n)` in an `ensures`.
     In stage 05, "contract callable field in a clause".
2. Theorems generic over `S` and `At`:
   `machine sample<S, const At: Sampler<S>>(s: Sequence<S, At>, n) -> N { At(s.state, n) }`.
   See `read/templates.rs`: `mathematical_template` admits only Machine and
   Value params today, and `type_template` admits only Type params.
3. After the owner rules on question 1, declare Real and lift add and multiply.
   - The math at one index is already proved in `source/library/core/rat.omg`:
     `regular_add_at`, `related_add_at`, `regular_mul_at`, `related_mul_at`,
     `regular_transitive_at`.
   - The relation is |x_n - y_n| <= 4/n at every n. It is pointwise, so #6559 rows apply.
   - Laws over all reals need quotient induction. `QUOTIENT_IND_PROP` (scheme 16)
     is not yet used by the source pipeline.
4. Update real.omg, cauchy.omg, TASKS.md and the audit doc.

## Commands and gotchas

- Use `mbx`, never cargo, always with `< /dev/null`. Example:
  `mbx test -p lowered-program-to-checked-program -p prepared-typed-trees-to-lowered-program -p typed-trees -p symbol-resolved-trees-to-typed-trees`.
  The full run takes about 6 minutes.
- Formatting: `mbx xtask fmt --check --base origin/main`. Fix by hand; never
  run rustfmt or full fmt on files.
- Clippy: `mbx clippy -p <crates> --all-targets -- -D warnings -A dead_code -A clippy::arc_with_non_send_sync -A clippy::needless_return -A clippy::collapsible_if`.
  `-A dead_code` is needed because main has an unused fn in `terminal-wire`.
- PR body check: `mbx xtask check-rules --base origin/main --title ... --body-file ...`.
  - Bodies need `Gate: RC-SOURCE-SEMANTICS | ...`, `Spec:`, `Proof: ... | clean`
    and an `Agent:` line from `mbx xtask provenance --full-session` with Model
    set to `unavailable`.
  - Titles take the form `lane: statement`.
- Quick probing:
  - Add an `#[ignore]` test that reads the env var `OMEGA_SCRATCH` and prints
    verdicts through `judged()`, as in tests/quotient.rs.
  - Run the built test binary with `--ignored --exact <name>`.
  - Kernel symbol kinds: a machine name is `SymbolKind::Machine`, and a machine
    binder is `SymbolKind::MachineParameter`.
- Edit `wiki/spec/` only with Jarod's explicit approval. Design questions go
  in OWNER_QUESTIONS.md.
