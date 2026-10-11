# build_subdir_edge

`Source::Path { location: "sub" }` pointing at a subdirectory of the
declaring package resolves as an ordinary independent package — the
edge is legal. The self-edge rejection (`invalid_self_edge` in
`build_sibling_checks`) applies to the root directory itself, not to
subdirectories within it.
