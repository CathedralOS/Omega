# Forwarding a process exit

A program passes status 70 through several helpers before exiting. The selected
exit implementation must reach the operating system's exit operation with that
same status, whichever branch it takes. Native execution and the Terminal
interpreter must both exit 70.

The build explicitly selects `NativeConsole`. The unused `UnusedReturning`
alternative returns instead of exiting; its presence must not reject this
program. The compiler saves each implementation's completion check before
publishing Terminal Psi and reports an error only for a selected implementation.

The status travels from a field through a local variable, a function result,
two helpers, and two branches. This tests a checked wrapper around the external
exit operation, rather than a direct call to that operation.

Sibling invalid controls select implementations that return on one branch,
call the same API again without reaching an external exit, use an ordinary
same-named function, or replace the status. All four must still reject:

- `fail/providers/checked_completion_returning_branch`
- `fail/providers/checked_completion_service_redispatch`
- `fail/providers/checked_completion_ordinary_leaf_lookalike`
- `fail/providers/checked_completion_changed_status`
