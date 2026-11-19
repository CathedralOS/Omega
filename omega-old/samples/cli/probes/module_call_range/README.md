# module_call_range

First sample with a module-qualified machine call as a
const contract endpoint: `const GOOD: u64 = 2` checked by
`requires GOOD <= selected::limit()` — sibling files `other` /
`selected` each define their own `limit()`; the qualified
spelling selects one in const context.

Compile-checked only: no standard-library dependency and no
`Main::exit` — run a check target, not native.
