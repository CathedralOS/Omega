# case_requires_payload

A callee whose entry requires a case reads that case's non-atomic payload field — `requires choice in Choice::Cell` establishes the case for `choice.counter` by reference and by value; an arm guard over another subject reads it without planning a dispatch.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
