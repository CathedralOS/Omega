# Layout-dependent array extent

`CHECKED-PACKAGE-REUSE` requires retained layout inputs to survive source
completion and body-dependent queries at the same precision. This program
uses `size_of<View>()` only through a fixed-array extent, then stores and
reads element seven. The authored plan produces eight elements; both native
and Terminal execution must exit zero.

The [reject twin](../../../fail/layouts/recipe_only_array_extent/main.omg)
changes only the plan extent to four. It must reject the same element-seven
access. The Terminal inspection additionally checks that the transported
condition belongs to the actual array consumer, that rebinding it to an
unrelated machine rejects, and that changing its geometry observation rejects
receiving admission independently of the remaining conditions.

The repair preserves exact lexical selections in their original preparation
rather than scanning predecessor expression history. Target capsules close
only their semantic recipe positions and discharge every consuming obligation
in its own application. This fixture establishes that indexed-array mechanism;
it does not close persisted reuse, parent timing or other recipe acceptance.
