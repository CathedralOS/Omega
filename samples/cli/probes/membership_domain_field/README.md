# membership_domain_field

First sample with a slice-predicate domain gating a data
field: `domain [u8]::NonEmpty(bytes) requires
non_empty(bytes)` + `data Message requires text in NonEmpty` —
the `"omega"` literal's bytes establish the otherwise
zero-invalid gate at construction.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
