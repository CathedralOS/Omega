# Applied generic result extraction

Run `omega update --project samples/cli/owned_extraction --target <host>`,
review the package changes, then run
`omega run --both samples/cli/owned_extraction/main.omg`.
Both execution engines must exit 0; a wrong payload or selected case traps.

The sample retains an applied boundary requirement's result through a generic
wrapper, and extracts an ordinary generic helper parameter. Simultaneously live
applications use different payload types. Both cases transport borrowed payloads
with an explicit backing lifetime. Linear payloads leave both a one-case carrier
and a two-case carrier exactly once, retaining their value before destruction.

The compiler must relate authored template case/field selectors to the exact
applied carrier, through checking, custody, Terminal lowering and execution.
That correspondence supplies no new case proof, backing lifetime or ownership.
The matching fixture is
`tests/omega/pass/providers/runtime_generic_case_extraction_exit`.
Reject controls under `tests/omega/fail/generics/extraction_*` exercise a foreign
case, a different application, an inactive or overwritten tag, escaping backing,
and reuse of a consumed carrier. The applied-provider backing control under
`tests/omega/fail/borrows/applied_provider_output_requires_backing` retains
concrete descriptor obligations beside symbolic fields.

This is a prerequisite exposed by Cathedral's owned placed-memory retirement
wrapper, not evidence that physical initialization is implemented. The remaining
customer acceptance is to initialize a word to 17 in qualified vacant backing,
write 29 through a helper, move out 29, and recover the exact vacant extent in
native, Terminal and Cathedral boot execution.
