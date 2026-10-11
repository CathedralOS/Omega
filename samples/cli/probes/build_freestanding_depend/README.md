# build_freestanding_depend

`builder.freestanding = true` opts the package out of lock and install
context — it does not forbid dependency edges. A freestanding
application may still `depend` on a local sibling package: the edge
resolves against the filesystem, not the package cache, and checks
cleanly. Freestanding is an acquisition mode, not a graph restriction.
