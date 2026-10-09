# Selected reference lifetime

```text
omega run --both tests/omega/pass/ownership/runtime_selected_reference_local_exit/main.omg
```

Both engines must exit zero. The helper selects between two distinct incoming
reference backings, reads through the selected descriptor in an ordinary call,
and returns a scalar. Both selections are exercised. The caller independently
selects a reference to activation-owned primitive storage and reads it through
the same helper. Both local backings become writable after the selected
descriptor's last read, and the caller observes both writes.

The materialized descriptor owns its affine lifetime resource even though its
referent access is shared. Its branch transfer and final retirement must not
copy the backing, leak the descriptor, or create a second disposal. This is an
ordinary-call control, not evidence of suspension preservation.
