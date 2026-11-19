# build_member_selection

Member selection across dependency occurrences: `dep_one` and `dep_two`
export distinct machine names and both resolve in one product body.

- `invalid_ambiguous_member`: two deps exporting the same machine name
  through different aliases leave the name unselectable — the call
  "selects no declaration" (not an ambiguity diagnostic; the name is
  simply unreachable).
- `invalid_private_member`: a dependent package cannot call a dep's
  non-`pub` machine — "package `invalid_private_member` (...) selects
  private machine `hidden::entry`".
- `invalid_ghost_member`: `use kit::ghost` names a module file that
  does not exist in the dep's snapshot — raw resolve failure
  ("failed to resolve .../source/ghost.omg").
