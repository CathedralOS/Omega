# Owned floating record callback

After preparing the first-party package review, run:

```sh
omega run tests/omega/pass/calls/callback_owned_float_record/main.omg
```

System `/usr/lib/libffi.dylib` prepares an AArch64 call interface and invokes
the selected callback with a two-double record in D0/D1. Its ordinary body
calls a helper and returns the transformed record; libffi writes both result
members to the caller's result. Exit 0 requires 92/19 and unchanged 17/91 input.
The callback entry must bridge both argument and result storage, not relabel
its ordinary integer-class ABI as a homogeneous floating aggregate.

The record definitions mirror the development SDK's `ffi/ffi.h` and
`ffi/ffitarget_arm64.h`: struct kind 13, double kind 3, ABI 1, and Darwin's
extra `aarch64_nfixedargs` member in `ffi_cif`. These are genuine exported
host functions, not a test-only registration provider or compiler intrinsic.

The invocation borrows a wrapper with the CIF bytes at offset zero and an
explicit reference to the type metadata. That metadata remains live through
the native call; the inert pointer fields inside the CIF are not lifetime
evidence. Already-complete type sizes keep preparation from modifying them.
