# Shape area

Run from the repository root on a supported native host:

```sh
omega run samples/cli/proofs/shape_area/main.omg
```

Complete the normal local package review if requested. The program prints its
three authored messages, waits for Enter, and exits **42**, the area of its
initial 6-by-7 rectangle. EOF also releases the input wait:

```sh
omega run samples/cli/proofs/shape_area/main.omg < /dev/null
```

The unchanged program has been executed on macOS ARM64. The circle arm also
compiles: its 0..=100 radius bounds establish the safety of the intermediate
square and then multiplication by three. This uses independently checked
certificates, not an exemption from Exact arithmetic's intermediate overflow
obligations. The initial rectangle does not execute the circle arm.

Omit `--target` for execution: an explicitly selected target uses the CLI's
compile-only route.
