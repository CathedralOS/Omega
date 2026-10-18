# Nested result extent

Two contracted minima bound the exclusive end of a mutable subslice by the same view's length.

Each place computes `minimum(minimum(count, buffer.len), 2147483647)`,
forms the corresponding mutable prefix, and observes its length. The runtime
oracle covers both an eight-byte prefix and an empty prefix; the match place
executes both arms. All nine places retain the same bound and operation.

Each `invalid` twin changes only the inner minimum's length observation from
`buffer.len` to an unrelated `other.len`. That bound cannot justify accessing
`buffer`; the larger other view must not donate its extent.

This is the FILESYSTEM-PLATFORM-SERVICES prerequisite for directly clamping
native directory/read-link buffers without staging. The original local
reproduction rejects on main b148e7ded8e with:

```text
cannot prove subslice range end bound `0..capacity` is within unknown slice length
```

Run each place with `omega run --both <place>/main.omg`; both executions must
exit 0. Check `<place>/invalid/main.omg` separately: it must reject with the
fragments in that folder's `expected.txt`. A successful positive alone does
not establish the fact.
