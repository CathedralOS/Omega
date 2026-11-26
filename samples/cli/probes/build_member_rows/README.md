# build_member_rows

The `Build::member` row family names workspace-member package
directories — the shape the repository root build uses for the packages under
`source/subsystems/compiler` and `source/application`, and `source/library/system`. A member row declares a *workspace*,
and exactly one declaration kind is allowed per build:

- `invalid_mixed_kinds` combines `Build::application` and `Build::member`
  in one activation. The refusal identifies the mixed roles.
- `invalid_workspace_root` passes an explicit workspace to Product source
  checking. A workspace declaration supplies no Product source owner.

Member rows therefore grant no product imports on their own; the plain
application root beside the controls is the passing coverage.
