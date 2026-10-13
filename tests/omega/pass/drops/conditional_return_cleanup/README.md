# conditional_return_cleanup

A generic conditional returns one owner and cleans only the other:
`pick` selects between two `Guard` owners, and the unselected arm's
owner runs its cleanup at the return.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
