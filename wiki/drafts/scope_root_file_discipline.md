# Scope verification: SV root-file discipline

Scope-verification draft for `NEW-SV-ROOT-FILE-DISCIPLINE-SCOPE`, re-mining
**ROOT-FILE-DISCIPLINE** (TASKS.md). Recorded against `59e0b5ec22d`,
re-verified at `b53c7ea2603` on linux x86-64.

## What the name binds

Two rules in `AGENTS.md` cover "root file discipline":

1. The named-root rule — "Each program representation has one named root file
   beside `lib.rs`; the root defines the current program and leads into
   subordinate concept-owned areas." This rule is mechanically scoped to
   `*/representations/*` crates by `tests/architecture/crate_root_responsibility.rs`:
   `REPRESENTATION_ROOTS` enumerates every representation crate and its named
   domain roots, `named_roots_match_the_tree_and_are_wired_from_lib_rs` enforces
   the on-disk match, and the audit asserts every representations crate is
   classified (`REPRESENTATION_ROOTS` or `LIB_RS_OWNED`).
2. The discoverability contract — "every crate must have an obvious starting
   point that explains its responsibility through code"; `lib.rs` wiring,
   re-exports, and a prose map do not substitute for an orchestrating entry
   file in operation-owning crates.

`semantic-vocabulary` (the `SV` in the item name) is a `psi/foundation/`
crate, not a `representations/` crate: it is absent from
`REPRESENTATION_ROOTS` and is not required to join it — the inventory's own
exhaustiveness check only sweeps `omega-rust/*/representations/*`. Its binding
discipline is therefore the second rule.

## Measured state at `b53c7ea2603`

`omega-rust/psi/foundation/semantic-vocabulary/src/`:

- `lib.rs` is a thin map — 51 lines, zero item definitions: a `//!`
  responsibility doc ("target-neutral identities and propositions shared by
  terminal Psi"), `#![forbid(unsafe_code)]`, six `mod` declarations, and
  `pub use` re-exports.
- The doc names the starting point explicitly: "Start at `identity.rs` for
  the ids every terminal artifact names … and `proposition/` for the scalar
  types, terms and propositions built over them" — the dominant "Start at …"
  convention measured by `wiki/drafts/crate_root_responsibility_audit.md`
  (69/117 roots).
- Subordinate concept-owned areas exist per concept, not per template:
  `identity.rs`, `content.rs`, `bounded_integer_type.rs`,
  `qualified_scalar_type.rs`, `ieee_float_comparison_operation.rs`, and
  `proposition/` behind `proposition.rs` (the file-plus-directory module-root
  idiom, not a discipline violation).
- The crate deliberately holds no dependency on Omega representations and
  owns no program representation — the named-root rule has no object here.

## Verdict

The discipline is already satisfied for this crate: `lib.rs` is a thin map
naming a real domain entry (`identity.rs`), and no item definitions or
implementation details live at the root. There is no slice under this name —
a fix would invent a `semantic_vocabulary.rs` root the architecture test does
not require and move the doc pointer sideways, not forward.

If the mined name instead meant the ROOT-FILE-DISCIPLINE row's general
surface: the audit's open findings live in `omega/representations/target`,
`omega/backend/layout`, and `omega/backend/runtime/runtime-abi`
(`crate_root_responsibility_audit.md` F3 — vocabulary types held in oversized
`lib.rs` files), and the pin is `tests/architecture/crate_root_responsibility.rs`.
Those are the bounded legs a future pass could split out; each is a named
representation-root extraction with `REPRESENTATION_ROOTS` registration.
