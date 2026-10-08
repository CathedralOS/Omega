# Binding custody across a scalar-returning state

Run on macOS arm64:

```
omega run --both main.omg < input.txt
```

The helper forwards the same borrowed descriptor Binding and mutable array
into a state that returns a scalar. Reading four bytes, reading EOF, and
writing the original array must produce `abc\n` and exit 0 in both engines.
The untouched final word detects replacement of the original backing.

Parameter custody is independent of the body's result shape. This does not
grant new Binding construction authority or weaken shared/mutable admission.
