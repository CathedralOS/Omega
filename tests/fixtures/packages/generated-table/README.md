# generated-table

Build-scoped fixture. Its canonical build machine uses the exact toolchain-owned
`FilesystemHost` to read `inputs/table.txt`, writes `table.generated.omg` only
under its fresh Output root, and explicitly hands that retained source to the
compiler. The generated `table_size` machine then passes through the ordinary
final frontend with the rest of the package.

Expected package evidence:

- the exact static and realized build-host service use is recorded;
- read authority is limited to the package source tree;
- write authority is limited to the package staging directory;
- output does not enter compilation without `include_source`;
- generated runtime source carries no build-time authority.
