# indexed_element_mut_loan

First sample lending `&mut` from a runtime-indexed array
element into a call parameter: `bump(&mut
rooms[from_index])` loans the selected element
exclusively; callee writes land in the caller's array.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
