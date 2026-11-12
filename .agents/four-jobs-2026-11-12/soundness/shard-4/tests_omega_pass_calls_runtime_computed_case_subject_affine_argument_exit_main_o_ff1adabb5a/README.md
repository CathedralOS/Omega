# Computed case subject with an affine argument

The computed dispatch subject takes ownership of an affine `Choice` and updates
one counter. Both case alternatives return their saved scalar payload through a
terminal machine call. Native and interpreter execution must return values 70
and 71 with exactly two producer calls, without repeating either ownership
transfer while checking the subject copies used by guards and payload reads.
