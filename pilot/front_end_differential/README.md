# Front-end differential pilot (reference only, not for main)

This pilot tried to compare accept/refuse verdicts from the Omega-written
front end (`source/psi`) and the Rust compiler (`omega --check`) over the same
programs. It could not run: on main at `1e1e9fc3` (2026-11-16) the Rust
compiler refuses to build `source/psi` into any runnable program, and
bootstrap compiler D does not compile `source/psi` either.

## What blocks it

The only runner for the Omega-written front end is a `tests/psi/*` gate
harness, compiled natively by the Rust CLI. `tests/psi/checking` (lex, parse,
resolve, type, check; exit code per phase) fails in three layers:

1. `harness.omg` itself: 37 diagnostics. 8 sites need explicit domain casts
   (`as +Trapping`, `as -Trapping`); `harness_domain_casts.patch` fixes them.
   The other 25 are "type arithmetic policy on a type that is not an
   integer" on any signature naming `SourceSet`, `Parser` or `SyntaxTrees`.
2. Those 25 come from `source/psi` record fields written in the retired
   whole-array form `[T; N] in Trapping` (52 fields in 4 files).
   `psi_array_qualifiers.patch` drops it on record arrays and moves it onto
   integer elements (`[u8 in Trapping; N]`).
3. With both patches, the build reaches `source/psi` bodies and refuses them
   with 551 diagnostics (`harness_build_refusals.txt`): about 460 implicit
   domain qualifications (the explicit domain-accounting rule `source/psi`
   has not migrated to), and about 90 "not supported yet" gaps in the Rust
   compiler (61 "call argument that names storage another argument lends",
   23 operators on non-scalar operands, 5 `field: value` transition patterns,
   others).

`omega --check source/psi` exits 0 because a package with no entry point
reaches no bodies; it is not evidence that `source/psi` compiles.

## Files

- `build_corpus.py` builds the corpus: standalone, library-free `main.omg`
  programs from `samples/` and `tests/fixtures/` (7,478) plus 8 mutation kinds
  (2,113 mutants).
- `run_diff.py` frames each program for the checking harness and runs
  `omega --check` on it, writing both verdicts per program.
- `rust_timing.py` times the Rust half alone.
