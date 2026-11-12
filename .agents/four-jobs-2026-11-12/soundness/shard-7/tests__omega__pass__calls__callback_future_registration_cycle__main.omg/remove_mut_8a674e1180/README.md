# Future-only callback linkage

Prepare the repository-owned package with
`omega update --project tests/omega/pass/calls/callback_future_registration_cycle --offline`,
accept the generated local review, and resume with `omega update --project
tests/omega/pass/calls/callback_future_registration_cycle --resume --offline`.
Then run `omega run tests/omega/pass/calls/callback_future_registration_cycle/main.omg`.

The genuine macOS pthread_key_create registrar receives a destructor whose body
can register itself again. The native code closure is cyclic, but the registrar
does not invoke the callback during registration. Its contract omits invokes.
Successful registration followed by the native _exit primitive exits 0; a
registration failure exits 71. No thread destructor is executed, and this fixture
does not establish future-root permission, registration capacity or lifetime.

Same-stack demand must follow declared synchronous reach, not every emitted
callback address. Preserve the pthread_once closure and qsort fixtures: their
registrars do declare synchronous callback invocation.
