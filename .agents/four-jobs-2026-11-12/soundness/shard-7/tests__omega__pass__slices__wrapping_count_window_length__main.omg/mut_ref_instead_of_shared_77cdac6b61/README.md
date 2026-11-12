# Wrapping count window length

A slice `items[0..count]` whose `count` is `i32 in Wrapping`, proved
nonnegative and inside the array, has length `count as u64`. The end is a
parameter, a local, or a field. The caller reads one returned window from a
call. A count that is not proved nonnegative is not an exact `u64` cast.

Native execution exits 0.
