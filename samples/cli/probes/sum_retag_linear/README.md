# sum_retag_linear

First sample retagging a sum with linear payloads through
`&mutable` repair: `repair(&mutable holder.choice, inc)` runs
`choice = transform(choice, inc)` — per-case payload
extraction, drop-and-rebuild under the opposite tag, in-place
restore.

## Learnings

`holder.choice = transform(holder.choice, ..)` written
directly at the member path fails ("no unique conserved
claim mapping") — the `&mutable` param place is the spellable
route. `let x = transition ...` is not an expression form;
`&mutable` places can't be read while vacated.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
