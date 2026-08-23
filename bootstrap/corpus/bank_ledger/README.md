# Bank Ledger

A fixed-capacity transaction ledger using a `[i32; 8]` array. Combines
deposit/withdraw operations (withdrawals stored as negatives), const-indexed
array reads and writes, and a running-balance sum computed from all slots.

Transactions and expected balance:

```
deposit(500)   -> txns[0] = +500,  cumulative = 500
deposit(200)   -> txns[1] = +200,  cumulative = 700
withdraw(150)  -> txns[2] = -150,  cumulative = 550
deposit(300)   -> txns[3] = +300,  cumulative = 850
withdraw(80)   -> txns[4] = -80,   cumulative = 770
withdraw(700)  -> txns[5] = -700,  cumulative = 70
```

Final balance `500+200-150+300-80-700 = 70`. Exits **70**.

```
omega --target windows_x64 --build-dir build samples/bank_ledger/main.omg
./build/omega-program.exe   # exit 70
```

**Workaround noted:** a native miscompile triggers when a `let`-bound local
(e.g. `let slot = count`) is passed as an argument to a nested dispatch state
in repeated calls. The deposit machine therefore guards on `self.ledger.count`
directly (post-increment) rather than forwarding a captured local. The bug is
tracked at `canaries/pending/calls/let_local_passed_to_nested_state_arg_wrong`.

Exercises: `[i32; 8]` fixed array, const-indexed reads/writes in arithmetic
sum, machine-to-machine call chaining (`withdraw` calls `deposit`),
`[copy]`, multi-level dispatch states.
