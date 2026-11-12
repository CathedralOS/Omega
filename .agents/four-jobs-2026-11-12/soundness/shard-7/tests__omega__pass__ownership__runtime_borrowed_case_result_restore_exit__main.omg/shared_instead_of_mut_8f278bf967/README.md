# Recoverable outcomes restore caller-owned storage

A consuming transform returns an ordinary success/failure sum with one active
linear payload carrying a lifetime-indexed shared reference. The failure arm
returns the original token; the success arm disposes it and returns its
replacement with the same referent. Both outcomes repair the same borrowed
field, retain its sibling and execute through selected control-flow cleanup.
Observations after each repair must still read 73 from the original backing;
the payload changes from 41 to 42 only on success. The data-scoped lifetime
belongs to the nominal cleanup owner's application, not a method-local binder.
Native and Terminal must both exit 0.

Run `omega run --both tests/omega/pass/ownership/runtime_borrowed_case_result_restore_exit/main.omg`.
