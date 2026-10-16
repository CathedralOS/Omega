# Const-extent node ordinal

A fixed node table whose length is a const parameter, read by an ordinal
declared `u64 [0..N]`. The same machine is specialized at 6 nodes and at 4.
The 6-node coordinates are the clearance-route catalogue; indexes 1 and 3
sum to 17. The 4-node table contributes its origin through a call result
and index 2 through a parameter, summing to 4. On agreement the program
prints `17` and exits 0.

`invalid/` indexes that six-node table with a plain `u64` and must reject
with `cannot prove index `index` is within length 6`.

```sh
python3 tools/sample_native_gate.py --cli "$omega" --filter collections/const_extent_node_ordinal --jobs 1
```
