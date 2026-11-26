# build_member_selection

Member selection across dependency occurrences: `dep_one` and `dep_two`
export distinct machine names and both resolve in one product body.

- `invalid_ambiguous_member`: two deps exporting the same machine name
  through different aliases leave the name unselectable — the call
  has no visible declaration in the caller’s scope.
- `invalid_private_member`: a dependent package cannot call a dep's
  private `hidden` machine through a file alias.
- `invalid_ghost_member`: the entry selects a declaration from `{kit}/ghost.omg`.
  The dependency has no admitted file with that name.
