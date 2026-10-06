# Reserved generic construction

Run `omega run --both tests/omega/pass/drops/runtime_generic_reserved_construction_exit/main.omg`.
The program establishes the contained token before observing or cleaning it and
expects exit 0. It currently reaches the retiring Terminal restoration replay,
which rejects the owned construction store with `BorrowedStorageRepairMismatch`.
This is a construction-frontier control, not an accepted runtime result.
