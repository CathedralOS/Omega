# Anonymous slice argument and returned view

`omega run --both main.omg` must exit 0 in both engines. A helper scans an
anonymous mutable slice argument and returns its full prefix. The returned
view retains the original array; its loan release is not a second disposition
of the anonymous argument's carrier.
