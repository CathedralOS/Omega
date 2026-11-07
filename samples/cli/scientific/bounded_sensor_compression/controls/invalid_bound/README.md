This complete source must fail for writing index 16 of a 16-element array.
It exercises the documented scalar index rule in
wiki/spec/language/numeric_values.md#collection-bounds, independently of the
codec, console provider, and runtime invalid-input cases. The expected fragment
uses the same diagnostic as the existing nested_array_element_view_out_of_bounds
corpus control. A package, syntax, or entry-binding failure is not a pass.
