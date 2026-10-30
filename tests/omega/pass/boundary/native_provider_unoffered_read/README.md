# Unoffered descriptor operation

This Linux runtime-admission control calls the exact libc `read` operation
through an ordinary boundary trait. Native execution reads the supplied `A`.
Terminal execution must decline because its descriptor adapter deliberately
does not offer Linux libc input. It must not run that operation against the
generic native worker's null stdin and fabricate exhaustion of the supplied
stream. The trait's own service reach does not identify an input implementation.

`tools/ci/compiler_retained_foreign.py` requires the actual native zero exit
and the explicit Terminal I/O-capability refusal. This is an intentional
unavailable comparison, not a positive native/Terminal agreement witness.
