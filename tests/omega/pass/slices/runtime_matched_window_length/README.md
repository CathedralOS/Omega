# Runtime matched window length

A match of two windows whose shared length is a runtime value publishes that
length. The caller binds the call, then uses the length from the local, from a
field of the record that stores the window, and from the parameter that writes
through it. A state-body spelling writes the other lane.

Native execution exits 0.
