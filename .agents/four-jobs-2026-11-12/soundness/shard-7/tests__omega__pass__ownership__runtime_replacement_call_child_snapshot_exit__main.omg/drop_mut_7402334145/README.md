A whole-referent store through a retained exclusive alias installs nested helper
results. The later sibling mutates the same cursor, but the completed first
child retains its result. Both result guarantees and caller-visible values must
survive without evaluating either helper again.
The array-return variation keeps the same completed observations through a
local binding and owned return, without any borrowed-replacement destination.

`omega run --both tests/omega/pass/ownership/runtime_replacement_call_child_snapshot_exit/main.omg`
must exit 0 in both engines.
