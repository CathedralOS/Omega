# Computed case subjects feed terminal results

Run `omega run --both main.omg`; both engines must exit 0.

A call computes the case subject and increments a counter. The selected arm
forwards its scalar payload into a record-returning terminal call. The caller
checks values 70 and 71 and exactly two subject evaluations across both arms.
Saved expressions and authored locals use the same field observation path.
