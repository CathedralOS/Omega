# write_only_member_forward

`&write` composes member-wise: `forward(&write pair,
choose_left, value)` holds one write-only record loan
and each state re-lends `&write destination.left` /
`.right` — member projection carries the qualifier.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
