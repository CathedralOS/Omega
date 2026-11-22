# process_exit_binding

The entry writes through `Console::write_line`, then calls `ProcessExit::exit_process(pick(35))`. Its `reaches StandardOutput + ProcessExit` ceiling covers both operations.

Compile-checked only: run a check target, not native.
