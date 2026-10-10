# Measured record window

An array-length endpoint composed with addition retains the records' field domains
and the view's extent. A partial window subsequently selects the same second
record at index zero. Both calls store 70 without changing its Utf8 label.

```sh
omega run --both tests/omega/pass/borrows/runtime_measured_range_record_domains_exit/main.omg
```

The authored result is exit 0; a mismatched value or label crashes Trap.
