# Owner rulings

The owner's decisions that shape the plans, newest last. A plan that
disagrees with a ruling here is wrong; a ruling that needs to change is
changed here first, with the date.

## 2026-11-17

1. **Omega is one linear pipeline.** Each stage reads one representation and
   writes the next. Nothing reaches back to an earlier stage, and nothing
   travels beside the pipeline: no sidecar, no library a stage calls instead of
   receiving its input, no stage work done outside a stage.
2. **The least code outside the pipeline.** Shared code is explicit, and there
   is as little of it as possible.
3. **Foundation holds primitives only.** A primitive is a generic building
   block with no Omega-language or compiler meaning, the way `arena.rs` is.
   There is no `language/` layer: language tables and anything computed from a
   program move into the pipeline stage or representation that first needs
   them, and later stages carry them forward. Code with no pipeline user leaves
   for its real owner (packages, products, Build, the CLI, test tools).
4. **A shared item must defend itself with evidence**: who uses it, at which
   pipeline positions, and who creates its values. A definition kept in prose
   leaks again.
5. **The backend should not exist.** `omega-rust/omega/compilation/backend/`
   dissolves into the pipeline: stage work becomes numbered stages, the
   selected target's description becomes an input representation, and the
   rest goes to its real owner.
6. **Everything is shown through evidence.** Plans and maps cite generated
   data and source quotes, and are published as pages in the Omega design
   system.
