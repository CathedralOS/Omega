# Building Omega

The [bootstrap compiler](compiler/README.md) is written in Epsilon and emits Alpha tape for its supported Omega subset. The [construction entry](construct_omega.epsilon) supplies it with the source units and package edges for the [Omega product compiler](../../source/application/construction/README.md).

The Alpha- and Linux-hosted Epsilon evaluators compiled the production wire decoder and `Request.clear` into identical runnable tapes. Building the complete product compiler and using it to reproduce itself remain open. [Shared product checks](../README.md#gamma-host-and-target-comparison) · [Whole construction results](../../tests/bootstrap/independent/WHOLE_REQUEST.md)
