# Fixed-layout reference readback

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**A reference loaded from fixed-layout storage retains its selected representation policy and live referent while the adjacent integer field keeps its value.**

The generic supplier writes the input reference and scalar 7 into the existing `EnvelopeLayout` representation, then loads both into an ordinary `Readback` carrier. Its declared output region remains the input pointer's region. A separate caller consumes the returned carrier after the temporary laid holder is gone, observing pointee 43 and scalar 7. Both independent input regions remain explicit; the consumer supplies the carrier's declared lifetime argument.

The [policy twin](invalid/main.omg) changes only the loaded reference's destination policy to `PayloadLayout<8>`, while the caller still supplies 7. Both policies have size 8 and alignment 8; equal geometry does not make the applications interchangeable. The [region twin](wrong_region/main.omg) changes only the returned carrier's region from the pointer input to the independent storage input. It rejects because the returned view still borrows `pointer`, which the changed result relation does not permit.

Optimized Rust producer `a070c131d52ba72ae94e6824ef2d8d0099fa697b` (Mac ARM64, image `84888d4736958ccf25572325932c9c1d7cac39ebd63d3c833b29708a52dffd08`) observes native/Terminal exit-zero agreement and both intended refusals in 46 seconds. This behavior already works on that retained producer; this change adds executable coverage and corrects the previously unqualified board frontier, without a compiler repair or rebuild. The earlier 33-second direct-read observation used the simpler pre-returned-carrier source and retains its own identity.

The first transport batch establishes [local](local/main.omg), [field](field/main.omg) and [parameter](parameter/main.omg), each with a returned-region-only twin. The field place carries the complete result inside `ReadbackField`; the parameter place passes the complete result to a generic inspector. Local/parameter and their twins pass on the same retained producer in the original 43-second scoped run. The field pair, corrected to avoid the imported layout module's public `Observation` name, passes in 36 seconds. The original mixed run retains its field-name failure; it is not relabeled as a wholly successful run.

The second batch establishes [call result](call_result/main.omg), [match join](match_join/main.omg) and [named constant inputs](const_name/main.omg), with the same output-region-only twins. The consumer takes the call result directly; both match arms execute; named inputs supply policy 7, scalar 7 and pointee 43 while the receiving type retains literal policy 7. The same retained producer observes all three native/Terminal exits and intended refusals in 75 seconds. No compiler rebuild was needed.

All four hosted ProgramEntry bindings and exact corpus mirrors are present. The root and six places are established on Mac ARM64. Nested machine, other machine and selected target implementation remain open, along with other-host execution, broader reference compositions and the parent capability. It does not qualify arbitrary descriptor bytes, mutable-reference transport, byte roundtrips or authority reconstructed from an address.

Rules: [representation applications](../../../../wiki/spec/layouts/plans.md#representation-applications), [returned views](../../../../wiki/spec/language/lifetimes.md#returned-views), and [carried-loan transport](../../../../wiki/spec/language/lifetimes.md#carried-loan-transport).
