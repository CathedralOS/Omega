# Join-invariant transport control

`build.omg` authors the same `Optimization::CopyPropagation` and
`Optimization::DeadPureScalarElimination` selection as the two
`join_invariant_*_transport_selected` fixtures, but `main.omg` owns no
runtime index obligation, so stage 05 retains no scalar join invariant for
the `joined` header.

The passes behave identically through a different route: `probe` is still a
copy bound to `other` on both edges and collapses, `unused` is still dead and
leaves -- and, no invariant naming it, `probe`'s dead parameter is removed
too. The withheld-rewrite report is empty here as well, which is the point of
the control: a route that simply declined every selected pass would look the
same as transport succeeding, so this row pins that the selected fixtures'
`module:differs` facts come from real rewrites rather than the report being
unconditionally empty.

On the native leg the authored program exits `0`; the `bad` arm is a
`crash Trap`.
