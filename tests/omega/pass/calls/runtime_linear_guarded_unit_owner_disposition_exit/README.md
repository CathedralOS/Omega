Two linear receipts enter the same guarded terminal-call machine, selecting its true and false arms. Each selected arm transfers the whole receipt to the same Unit callee, which passes it to ordinary core `drop` under the exact `Receipt::drop` owner hook. Each Unit completion returns to the caller, which proceeds to the second invocation and then exits zero. Ownership checking must account for disposition on each selected exit; the hook has no observable side effects.

Run `omega run --both main.omg` after preparing the package with `omega update --project .` and accepting its review.

The explicit expression guard and fallback complement the Boolean subject
patterns in the scalar fixture. Each alternative transfers the inherited owner
through its own ordinary terminal call.
