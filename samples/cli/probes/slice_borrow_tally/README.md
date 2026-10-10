# Slice Borrow Tally

Depend-free compile-scope probe: slices and shared borrows — the first
sample passing `&mut [T; N]` borrowed arrays, `&T` shared borrows, and
arrays by value through calls.

- `tally(&mut main.items, &main.filter)` lends the carrier's array
  mutably and shares the filter — the callee writes `buf[2]` through
  the borrow (fixed length keeps the index provable) and reads
  `filter.keep` through the shared borrow in the same call.
- `let view: &[i32] = &main.items[..]` then takes a shared slice view
  of the whole array — legal only after the mutable lend ends.
- `consume(main.items)` passes the array by value while `view` is live —
  a copy-eligible fixed array copies rather than moves, so the owner
  place and the live view both survive the call.

Note: `&mut [T]` open slices take arbitrary length, so indexed access
needs a provable bound — `&mut [i32; 3]` keeps the length in the type.

Compile-checked only (no std dependency, no ProcessExit provider):

```sh
mbx run -p omega -- --check samples/cli/probes/slice_borrow_tally/main.omg
mbx run -p omega -- update --project samples/cli/probes/slice_borrow_tally
```

Both green on Linux x86-64; `omega.lock` publishes locally, ignored.
