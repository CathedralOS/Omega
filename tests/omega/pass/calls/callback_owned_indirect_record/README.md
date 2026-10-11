# Owned indirect record callback

After package preparation, run:

```sh
omega run tests/omega/pass/calls/callback_owned_indirect_record/main.omg
```

Real system libffi passes a three-u64 record through X0 and its separate
caller-owned result destination through X8 on Darwin AArch64. The callback
calls an ordinary helper, permutes all members, then returns the complete
record. Exit 0 requires 43/17/91 and unchanged 17/91/43 input.

Its inbound promise excludes X9 and V16 from the ordinary clobber bank.
The adapter must preserve those registers across the unchanged ordinary body;
V16 preservation retains both 64-bit halves, not only a scalar lane.

The metadata uses ffi.h uint64 kind 11 and struct kind 13. The CIF prefix
wrapper retains explicit shared type metadata through the foreign invocation;
raw pointer fields do not establish lifetime or ownership.
