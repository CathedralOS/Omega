# build_member_rows

The `Build::member` row family names workspace-member package
directories — the shape the repository root build uses for `source/psi`,
`source/omega`, `source/library/std`. A member row declares a *workspace*,
and exactly one declaration kind is allowed per build:

- `invalid_mixed_kinds`: `Build::application` + `Build::member` in one
  activation rejects — "package, application, and workspace member
  declarations cannot be mixed".
- `invalid_workspace_root`: a member-only build is an explicit workspace;
  `--check` refuses it — "expected a package build declaration, found an
  explicit workspace declaration".

Member rows therefore grant no product imports on their own; the plain
application root beside the controls is the passing coverage.
