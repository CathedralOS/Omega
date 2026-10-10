`omega run --both tests/omega/pass/borrows/runtime_mutable_reference_captures_selector_exit/main.omg`
must exit 0 in both engines. The mutable reference freezes element zero before
its selector changes to one. Optional alias erasure must not substitute the
current selector when the captured reference replaces its whole record.
