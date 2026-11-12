# Borrowed restoration carries the detached value's loan

An exclusive helper extracts a reference-bearing carrier, transforms it through
ordinary states, and repairs the exact caller field. Both the fresh-result and
identity-result branches preserve the loan into independent backing. The caller
observes the original backing value and the updated marker, returning 42.

After package review, run `omega run --both main.omg` in this directory to compare
native and Terminal execution. The independent checked-tree interpreter is a
separate corpus control, not a fallback for Terminal execution.
