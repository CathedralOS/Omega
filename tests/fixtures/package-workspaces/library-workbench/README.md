# library-workbench

Small Git workspace for real package-command acceptance. The root is a member
catalog, not an importable package. Install `exact-math` by its declared name;
the default alias is `exact_math`. Its relative dependency selects the sibling
`integer-constants` package at the same repository revision.

The two pinned revisions change only the constant provider's implementation.
Tests must update the selected repository as a unit, keep unrelated repositories
pinned, and import the selected API through the ordinary compiler.

Remote revisions are recorded in
[Omega's remote pins](https://github.com/CathedralOS/Omega/blob/main/tests/fixtures/packages/REMOTE_PINS.md).
Tests fetch exact commits;
running them does not mutate the remote repository.
