# build_path_edges

`Source::Path` locations are filesystem paths, not package-relative
guards. Three legal spellings land on real packages:

- `inside` — ordinary relative sibling (`dep_inside`).
- `escape` — `..` traversal into a sibling sample's dep directory
  (`../build_dep_diamond/dep_c`, package `shared_kit`). Traversal out
  of the declaring package's directory is legal; custody is enforced
  by the resolved package's own declaration, not by path confinement.
- absolute paths are equally legal (verified separately — not pinned
  here because an absolute path would depend on the checkout
  location).
