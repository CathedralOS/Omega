# Returned float equality

A returned f64 equals the live field it came from.

`read_kept` records `result == item.kept && result == 7.0` on the returned float. `agree` reads that record as `saved == item.kept && saved == 7.0`. Local, call result, other machine, and target body carry that sentence. Each reject twin returns the other field, and the diagnostic is `cannot prove ensures contract` for the recorded equality.
