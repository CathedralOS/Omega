The count entry transports remaining and remaining + 0 into a named state.
The selected copy preserves rank across the call to step; step decrements it
under n > 0 before calling count again. Every joint cycle therefore contains
strict descent, and the independently enforced scalar bounds establish the
ranged member's endpoints. Count needs no explicit rank window.

The original Main machine bodies are preserved from the former
rank_range_call_computed_copy_unproven failure fixture. The ordinary program
entry invokes Main::main and crashes Trap on a result other than zero. The
adjacent Build selects that entry for each hosted target.

Run `omega run --both main.omg`; native and Terminal must both exit zero.
Frozen revision 6989229696c135c0903d210bc48e691beb1e2ba5 demonstrated both
exits on the faithful companion before this fixture migration.
