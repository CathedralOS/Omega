# Guarded view copy across a cycle

A shared input view and mutable fixed-buffer owner travel through a copy/store
cycle. The guard must establish the store state's exact incoming slice extent
and scalar bounds on every iteration. The native oracle checks copied bytes,
the untouched tail, zero-limit behavior, and stopping at the input extent.

This is the explicit-parameter form of the copy graph in Squalr's
`ProcessInfo::set_name` at application revision `b166d0f`. It does not establish
full application execution or excuse the application's remaining receiver-syntax
migration.

Run `omega update --project tests/omega/pass/slices/runtime_guarded_view_copy_cycle_exit`,
then `omega run tests/omega/pass/slices/runtime_guarded_view_copy_cycle_exit/main.omg`.
Expected native exit: 70.
