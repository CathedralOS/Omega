# Packed floating record callback

After package preparation:

```sh
omega run tests/omega/pass/calls/callback_owned_float_record_packing/main.omg
```

System libffi invokes the callback with a two-f32 record in S0/S1. The
ordinary body receives one complete integer word, calls its helper, and
returns one word that the native entry splits back into S0/S1. Exit 0 checks
the full 92/19 result and unchanged 17/91 input. This challenges byte offsets
within a word, not only redistribution of whole double registers.

The foreign type metadata uses the SDK's float kind 2 and struct kind 13;
the same real ffi_prep_cif/ffi_call exports and Darwin CIF layout are used.

The CIF is the prefix of a borrowed wrapper carrying an explicit shared
reference to its live type metadata. Its raw pointer fields do not supply
lifetime evidence.
