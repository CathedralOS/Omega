# Direct borrowed sum replacement

After package review, run:

```text
omega run --both main.omg
```

Expected native and Terminal exit: 0.

Direct assignment populates an empty borrowed sum and clears a populated one.
The assignment owns disposal of the old active payload; the source must not
detach and drop that value first. The hook zeros its receiver, so disposing the
new payload instead would change the observed result. A previously moved
disjoint sibling stays moved while the other sibling retains its contents.
