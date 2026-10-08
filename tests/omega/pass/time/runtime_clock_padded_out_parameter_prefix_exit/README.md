# Clock out-parameter prefix

On macOS ARM64, from this directory:

```sh
omega run --both main.omg < /dev/null
```

Both engines return 0. The helper lends the original nested record to exact
Darwin `clock_gettime`. Its 16-byte timespec footprint must replace only the
seconds/nanoseconds prefix. The record's marker, trailing padding and surrounding
owner fields are outside that footprint; the marker and owner fields retain
their authored values. The interpreter uses its explicitly supplied clock model,
not native latency evidence, and invents no padding contents.
