# Returned references keep caller backing

`collect` returns an array of references into the caller's `Pair`. A runtime
index selects its second descriptor before reading the bounded integer and
Boolean fields. The descriptor's stored address, not an arbitrary possible
origin or the array's own storage, must select the original second cell.

After package review, run `omega run --both main.omg` in this directory.
Native execution and Terminal interpretation must both exit 70. The nested,
forwarded and mutable-reference array fixtures challenge the same transport;
`fail/borrows/reference_array_result_{live_source_write,local_escape,duplicate_mutable}`
keep the backing-loan and result-custody refusals separate from physical layout.
