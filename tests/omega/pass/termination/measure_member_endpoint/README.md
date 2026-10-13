# measure_member_endpoint

A measure rank endpoint pinned to a sibling record member:
`in 0..=countdown.limit` — the member's data-where band
(`0 <= limit <= 5`) supplies the endpoint's proven range.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
