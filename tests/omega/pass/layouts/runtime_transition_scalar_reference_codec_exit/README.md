A machine transition returns either a canonical `&mut i64` or a signed-byte-backed `&mut i64`, selected by a runtime Boolean. A single forwarding helper writes through the returned reference on both invocations. The canonical scalar becomes -128, byte 0 becomes 127, and neighboring byte 1 remains 85. Both execution engines must exit 0. This exercises machine returns independently of reference-valued match expressions.

Run `omega run --both tests/omega/pass/layouts/runtime_transition_scalar_reference_codec_exit/main.omg`
after ordinary package preparation. The returned descriptor retains its selected
backing and codec through the forwarding call. Keep the shared lifetime and
authored runtime selection unchanged.
