A machine transition returns either a canonical `&mut i64` or a signed-byte-backed `&mut i64`, selected by a runtime Boolean. A single forwarding helper writes through the returned reference on both invocations. The canonical scalar becomes -128, byte 0 becomes 127, and neighboring byte 1 remains 85. Both execution engines must exit 0. This exercises machine returns independently of reference-valued match expressions.

The source checks with an explicit shared lifetime, but extracting the
structural-result loan leaves `select_and_write` without an executable body.
The missing custody representation is one returned descriptor retaining all
candidate source loans, rather than selecting one guessed source owner.
