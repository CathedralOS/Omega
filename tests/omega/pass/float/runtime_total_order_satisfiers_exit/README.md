# IEEE total-order satisfiers

This program checks signed zero, infinities and NaN payload ordering through
`F32::TotalOrder` / `F64::TotalOrder` and an `Order::before` satisfier. Its
authored success result is exit 70.

```sh
omega run --both tests/omega/pass/float/runtime_total_order_satisfiers_exit/main.omg
```

On macOS ARM64 at `7e0a5266418`, the public command returns native exit 70 and
checked-interpreter exit 70. Ordinary package preparation (`omega update`,
reviewing the proposed decisions, then `omega update --resume`) accepted 52
review rows before that run. An unaccepted package review remains a separate
prerequisite; the corpus harness supplies its reviewed fixture policy:

```sh
python3 tools/corpus_gate.py --both --filter float/runtime_total_order_satisfiers_exit
```

The baseline at `b0c5a2d9c78`, with current explicit Console/Build call spelling,
rejected native production at `computed primitive borrow lost its established
local`; checked interpretation returned 70. The repaired path gives each
immutable scalar parameter one original addressable home and retains a shared
reference carrier through helper calls. The reference's viewed type stays
separate from its backing type through serialization, native argument transport
and image validation. No copied bit value replaces the borrowed storage.
Multiple views share one home, and mutation remains blocked until every shared
loan ends. Scalar return positions still observe a reference local as a value.

The companion controls `recast/runtime_scalar_parameter_shared_views_exit`,
`recast/runtime_bool_representation_recast_exit` and
`recast/runtime_float_range_representation_recast_exit` also return 70 in both
engines. Width mismatch, mutable Boolean-to-unconstrained-byte aliasing and
fact-strengthening recasts retain their expected rejection diagnostics.

Source-to-serialized-artifact controls preserve negative zero and NaN payloads
at both IEEE widths, share one backing across differently typed views, and
reject a removed reference projection. Target translation controls exercise
ARM64 and x64; native execution above was measured on macOS ARM64.
