# Workspace members

A Build returns `BuildProduct::Workspace` with the member catalog it constructed. The package check executes each returned member.

- `invalid_repeated_member` includes the same member twice in its returned catalog.
- `invalid_missing_workspace_member` returns a member directory that does not exist.
