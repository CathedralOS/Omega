# guarded_param_subslice

A `&[Entry]` parameter subslices under an explicit guard:
`entries[start..end]` is admitted when `start <= end && end <
entries.len` dominates the edge, and the narrowed view forwards into a
`&[Entry]` callee.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
