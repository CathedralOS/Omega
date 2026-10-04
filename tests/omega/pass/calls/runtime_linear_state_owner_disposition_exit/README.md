Two linear Receipt owners move through separate local bindings, then enter a state with their same-typed parameters swapped. A terminal consuming call reads both codes and transfers each owner to ordinary core `drop`. Both engines must observe the swapped values and exit zero; a value mismatch exits one. The exact owner-designated cleanup discharges each whole claim once.

Run `omega run --both main.omg` after preparing the package with `omega update --project .` and accepting its review.
