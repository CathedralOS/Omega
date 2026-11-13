# Returned record window length

A record returned from a call carries the closed length of its slice field.
One window publishes its own length. A match publishes a length only when
every arm has that same length. The caller reads the length from the local
record, from the field, and from the parameter that writes through the field.

Native execution exits 0.
