A later state of `settle` lends its `mut` scalar formal to `bump`, then
assigns it from the lent result. The formal owns storage from the state's
arrival, so the loan writes that storage and the assignment and the
returned value read it back: 66 becomes 69, then 70, and the program
exits 70.

After normal package review, run from the repository root:

```sh
mbx run -p omega -- run --both tests/omega/pass/borrows/runtime_state_mut_formal_borrow_exit/main.omg
```
