A helper returns a view through a descriptor stored in the argument's
container, here a direct `&mut windows[..]` with no replacement. Passing the
returned view on reborrows through the container's stored loan rather than
competing with it.

Expected exit: 0 in native and Terminal execution. Run the corpus gate with
`--both` and this fixture's name.
