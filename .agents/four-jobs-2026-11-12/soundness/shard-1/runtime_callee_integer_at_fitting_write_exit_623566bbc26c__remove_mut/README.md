A direct helper and a forwarding helper write fitting signed-byte values through an ordinary `&mut i64` loan. The caller observes 127 and the neighboring byte remains 85; both execution engines must exit 0.

Before those loans, a helper reborrows the caller's record and writes the
neighboring byte. The following ordinary byte store must retain its destination
origin; an unrelated shared-reference transport query cannot make it unknown.
