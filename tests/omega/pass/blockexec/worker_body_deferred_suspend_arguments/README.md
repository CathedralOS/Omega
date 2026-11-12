# Selected worker suspension permission

`STATIC-MACHINE-MIGRATION`: the generic worker retains its exact callback-dependent
suspension permission until selection. The original worker declaration was
incorrectly pinned as rejecting even without a selected application. It is valid
under [conditional suspension carry](../../../../../wiki/spec/language/effects.md#conditional-suspension-carry).

This closed application passes a suspension-forbidden `Token` to the quiet
`identity` callback and returns its authored value. The [reject twin](../../../fail/blockexec/worker_body_requires_suspend_safe_arguments/main.omg)
selects the otherwise identical public `may_pause` callback, whose published
contract permits suspension. Only that selection changes admission. Both bodies
are checked; the generic declaration itself is preserved without a stronger
argument carry bound. The Build file binds all four hosted entries.

The independent [unknown-frontier control](../../../fail/concurrency/suspend_reference_permission_without_backing/main.omg)
retains its original source: an unconditional suspension requires complete loan
preservation even when the descriptor has `Carry::AcrossSuspend`. Known loans,
ordinary access, lifetime and runtime-preservation obligations are never erased
by the callback condition. The broader STATIC acceptance remains open.
