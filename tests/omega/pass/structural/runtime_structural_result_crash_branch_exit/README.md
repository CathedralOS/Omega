# Structural returns beside a crash exit

Run `omega run --both main.omg`; both engines must exit 0.

A wrapper forwards a record result through an ordinary local binding. Its caller
checks two returned values and traps only if either is wrong. The backend must
retain structural call results when another block ends in a crash: a crash has
no successor and is not a normal return.
