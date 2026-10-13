# Nested view descriptor replacement

Replacing an element-view field through a borrowed record collection installs
the completed descriptor, not its elements or its carrier address. Native and
Terminal execution must observe the replacement's two records and full-width
tag while the displaced backing remains unchanged.

Run `omega run --both main.omg` after `omega update`. The authored exit is 0.
