# Named-state nested view selection

`omega run --both main.omg` must exit 0 in both engines. A runtime selector
chooses between two record-carried mutable views with different backings and
lengths. The guard bounds the inner selector, a named state replaces that
element and returns the selected view, and the caller checks its contents and
the untouched sibling backing and sentinels.

The original compiler source-checked this program but declined executable
planning of the guard's runtime-selected nested length observation. The same
ordered projection now reaches both length observation and element access.
