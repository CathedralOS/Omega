# build_git_source

`Source::Git` dependency edges require nested record-literal syntax at
every level: the source itself must be `Source::Git { repository: ...,
revision: ..., selection: ... }` braces (call-style `Source::Git(...)`
rejects), and `selection` must be a direct `PackageSelection::Root {}`
or `PackageSelection::Named { package: "..." }` literal — bare
`PackageSelection::Root` without braces and call-style
`PackageSelection::Named("kit")` both reject at dependency projection.

A well-formed Git edge reaches real acquisition: on this worker
`example.invalid` fails DNS and the closure resolves with "cannot
resolve package source: git command failed with status Some(128)" —
a legible failure, not a hang.

Controls:

- `invalid_git_call_style`: call-style source literal rejected.
- `invalid_git_selection_call`: call-style `Named` selection rejected.
- `invalid_git_selection_bare`: bare `Root` without braces rejected.
- `invalid_git_unreachable`: well-formed edge on an unresolvable host
  rejects at source-adapter acquisition (stderr tail elided).
