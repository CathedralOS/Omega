# A selected body retains its initializer dependencies

Compared with the [positive](../../../pass/compile_time/const_overload_selected_dependency_exit/main.omg),
only the selected `u32` body changes from returning `value` to reading `ROOT`.
The header records the rejection oracle; both Build files bind all four hosted
entries. `ROOT: u32 = choose(1u32)` now has a genuine `ROOT -> ROOT` dependency.
A type error, ambiguity, or unrelated checker failure does not establish this
control. The exact diagnostic golden awaits the parent's frozen R25 observation;
no diagnostic or execution result is claimed here.
