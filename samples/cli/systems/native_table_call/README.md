# Native table calls

On Windows x86-64, run `omega run samples/cli/systems/native_table_call/main.omg`.
Expected exit: 0.

The program resolves `QueryPerformanceCounter` through ordinary DLL imports,
stores its address in a named table field, and selects `VtableField` dispatch.
The function writes the table's leading counter field. Both a local table and
a nested table forwarded through a checked helper must observe a positive
counter and a successful return. The imported names also exercise fixed byte
array loans across the native ABI.

This is a native host test, not a portable interpreter model of the Windows
counter. It isolates protocol-style table calls needed by Cathedral. It does
not establish Cathedral boot, dispatch-only `TableFunction` transport, callback
materialization within a table call, or non-x86 table-call encoding.
