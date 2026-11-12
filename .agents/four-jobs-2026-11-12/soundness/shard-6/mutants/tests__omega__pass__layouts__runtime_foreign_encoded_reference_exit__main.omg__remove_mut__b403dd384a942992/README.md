# Foreign access to encoded backing

On macOS AArch64:

```sh
mbx run -p omega -- run tests/omega/pass/layouts/runtime_foreign_encoded_reference_exit/main.omg
```

`memcmp` observes the original signed-byte encoding. A checked adapter re-views
the encoded record as a canonical `i8` record before passing it to `bzero`.
The caller observes zero through its original semantic `i64` field and retains
neighboring byte 85. The physical adapter shares storage; it does not copy or
export an internal scalar pointer-and-codec descriptor.

This fixture uses native system-library imports, not Terminal host handlers.
The companion `runtime_encoded_physical_view_exit` exercises the same checked
physical view without foreign calls in Native, Terminal and the checked oracle.
