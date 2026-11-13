# Element replacement through nested views

`omega run --both main.omg` must exit 0 in both engines. An outer slice selects
a record containing a two-element mutable view. Replacement uses the inner
view's extent, updates original backing, and preserves adjacent sentinels and
the wide record field. Outer and inner extents deliberately differ.
