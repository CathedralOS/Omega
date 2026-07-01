# `omega/` — the kept, Rust-free omega rung (beginnings)

`omega-rs/` is the full Rust compiler for Omega — the **untrusted fast producer** and today's
executable reference. This directory is where omega's **kept, lattice-built** artifacts live, exactly
as `epsilon/` is to `epsilon-rs/`.

## What's here

- **`omega-meaning.sh`** — the first omega meaning gate. Real Omega sample programs from
  `samples/` are translated to gamma by [`epsilon/eps2gamma.beta`](../epsilon/eps2gamma.beta) and
  executed by [`gamma/interp.beta`](../gamma/interp.beta) — both Rust-free — and must exit with the
  `Expected exit: N` their headers document. **19 audited samples** pass today, including
  `euclid_gcd`, `collatz_sequence`, `digital_root`, `modular_exponentiation`,
  `smallest_prime_factor`, `insertion_sort`, `tic_tac_toe`, and `bounded_counter`. Audited means a
  sample whose exit matches only through a mis-parse coincidence is excluded (`format_number`:
  string buffers; `alarm_probe2`: case-pattern dispatch).

## Why this works (and what it is not)

Epsilon was designed as omega's on-ramp, so the core machine surface — `data` blocks,
`machine Main::main(&mut self)`, states, guarded transitions, console boundary — is shared.
`eps2gamma.beta` (decision D2: meaning by elaboration to gamma) gained the omega surface deltas:

- **dotted field paths** — `self.state.n` (nested data flattens to one threaded slot per path)
- **subjectless transitions** — `transition { _ -> x() }` (subject ≔ 0)
- **`state name(&mut self)` headers** and **state-body `let`s** (collected machine-wide, deduped)
- **state arguments** — `state report(&mut self, code: i32)` + arm `-> report(70)`. Params register
  as machine locals; an arm's argument expressions are passed inline at those slots' positions in
  that arm's state call (branch-correct — no cross-arm pre-evaluation)
- `use` declarations, data attributes `[copy, zero_init]`, `in Trapping/...` domain annotations, and
  `as i32`-style widening casts are tolerated syntactically (semantic no-ops while values stay in
  range — a sample whose checks depend on saturation/wrap-around at the boundary is excluded)

This is **not yet** omega's full meaning: slices, strings, enums/cases, contracts, state arguments,
and cross-data method calls are outside the subset, and omega-rs native execution is not yet
cross-checked against this route (that is translation validation, decision D3). The subset grows
slice by slice, exactly as epsilon's did.
