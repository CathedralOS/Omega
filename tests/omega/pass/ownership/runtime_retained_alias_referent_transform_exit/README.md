# Retained alias whole-referent repair

An exclusive local alias extracts a whole linear referent, calls a branching
transform, and restores the same caller storage. The alias reads the repaired
value, ends before return, and permits the caller's subsequent mutation.
Both a fresh result and an unchanged returned value must preserve that route.
An alias chain also selects a field after an unrelated scalar in its parent;
the caller must see the field's repair and the untouched sibling afterward.

Prepare packages with `omega update --project . --dangerously-accept-all`,
then run `omega run --both main.omg`. Both engines must report exit 42.
