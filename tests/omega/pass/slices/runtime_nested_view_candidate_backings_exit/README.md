# Nested view candidate backings

`omega run --both main.omg` must exit 0 in both engines. The outer slice holds
two records whose views borrow different arrays and have different lengths.
The helper replaces an element in the selected record's view and returns it.
The caller checks the returned payload and both arrays' untouched sentinels.

Descriptor origin analysis must retain every possible backing when it cannot
retain a selector's coordinates; it cannot use the first successful candidate
as the entire result. Execution still uses the authored selector.
