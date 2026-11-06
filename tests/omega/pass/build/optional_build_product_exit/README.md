# Optional values retain their selected case

Compiling an application and its build script together must preserve the fact that zero-initialized `Optional<u64>` selects `None`. The application also constructs `Some(70)` and traps if either case is misidentified or the neighboring canaries change.

The explicit closed theorem and the library’s generic theorem remain in the source. Build imports the same library so checking cannot rely on globally unique type names. The companion `optional_product_only_exit` removes only Build’s import and type use, isolating the closed application. `fail/build/optional_wrong_home` changes only the theorem’s expected case to `Some`; the runtime body is unchanged.

The compiler retains the complete selected constructor carrier and case through proof values, substitutions and decoded static constants. Diagnostic names do not select a declaration. Extra proved domains do not change the underlying value identity; physical layout and access obligations remain attached to the surrounding observation.

Validation records and producer identities are retained in the change description. This repairs the reproduced Optional proof failure; it does not establish the full Cathedral timer milestone.
