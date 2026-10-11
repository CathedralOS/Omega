# Private callback call closure

After reviewing the bundled-library dependency with `omega update --project
tests/omega/pass/calls/private_callback_call_closure --offline`, run:

Accept the local packages through the normal review/resume workflow. An
unattended disposable-fixture run may use `--dangerously-accept-all`; that
accepts unread decisions and is not evidence that an audit happened.

```sh
omega run tests/omega/pass/calls/private_callback_call_closure/main.omg
```

On macOS Arm64 this must emit exactly `A\n` and exit 0. Two real
`pthread_once` calls share one initialized control. The callback calls an
ordinary helper, which queries the real `getpid` import before writing through
Console, dispatches through a runtime implementation selector, and writes an
addressed byte literal; the second registrar call must
not invoke it again. The PID result
controls a branch so the callback-only import cannot disappear as unused work.
Compilation alone does not establish invocation.

The initializer promises to preserve X9 and V16, unlike its ordinary body.
Even though void(void) needs no value conversion, this clobber difference
requires an entry bridge; it cannot alias the ordinary body address.

The public update is the calling-commitment regression: the unclosed calling
recipe must not be captured in provider schemas and then changed afterward.
Package review succeeds without relaxing exact identity checks. Native
production admits the callback's own provider closure and emits an ordinary
body object. Composition retains its helper and import closure without
transplanting artifact-local machine identities into the application.
The two registrar occurrences share a source body but have distinct private
entry identities; their duplicate imports deduplicate by atomic locator.

The foreign control matches the LP64 SDK's `pthread_once_t`:
an eight-byte long signature, eight opaque bytes, and `PTHREAD_ONCE_INIT`
signature `0x30B1BCBA`. The system SDK exports `_pthread_once`; Apple's
[implementation](https://github.com/apple-oss-distributions/libpthread/blob/main/src/pthread.c)
invokes the initializer synchronously. The control borrow is call-scoped;
this does not create a future callback root or qualify durable registration.

Windows callback entry and structural inbound arguments remain separate
acceptance requirements of CALLBACK-PRIVATE-MATERIALIZATION.
