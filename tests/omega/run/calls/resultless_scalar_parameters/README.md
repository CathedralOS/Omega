# Resultless scalar parameter transport

Prepare the bundled-library dependency with `omega update --project
tests/omega/run/calls/resultless_scalar_parameters --offline`, review its
decisions, and resume the update before the first public-command run.

`omega run tests/omega/run/calls/resultless_scalar_parameters/main.omg`
must emit `A` followed by a newline and return 0. The resultless helper takes
an integer and a Boolean, dispatches on the Boolean, and invokes a host
boundary. Its disabled call emits nothing.

The native and interpreted results must agree. This constrains parameter,
control-flow and call transport independently of the callback's zero-argument
initializer witness; neither helper invents a scalar return value.
