# Private callback slots

`WindowClass` has one source-visible field. Its fixed layout `Spread` places that field at offset 0 and two private `WindowProcedure::call` slots at offsets 8 and 16. Each `PrivateCallbackSlot<WindowClass, Spread, WindowProcedure::call>` conformance names a distinct slot that the policy places explicitly. A caller can access `id`. The private slots have no source projection.

The entry stores 7 in `id` and checks the value through the selected layout. On Linux x86-64, interpret it with `omega run build.omg`. To check the compiled layout too, use `omega build build.omg` and launch the published executable. The example exercises storage placement. A registrar and callback invocation are separate operations.
