# Optional values retain their selected case

Compiling an application and its build script together must preserve the fact that zero-initialized `Optional<u64>` selects `None`. The application also constructs `Some(70)` and traps if either case is misidentified or the neighboring canaries change.

The explicit closed theorem and the library’s generic theorem remain in the source. Build imports the same library so checking cannot rely on globally unique type names. The companion `optional_product_only_exit` removes only Build’s import and type use, isolating the closed application. `fail/build/optional_wrong_home` changes only the theorem’s expected case to `Some`; the runtime body is unchanged.

The checker remembers the exact type and selected case instead of recovering them from names. Copies from separate imports compare as the same value when they describe the same applied type. Different type arguments and cases remain distinct; layout, access and ownership checks still apply.

Validation records and producer identities are retained in the change description. This repairs the reproduced Optional proof failure; it does not establish the full Cathedral timer milestone.
