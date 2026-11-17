This library unit pair runs through the existing `filesystem_receiver` test:

```sh
mbx test --locked -p compiler --test filesystem_receiver windows_transfer_count_ -- --test-threads=1
```

The factory copies the complete production
`source/library/std/targets/windows_x86_64/windows_filesystem_transfer_count.omg`
beside each program in a private project. The test uses that exact implementation
and its contracts, without exporting a private helper from std.

The positive checks zero, one, the values around `DWORD_MAX`, and `u64` maximum;
it also mutates a real three-byte prefix and verifies the original array's
suffix, short/zero completion, and transfer-error precedence over close errors.
The negative changes only the available capacity of the `DWORD_MAX + 1` request.
That original request must refuse even though its native prefix would fit.

The factory checks the negative and runs the positive in native code and Psi on
Linux x86-64. These controls use scalar counts and a five-byte array; physical
Windows I/O with a buffer above 4 GiB remains unmeasured here.
