The acceptance command is:

```sh
cd tests/omega/pass/borrows/runtime_direct_immutable_scalar_loan_exit
omega update
omega run --both main.omg < /dev/null
```

Both engines must return zero. Direct computed calls and explicit reference
captures borrow the same immutable original, whether it is a caller local,
a scalar ingress or a scalar-body local. The mutable-ingress variation checks
that assignment and a helper's write change that same home, not a snapshot.
The program traps if any observation differs.
