# Elapsed timer

From the repository root, compile and execute the authored program:

```sh
mbx run -p omega -- run samples/cli/systems/elapsed_timer/main.omg
```

Use Cargo when `mbx` is unavailable. The program prints a banner, waits for a
50 ms workload through `std::time`, and prints the measured milliseconds using
its local UTF-8 byte buffer. Its success exit is 70; the elapsed duration must
be at least 50 ms. The elapsed value is intentionally nondeterministic.

The macOS AArch64 host has the native TimeHost provider. Other declared targets
still depend on their respective native provider implementations.
