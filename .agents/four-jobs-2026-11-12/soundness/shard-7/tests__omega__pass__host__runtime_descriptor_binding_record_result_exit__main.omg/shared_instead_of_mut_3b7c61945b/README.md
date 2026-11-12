# Binding custody across a record-returning state

Run on macOS arm64:

```
omega run --both main.omg < input.txt
```

The helper forwards a borrowed descriptor Binding and mutable array into a
state, then returns a record containing the read count. The program must
produce `abc\n` and exit 0 in both engines, with the untouched final word
retained after a short read and EOF.

Structural completion uses the same parameter-custody validation as Unit and
scalar completion; it does not establish a new Binding or copy borrowed storage.
