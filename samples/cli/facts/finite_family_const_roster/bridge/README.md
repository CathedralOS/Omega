# Runtime selection before a const-only call

This production extends the [existing const-family fact](../README.md) for
**FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../../TASKS.md).

The runtime family admits widths 16 and 32. Its selected provider must reach a
closed specialization before calling the const-only family. Both implementations
are exercised at both widths: their expected results are 16/17 and 32/33. The
original literal calls remain in the program.

The [strictness twin](local/invalid/main.omg) changes only one helper call:
it sends the ordinary runtime width directly to the const-only family. Its
membership premise remains unchanged; that premise does not make the argument
static.

The frozen baseline with actual optimized compiler `17be13e1a9ff` refuses the
positive because the provider's original Value binder reaches const admission.
The twin refuses the direct runtime argument as intended. Neither reaches an
execution engine. The source and input hashes are retained with the claim.
The unchanged local program executes with native and Terminal exits 0 on macOS
ARM64 at optimized producer `d023d9db7fb8`. Its strictness twin still rejects the
direct runtime argument. The parent fact is carrying this repair through its
existing transport places; their qualification and the parent task remain open.
