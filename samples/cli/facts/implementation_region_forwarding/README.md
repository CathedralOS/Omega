# Implementation-region forwarding

Parent: **LIBRARY-REPRESENTATION-FAMILIES**.

**A forwarded implementation argument keeps its environment region and constant index independently of the caller's storage region.**

The original `Tag`, `Choice`, `Holder`, `forward` and `observed` declarations are preserved from checkpoint `ee2c8a1fceffff32a4533f83bb46d38010735a47`. In particular, `observed` still calls `forward<N>(value)`: its environment region must map to `forward`'s independently declared storage region, despite the different storage binder already in the caller. The added executable caller instantiates index 7, transports value 42 and checks the returned value. Its separate environment/storage reference parameters keep both source binders in scope.

The [region twin](invalid/main.omg) changes only the result's environment region to the independent storage region. The [index control](wrong_index/main.omg) changes only the forwarding selection from `N` to 8 while the executable caller supplies 7. Both keep the caller and value operation intact. Four additional controls reject unused phantom arguments with the wrong role/trait and reject implementation selections as ordinary or erased field types. Corpus mirrors retain the historical positive/region-negative names and add a wrong-const control.

Contents classification treats a declared implementation argument as non-storage metadata while the actual fields determine the record's contents. The marker cannot resolve as an ordinary or erased field type. Complete trait/argument/region admission remains independent; machine-binder restrictions are unchanged.

Optimized Rust producer `9df73bde3f84b50cd04cd77682afb95f46c18bd3` (Mac ARM64, image `ced04afa3402b39ffee569534002dfbd8b40f87df2b219cbf5b7679ad7bb4ace`) observes native/Terminal exit-zero agreement for value 42 and all six intended refusals in 37 seconds. The original declaration-only positive and the active caller already checked on the preceding producer; its native result classification failed before the contents repair.

The first transport batch adds [local](local/main.omg), [field](field/main.omg) and [parameter](parameter/main.omg), each with an adjacent region-only twin. Their programs keep the same original declarations, index 7, value 42 and forwarding operation. The field place forwards a data-owned implementation promise into `Holder`; [wrong data promise](wrong_data_promise/main.omg) refuses a data binder that promises `Other` rather than `Tag`.

The next batch adds [call result](call_result/main.omg), [match join](match_join/main.omg) and [named constant inputs](const_name/main.omg), again with region-only twins and exact corpus mirrors. Both match arms execute. Named constants supply index 7 and payload 42; the region-sensitive carrier is constructed in its runtime environment scope, rather than claiming a region-scoped constant facility.

The first six places are established on Mac ARM64. Local/parameter and their twins use the retained `9df73bde3f84b50cd04cd77682afb95f46c18bd3` observations. Optimized producer `674e381dc0373db7aef70bed6c1c0d9747b251c4` (image `920a3a2da2a3e0da72dcf9d2fc5591ce637cab32448926a945085b7302a02d4f`) executes field with native/Terminal exit-zero agreement and both field-related refusals in 26 seconds; call result, match join, named constant inputs and their three twins pass in 54 seconds. These are runtime observations of the recorded producers, not fresh compiler builds for later documentation commits.

This root executes the named historical continuation witness. The nested-machine, other-machine and selected-target-body places remain outstanding, as do other-host execution and the parent capability. Raw contents queries supplied only untyped implementation substitutions retain their existing conservative limitation; this root's exact application path does not establish that separate input route.

Rules: [conformance telescope identity](../../../../wiki/spec/language/conformances.md#declaration-and-selection), [closed applications](../../../../wiki/spec/language/generics.md#closed-applications-and-canonical-identity), and [lifetime binders](../../../../wiki/spec/language/lifetimes.md#binders-and-source-applications).
