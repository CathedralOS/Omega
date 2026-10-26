# Named public IEEE interval

This is a second production of the [dynamic IEEE guarantee fact](../README.md),
under `RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY`.

Inline exact IEEE value `0.25` guarantee refines public inclusive f64 interval
`[0, 0.5]`; the dynamic call preserves the public interval for a precondition and observes `0.25`.

The public requirement spells its interval as the checked transparent definition
`public_interval(result, 0.0, 0.5)`. Every transport place retains the parent
shape, exact implementation guarantee, runtime value and four hosted target
bindings. A consumer requires the same interval; forwarding helpers declare it
on their own results. The parent inline production remains unchanged.

At baseline `e06334c63dc4a811b0bcf9f530cbc91b14b23eac`, optimized producer
`b3a895c26149208c72bfa1f186e0c26bee9486ca59746de925d0f7fb5f8f5520` rejects
the named public guarantee during conformance. Reduced mathematical predicates
lost authenticated IEEE relations/literals and their format. Later consumers
also lacked alias projection and the all-target public interval union.

The repair retains authenticated relations and exact format-landed literals,
projects checked positive conjunctions, and reuses the existing IEEE implication
and observation machinery. It preserves subject/formal identity and qualification
formation. Target-family results union every member's public contract, including
nonhost members; no private implementation body supplies a caller's guarantee.
IEEE comparisons never become reflexive kernel identity or integer complement.

The root and all nine places execute with native exit 0 and Terminal exit 0 on
`linux_x86_64`. Each place's invalid twin changes the implementation to `0.75`
and rejects its public guarantee. Pins are
`tests/omega/pass/facts/dynamic_float_alias_through_<place>_exit` and
`tests/omega/fail/facts/dynamic_float_alias_through_<place>_outside_interval`.
Focused `dyn_requirement_*` fixtures additionally cover alias provider/caller
composition, f32 rounding, NaN-sensitive self equality, strict/negated/inequality
controls, weaker public contracts, carrier qualifications, wrong formats,
nonhost wider contracts and distinct effectful call observations.

Run `omega update --project <place> --target linux_x86_64`, then
`omega run --both <place>/main.omg`. Raw command, source and producer hashes,
engine outcomes and reject diagnostics are retained in the task evidence archive.
No foreign native execution is claimed by the four target bindings.
Suspending requirements, crash/ranking, returned-borrow/qualification slot
metadata, package review, product parity and foreign-host execution remain in
the parent outcome's remaining acceptance.
