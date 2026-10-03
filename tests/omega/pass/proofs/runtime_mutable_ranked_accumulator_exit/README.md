This accumulator decrements mutable current storage and increments a second
parameter before explicitly passing both current values into the next arrival. Its arrival contract preserves the total;
its rank must compare the incoming counter with the actual updated successor.
`accumulate(3, 4, 7)` returns 7, and the program exits 70.

After normal package review, run from the repository root:

```sh
mbx run -p omega -- run --both tests/omega/pass/proofs/runtime_mutable_ranked_accumulator_exit/main.omg
```
