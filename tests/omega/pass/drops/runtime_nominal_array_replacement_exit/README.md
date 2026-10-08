Run `omega run --both tests/omega/pass/drops/runtime_nominal_array_replacement_exit/main.omg`.

Whole-array replacement materializes both new records from old element values,
then runs each selected old-value hook before writing the replacements. Cleanup
cannot zero the new records or observe an already overwritten sibling.
