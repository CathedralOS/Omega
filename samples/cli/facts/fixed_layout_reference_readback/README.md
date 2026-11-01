# Fixed-layout reference readback

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**A reference loaded from fixed-layout storage retains its selected representation policy and live referent while the adjacent integer field keeps its value.**

The generic supplier writes the input reference and scalar 7 into the existing `EnvelopeLayout` representation, then loads both into an ordinary `Readback` carrier. Its declared output region remains the input pointer's region. A separate caller consumes the returned carrier after the temporary laid holder is gone, observing pointee 43 and scalar 7. Both independent input regions remain explicit; the consumer supplies the carrier's declared lifetime argument.

The [policy twin](invalid/main.omg) changes only the loaded reference's destination policy to `PayloadLayout<8>`, while the caller still supplies 7. Both policies have size 8 and alignment 8; equal geometry does not make the applications interchangeable. The [region twin](wrong_region/main.omg) changes only the returned carrier's region from the pointer input to the independent storage input. It rejects because the returned view still borrows `pointer`, which the changed result relation does not permit.

Optimized Rust producer `a070c131d52ba72ae94e6824ef2d8d0099fa697b` (Mac ARM64, image `84888d4736958ccf25572325932c9c1d7cac39ebd63d3c833b29708a52dffd08`) observes native/Terminal exit-zero agreement and both intended refusals in 46 seconds. This behavior already works on that retained producer; this change adds executable coverage and corrects the previously unqualified board frontier, without a compiler repair or rebuild. The earlier 33-second direct-read observation used the simpler pre-returned-carrier source and retains its own identity.

All four hosted ProgramEntry bindings and exact corpus mirrors are present. This establishes the executable root only: the full nine-place readback row, other-host execution, broader reference compositions and parent capability remain open. It does not qualify arbitrary descriptor bytes, mutable-reference transport, byte roundtrips or authority reconstructed from an address.

Rules: [representation applications](../../../../wiki/spec/layouts/plans.md#representation-applications), [returned views](../../../../wiki/spec/language/lifetimes.md#returned-views), and [carried-loan transport](../../../../wiki/spec/language/lifetimes.md#carried-loan-transport).
