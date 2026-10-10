# case_where_common

First sample with a case-where spanning payload AND common
fields: `case Within(hi) where lo <= hi` reads common `lo`
with payload `hi`; omitted declared fields fold to ZII zero
(`Within { hi: 4 }` proves `0 <= 4`).

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
