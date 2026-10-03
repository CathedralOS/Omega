# Borrowed case payload across a named state

After ordinary checkout-local package review, run:

```text
omega run --both tests/omega/pass/calls/case_scrutinee_self_dispatch_exit/main.omg
```

Both engines must exit 0. The authored program remains unchanged. The selected
case's record payload lends its original storage to a named state, which calls
an ordinary helper through that loan. State argument correspondence must not
depend on an explicit `&` token or the retired receiver spelling in the fixture
name.

`borrows/case_payload_state_loan_exit` additionally checks the actual returned
payload values. Package acceptance here is local test preparation, not an audit
claim about the bundled packages.
