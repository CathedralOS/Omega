# Selected indexing supplies its declared collection loan

Run `omega run --both main.omg`; both engines must exit 0.

The selected index body mutates the caller's Frame through its explicit mutable
parameter. A selected range body receives a shared ReadOnly collection and
executes its authored result. An ordinary generic slice call infers its element
only from a borrowed fixed array, not its Boolean result. The call observes the
array's actual extent. Bounds, access and value admission remain separate.
