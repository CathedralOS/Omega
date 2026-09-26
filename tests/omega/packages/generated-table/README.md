# generated-table

Build-scoped fixture. Its canonical build machine uses compiler-owned
`BuildSource` and `BuildOutput` facets to read `inputs/table.txt`, writes
`table.generated.omg` only under its fresh Output root, and explicitly hands
that retained source to the compiler. The generated `table_size` machine then
appends through the retained build checkpoint; the package does not reconstruct
a second frontend.

Expected package evidence:

- the exact static facet use and realized sponsored operations are recorded;
- read authority is limited to the package source tree;
- write authority is limited to the package staging directory;
- output does not enter compilation without `include_source`;
- generated runtime source carries no build-time authority.
