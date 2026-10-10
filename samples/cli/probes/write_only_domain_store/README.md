# write_only_domain_store

Whole-array store of a domain-qualified leaf through a
`&write` loan: `tagged.label = label` where `label:
[u8; 8] in Utf8` discharges `valid_utf8` membership on
the stored value — same write-side check as `&mut`.

## Learnings

Indexing a domain-qualified array member reads as an
unknown-length slice (`tagged.label[0]` fails "cannot
prove index within unknown slice length"); copy to a
plain `[u8; 8]` local to index.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
