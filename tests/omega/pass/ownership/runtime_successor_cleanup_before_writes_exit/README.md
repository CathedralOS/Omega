# Cleanup and destination writes

The internal successor sees 17: its argument call wrote 7 before the departing
scope's hook added 10. Its own write then leaves 29. Native and Terminal
execution must both exit 0.

This pins the execution order used by cleanup-case evidence. A destination's
future writes are not completed-call invalidations in the departing state.
Calls inside outgoing arguments, ordinary calls and terminal machine transfers
retain their actual completion effects.

The `successor_argument_write_keeps_linear_debt` and
`terminal_successor_write_keeps_linear_debt` controls install a linear payload
before caller cleanup. Both must reject its abandonment; the prior Empty
observation cannot excuse either call's actual mutation.

The motivating failure was Cathedral's resident-placement outcome: an opaque
destination write footprint erased the selected case before residual cleanup.
That customer still needs resident backing/profile publication; this witness
does not claim that the resident-memory lifecycle executes.
