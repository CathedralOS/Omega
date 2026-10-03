# Runtime interior slice recast

The program re-views the byte suffix starting at `main.bytes[main.k * 2]`
as mutable `u16` elements,
forwards the formed descriptor to another state, writes two elements, then
checks the original bytes after dropping the slice on the next state edge.
Both engines must return 70. The composite offset must be captured when the
view forms; subsequent selector changes cannot move its backing window.

```sh
omega update --project tests/omega/pass/recast/runtime_interior_slice_congruent_offset_exit --target macos_arm64
# Review the proposal and accept its required package decisions, then:
omega update --project tests/omega/pass/recast/runtime_interior_slice_congruent_offset_exit --resume
omega run --both tests/omega/pass/recast/runtime_interior_slice_congruent_offset_exit/main.omg
```

Use the appropriate target profile on another supported host. The corpus route
prepares its own provider context:

```sh
python3 tools/corpus_gate.py --both --filter runtime_interior_slice_congruent_offset_exit
```

The mutable loan suspends direct access to the byte backing in the state that
holds it, even after the last write. Readback therefore happens in a successor
that receives no slice. The negative sibling
`fail/recast/interior_slice_exclusive_backing_read` keeps that aliasing rule
observable; omitting an unsized recast's loan must not make the invalid read
acceptable.
