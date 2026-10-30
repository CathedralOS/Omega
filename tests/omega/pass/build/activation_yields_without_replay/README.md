# Build scheduling preserves one activation

This witness covers the resumable-activation obligation of
`PIPELINE-ARCHITECTURE-ENFORCEMENT` and `TERMINAL-BUILD-SEED`.
Its project request precedes a terminating local computation, and its entry
bindings follow that computation. Replaying the earlier effect would produce
duplicate project roles and reject completion. The main program exits zero in
native and Terminal execution on the executing host.

The existing `compiler` example `build_charge` also runs this source through
the production Build path. It asserts one activation, more than 1,024 consumed
fuel units (the production scheduling slice), and zero retained live storage
after completion. A second invocation with one fewer total fuel unit must refuse
for fuel exhaustion and retire its storage. Local replenishment cannot increase
that total allowance.

This does not establish dependency acquisition or provider readiness: those
remain part of the parent task. The ordinary Build executor still refuses
unsettled dependency requests before Product compilation.
