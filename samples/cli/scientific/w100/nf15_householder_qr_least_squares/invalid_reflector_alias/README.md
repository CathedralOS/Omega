# Invalid reflector scratch alias

This separate complete source intentionally creates reflector scratch as a
shared slice of factor backing, writes factor[0], then reads the scratch slice.
The scratch loan remains active across the write. The intended rejection is an
ownership diagnostic naming `reflector_scratch`, matching `expected.txt`.

This is not a runtime rank-deficient case: those are valid inputs producing
an explicit unsupported status. This control violates the required storage
separation and must be rejected before execution. Its build has the same hosted
program-entry bindings and std dependency as the two valid sample projects.

```sh
omega --check samples/cli/scientific/w100/nf15_householder_qr_least_squares/invalid_reflector_alias/main.omg
omega run --target linux_x86_64 samples/cli/scientific/w100/nf15_householder_qr_least_squares/invalid_reflector_alias/main.omg
omega inspect-terminal --machine Main::main --target linux_x86_64 samples/cli/scientific/w100/nf15_householder_qr_least_squares/invalid_reflector_alias/main.omg
```

The activated executor starts Omega but resolver cache custody rejects before
source validation. Intended rejection is documented but unobserved; an
environmental failure does not qualify this control. The fixture
`tests/omega/fail/slices/call_endpoint_view_backing_write/main.omg` at the base
revision provides an existing example of the same lifetime rejection category.
