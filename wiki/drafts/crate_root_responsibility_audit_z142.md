# CRATE-ROOT-RESPONSIBILITY-AUDIT — verification record (2026-09-20, `4c8ebd7ba8`)

Bare mined stub at `TASKS.md:7760` (no `**ITEM.**` marker — claimed
freeform; sibling Jarod / swarm-w9-root-audit holds the item, exp 00:35Z,
overlap claimed for this verify+record leg).

Re-mines AGENTS.md's crate-root discoverability clause ("Every crate must
have an obvious starting point that explains its responsibility through
code... `lib.rs` wiring, re-exports, and a prose file map do not
substitute"; "Each program representation has one named root file beside
`lib.rs`").

**The audit is landed and pinned.** `tests/architecture/crate_root_
responsibility.rs` (added `e20b7f651a1b`, refreshed `a2d512887dcf`)
enforces it structurally: `REPRESENTATION_ROOTS` names every
`omega-rust/{psi,omega}/representations/` crate's complete top-level
domain roots (file `<domain>.rs` or `<domain>/mod.rs` form — a bare
directory is subordinate to the same-stem root, not a second root), plus
an explicit leaf list for `lib.rs`-only crates.

Fresh witness at `4c8ebd7ba8c5e558ab984e41946f7683fc575f9b` (linux
x86-64, cargo nextest): `omega-architecture-test::crate_root_
responsibility` — **5/5 PASS**
(`named_roots_match_the_tree_and_are_wired_from_lib_rs`,
`subordinate_areas_belong_to_their_named_root`,
`lib_rs_owned_crates_define_their_items_inline`,
`every_crate_root_opens_with_a_responsibility_doc`,
`the_audit_covers_every_representation_crate`).

Residual: the pin covers `representations/` crates; pipeline/semantics/
backend crate roots are governed by the same AGENTS.md clause but are not
inside this audit's inventory — an extension there would be a new audit
surface, not a defect in the landed one.

Verdict: resolved by landed audit; coordinator can drop the stub. Record
only; no code change.
