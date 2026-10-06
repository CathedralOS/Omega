Run `omega run --both tests/omega/pass/collections/runtime_nested_record_element_window_store_exit/main.omg` from the repository root; the authored result is exit 0.

Both the refined field and the nested primitive-array element are written
through a borrowed window of records. The caller must observe the selected
backing at physical index 2, without changes to neighboring elements or fields.

The corpus `--both` gate also runs the shared Terminal interpreter; the CLI's
`--both` comparison runs the checked interpreter.
