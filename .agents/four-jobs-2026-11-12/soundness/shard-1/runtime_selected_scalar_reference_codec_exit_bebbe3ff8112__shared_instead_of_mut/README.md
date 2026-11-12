A runtime Boolean selects either a canonical `&mut i64` or a signed-byte-backed `&mut i64` at the same match join. One forwarding helper writes through the selected reference on both invocations. The canonical scalar becomes -128, byte 0 becomes 127, and neighboring byte 1 remains 85. Both execution engines must exit 0.

Run `omega run --both tests/omega/pass/layouts/runtime_selected_scalar_reference_codec_exit/main.omg`
after ordinary package preparation. The selected descriptor keeps its backing
and physical codec through the call and retires its own custody at completion.
Keep the authored runtime selection and shared forwarding helper unchanged.
