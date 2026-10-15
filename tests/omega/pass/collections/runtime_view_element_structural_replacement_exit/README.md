Run `omega run --both tests/omega/pass/collections/runtime_view_element_structural_replacement_exit/main.omg`
from the repository root; native and checked execution must exit 0.

A whole-collection element view (`&mut xs[..]`) of a borrowed fixed array
preserves the array's element coordinates. Replacing a whole structural
element through that view opens and repairs the same restoration window as
replacing it through the array, so the window names the array's storage: no
Terminal route crosses into a view's element. A reference local keeps its own
`Referent` route. Scalar element writes through the live view still use the
view's runtime extent.
