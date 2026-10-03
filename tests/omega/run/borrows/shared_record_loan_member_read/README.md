# Shared record loan across a guard

The program reads `plain.tag` through a shared loan of `main.desc`, compares
it with the caller's zero-initialized byte, and transfers `main` to the
selected state after the loan's last use. Both execution engines must exit 0.

With the built `omega` executable on PATH, prepare the checkout-local package
review with `omega update --project tests/omega/run/borrows/shared_record_loan_member_read
--target macos_arm64`, settle the reported decisions, and resume the update.
Use the matching target on another supported host. Project acceptance is not
an audit claim.

```sh
omega run --both tests/omega/run/borrows/shared_record_loan_member_read/main.omg
python3 tools/corpus_gate.py --both --filter run/borrows/shared_record_loan_member_read
```

Construction and call replay own result-type validation. State-exit disposal
checks ownership receipts without guessing from `&T` syntax whether the result
is an alias of existing storage or a separately materialized reference carrier.
Loan closure and restoration remain independently checked.
