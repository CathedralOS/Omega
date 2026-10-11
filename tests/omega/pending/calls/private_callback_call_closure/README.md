# Private callback call closure (pending)

After reviewing the bundled-library dependency with `omega update --project
tests/omega/pending/calls/private_callback_call_closure --offline`, run:

Accept the local packages through the normal review/resume workflow. An
unattended disposable-fixture run may use `--dangerously-accept-all`; that
accepts unread decisions and is not evidence that an audit happened.

```sh
omega run tests/omega/pending/calls/private_callback_call_closure/main.omg
```

On macOS Arm64 this must emit exactly `A\n` and exit 0. Two real
`pthread_once` calls share one initialized control. The callback calls an
ordinary helper, which writes through Console; the second registrar call must
not invoke it again. Compilation alone does not establish invocation.

The public update is the calling-commitment regression: the unclosed calling
recipe must not be captured in provider schemas and then changed afterward.
Package review now succeeds without relaxing exact identity checks. Native
production still rejects with `MissingBoundarySettlement` while lowering the
callback's own Terminal artifact. Its provider/import closure and ordinary
function/relocation custody remain unfinished; this is not a passing run case.

The foreign control matches the LP64 SDK's `pthread_once_t`:
an eight-byte long signature, eight opaque bytes, and `PTHREAD_ONCE_INIT`
signature `0x30B1BCBA`. The system SDK exports `_pthread_once`; Apple's
[implementation](https://github.com/apple-oss-distributions/libpthread/blob/main/src/pthread.c)
invokes the initializer synchronously. The control borrow is call-scoped;
this does not create a future callback root or qualify durable registration.

Windows callback entry and structural inbound arguments remain separate
acceptance requirements of CALLBACK-PRIVATE-MATERIALIZATION.
