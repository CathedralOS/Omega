# Whole elements replaced through exclusive slice parameters

Run `omega run --both tests/omega/pass/ownership/runtime_view_parameter_element_window_exit/main.omg`.

The authored exit is 0. A `&mut [T]` parameter owns none of its elements and
has no static extent, so each whole-element replacement opens and repairs a
restoration window on the view's own loan, selecting the element with a runtime
index bounded by the view's live length. Copy elements take a call result, a
copy of another index and a runtime-selected replacement between scalar writes,
directly and through a subslice. Affine elements dispose the displaced value
before an occupied store; a moved-out element is repaired with no disposal, two
moved-out elements swap, an affine field of a view element is replaced and
moved through the same selected element, and a view forwarded to a state roots
the window at its block parameter. The zeroing drop hook would expose cleanup applied to a
new value instead of the displaced one.
