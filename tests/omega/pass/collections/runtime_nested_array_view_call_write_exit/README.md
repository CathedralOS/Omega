Run `omega run --both tests/omega/pass/collections/runtime_nested_array_view_call_write_exit/main.omg`; the authored result is exit 0.

The helper changes the outer selector while producing the stored value. The
assignment must still write the selected record's nested array at the captured
index, without changing its sibling element or field. The corpus `--both` gate
also checks the shared Terminal interpreter, independently of the CLI's checked
interpreter comparison.
