# Mutable matched window length

A match of two mutable two-byte windows publishes length 2. The caller binds
that call, then uses the length from the local, from a field of the record
that stores the window, and from the parameter that writes through it.

Native execution exits 0.
