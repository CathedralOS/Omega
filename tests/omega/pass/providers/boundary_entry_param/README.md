# boundary_entry_param

First sample with a `boundary machine` entry taking a linear
parameter through the platform boundary: the state-entry
establishment is realized by the entry prologue's inbound
argument write, the body releases it through
the boundary-trait requirement `ImageHandleRelease::release`, and the boundary entry returns `i32`
(unlike bound `Main::main(main: &mut Main)` carriers).
Build is freestanding (`Subsystem::EfiApplication`), not bound
to `ProgramEntry`.

Compile-checked only: freestanding target has no hosted exit.
