# Rollback applies on an inert rule, over a request superset

`build.omg` authors `Optimization::ControlFlowCleanup` on an empty
`main`, and `rollback.txt` declares two rules. Settlement subtracts the
authored one and ignores the request the build never selected, so the
receipt records `requested=[ControlFlowCleanup,CopyPropagation]` beside
`applied=[ControlFlowCleanup]` -- a request superset of the authored
selection is legal.

`module:same` records the inert boundary case: the authored rule is
genuinely rolled back (`effective=[]`) but its rewrite was already a
no-op on this source. On `rollback_control_flow_cleanup_selected` the
same subtraction appears beside `module:differs`, which is the vacuity
guard showing a rolled-back rule that had actually rewritten its module.
