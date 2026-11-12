# Exact floating-point backing at descriptor boundaries

On macOS AArch64:

```sh
omega run --both main.omg < input.txt
```

Both engines exit zero and stdout is exactly `AAA` followed by a newline.
Those four input bytes are the little-endian binary32 encoding `0A414141`,
a finite positive value less than one. No numeric conversion or lossy float
optimization supplies the output: the read changes original scalar storage
and the write observes its exact IEEE bits.

The program checks a zero-count read preserves the initializer and consumes
no input, a mutable loan retains the original backing through a helper and
state edge, and an EOF read preserves the completed value. Scalar result
checks and the byte oracle are independent.
