# sum_member_elision

First sample eliding a sum-typed record member whose first
case has scalar payloads: `Snapshot {}` and
`Snapshot { region_count: 4 }` both establish — zero mints
`Unaligned`'s tag cell and `offset: 0`; the `[u8; 8]`
padding member elides too.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
