# Implementation-region forwarding

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**A forwarded implementation argument keeps its environment region and constant index independently of the caller's storage region.**

The original `Tag`, `Choice`, `Holder`, `forward` and `observed` declarations are preserved from checkpoint `ee2c8a1fceffff32a4533f83bb46d38010735a47`. In particular, `observed` still calls `forward<N>(value)`: its environment region must map to `forward`'s independently declared storage region, despite the different storage binder already in the caller. The added executable caller instantiates index 7, transports value 42 and checks the returned value. Its separate environment/storage reference parameters keep both source binders in scope.

The [region twin](invalid/main.omg) changes only the result's environment region to the independent storage region. The [index control](wrong_index/main.omg) changes only the forwarding selection from `N` to 8 while the executable caller supplies 7. Both keep the caller and value operation intact. Corpus mirrors retain the historical positive/region-negative names and add a wrong-const control.

This root executes the named historical continuation witness. It does not close the nine-place row or the parent capability. Native/Terminal observations and their exact producer belong in the progress PR; other-host execution remains a separate frontier.

Rules: [conformance telescope identity](../../../../wiki/spec/language/conformances.md#declaration-and-selection), [closed applications](../../../../wiki/spec/language/generics.md#closed-applications-and-canonical-identity), and [lifetime binders](../../../../wiki/spec/language/lifetimes.md#binders-and-source-applications).
