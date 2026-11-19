# Exact package entry nomination

Run `python tests/product/entry_review/check.py --omega <fresh-omega-binary>`.
This works on Windows and macOS; it checks UEFI source/selection and does not
execute a target image. Set the host temporary-directory environment normally.

The public package update workflow checks three temporary source graphs:

- An explicit physical provider selects its package's entry schema despite
  unrelated same-named schemas in other dependencies.
- No selection cannot choose among those schemas.
- Two schemas inheriting the same selected physical requirement still reject.

The positive result is a pending review, never an accepted lock. The test
does not authorize package code or replace an audit.
