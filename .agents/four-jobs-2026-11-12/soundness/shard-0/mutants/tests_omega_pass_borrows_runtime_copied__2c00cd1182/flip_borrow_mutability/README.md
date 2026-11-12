# Copied range coordinates retain record obligations

Run `omega run --both main.omg`. A copied collection measure forms a
mutable window. Replacing that scalar after formation must not retarget the
view. A direct index reads that same captured extent before the callee writes
its second record without discarding the
record's Utf8 membership. Both executions must exit 0.
