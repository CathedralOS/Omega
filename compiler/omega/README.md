# `omega/` — the kept, Rust-free omega rung

`omega-rs/` is the full Rust compiler for Omega — the **untrusted fast producer** and today's
executable reference. This directory holds omega's **kept, lattice-built** artifacts: the Rust-free
meaning route and its gates.

**Epsilon was absorbed here** (decision D7, 2026-07-02): what was a separate rung is now the
**Omega kernel subset** — the machine-surface fragment of Omega the lattice already gives Rust-free
meaning to. Its translator, gates, and certifier corpus live on below; `epsilon-rs/` keeps its
historical name as the kernel subset's disposable Rust producer.

## What's here

- **`omega2gamma.beta`** (née `eps2gamma.beta`) — the Rust-free **Omega → gamma meaning translator**,
  written in Beta (built alpha→beta→bc, the same lineage as `gamma/interp.beta`). Reads Omega source
  on stdin, prints a gamma s-expression; `interp.beta` runs it. Decision D2: meaning by elaboration
  to the canonical interpreter. Covers the kernel subset (the full former-epsilon feature set: state
  machines, self fields/arrays, cross-machine calls + recursion, stdin/stdout, self-methods incl.
  value-returning) plus the omega surface (dotted field paths, subjectless transitions, state
  arguments, cross-data method calls via single-instance monomorphization, `use`/attributes/domain
  annotations tolerated in range).

- **`omega-meaning.sh`** — real Omega samples from `samples/` run down the meaning route; each must
  exit with the `Expected exit: N` its header documents. **19 audited samples** + feature tests under
  `tests/` (audited = a sample whose pass depends on a mis-parse coincidence is excluded).

- **`kernel-diamond.sh`** — the 42-case triple diamond on kernel-subset programs: native execution
  (epsilon-rs backend) == Rust-free `omega2gamma→interp` == the Rust `gamma_emit` cross-check, over
  arithmetic, comparisons, state machines, fields, arrays, calls, stdin/stdout, self-methods.

- **`convergence-reference.sh`** — the proof-carrying loop with **no Rust anywhere**: certifiers
  (incl. the omega safety obligations `certify-lt/bounds/accesses/safety` and the certifying compiler
  frontend `certify-source`) are translated by `omega2gamma.beta`, *run* by `interp.beta`, and their
  delta certificates checked by `check.beta` — all on the alpha→beta→bc lineage. Mutated certs and
  unsafe source are rejected.

## What this is not yet

Omega's *full* meaning: slices, strings, enum cases, contracts surface, floats (interp-blocked), and
bitwise (seed-blocked) are outside the subset. omega-rs native execution is not yet cross-checked
per-compilation against this route — that is translation validation (decision D3), the intended
next deep arc. The subset grows slice by slice, exactly as it has since slice 0.
