# build_helper_call

The build activation is a real machine call graph: `build` may invoke
sibling machines in `build.omg`, handing `builder` on. Declarations a
helper makes against the same `&mut Build` bind to the one activation
— the exclusions selected in `configure` apply exactly as if spelled
inline in `build`.
