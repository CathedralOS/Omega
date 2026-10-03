# Structural results from terminal call arms

Run `omega run --both main.omg`; both engines must exit 0.

Each selected transition arm invokes a record-returning machine. The call's
normal result completes the caller and reaches its own caller unchanged. The
program selects both arms and checks the returned values 70 and 71.
