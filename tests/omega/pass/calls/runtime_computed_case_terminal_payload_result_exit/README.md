# Computed case subjects feed terminal results

Run `omega run --both main.omg`; both engines must exit 0.

A call computes the case subject. The selected arm forwards its scalar payload
into a record-returning terminal call; the caller checks both case results.
The saved subject uses the same field observation path as an authored local.
