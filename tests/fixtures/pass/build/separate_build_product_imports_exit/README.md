# Separate Build and Product source frontiers

PIPELINE-ARCHITECTURE-ENFORCEMENT: initial Build prepares its local helper import
and executes a conditional binding request before Product imports are loaded.
The bindings name the Product entry inertly. After Build return, Product loads
its own helper from the same captured source namespace and returns zero in both
Terminal and native execution.

The `build_charge` SDK example inspects both preparation frontiers, checks that
the Build source bytes are shared across them, and covers one activation across
two target products. A temporary directory-root copy loses its live Product
helper after capture; deferred import discovery must still use the captured
namespace, not the host's new absence. The
`build_failure_precedes_product_import` invalid control requires an executed
helper's duplicate role to refuse before an unavailable Product import;
`product_import_checked_after_build` verifies that successful Build return does
not suppress that Product import error.

This does not establish dynamic dependency acquisition, provider installation,
or the overall application-owned Build coordinator cutover.
