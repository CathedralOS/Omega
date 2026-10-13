# match_domain_results

`domain i64::Km` — a requires-free marker domain; `as i64 in Km` inside
each match arm result delivers the `-> i64 in Km` declared return.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
