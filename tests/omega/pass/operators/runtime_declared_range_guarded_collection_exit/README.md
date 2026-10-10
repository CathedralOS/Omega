# Guarded declared range collection

`omega run --both tests/omega/pass/operators/runtime_declared_range_guarded_collection_exit/main.omg` must exit 0. The selected arm evaluates the effectful collection once, invokes the authored range body, and returns 11. The unselected arm must not invoke either machine. A second selected arm observes the same caller through a shared structural loan. Explicit endpoints isolate ordinary nested-call composition from omitted-end normalization.
