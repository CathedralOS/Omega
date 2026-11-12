# Explicit lifetime entry context

`omega run --both main.omg` must exit 0 in native and Terminal execution.
The context is the exact closed `Main<'entry>` application, not a bare named
type. An invocation-lifetime helper returns a view into the provisioned backing;
reading it must observe the caller's stored 37. Lifetime erasure for layout
does not remove the ordinary result-source and access obligations.
