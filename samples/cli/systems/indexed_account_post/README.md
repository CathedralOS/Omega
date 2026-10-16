# Indexed account post

A value-returning post applies `Deposit`, `Withdraw`, or `Freeze` to one slot
of `[Account; 4]`. The case `where` bound `0 <= index && index < 4` is the
index proof. The selected arm counts the post, then reads or writes
`accounts[index]`. A deposit or withdrawal on a frozen slot returns the
current balance and does not change it.

`account_ledger` still reaches the same slots through equality chains. This
program is that index, kept small.

Script and expected results:

```
Deposit(0, 100)  -> 100, hits 1,  balance[0] = 100
Deposit(1, 40)   -> 40,  hits 2,  balance[1] = 40
Freeze(0)        -> 0,   hits 12, frozen[0] = 1
Deposit(0, 5)    -> 100, hits 13, balance[0] stays 100
Withdraw(1, 10)  -> 30,  hits 14, balance[1] = 30
```

Final balances are 100, 30, 0, 0. Exit 70.

The intended-invalid control constructs `Deposit` with index 4 and is rejected
because that literal violates the case constraint.

```sh
omega run samples/cli/systems/indexed_account_post/main.omg
```

Expected exit: 70. Native and interpreter both exit 70 on Linux x86_64.
