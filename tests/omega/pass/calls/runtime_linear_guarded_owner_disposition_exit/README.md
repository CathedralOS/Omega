Two linear receipts enter the same guarded terminal-call machine, selecting its true and false arms. Each selected arm transfers the whole receipt to the same ordinary consuming callee, which captures its code and transfers it to core `drop`. The exact `Receipt::drop` owner hook authorizes disposition. Both engines must preserve results 7 and 13 and exit zero; a mismatch exits one.

Run `omega run --both main.omg` after preparing the package with `omega update --project .` and accepting its review.
