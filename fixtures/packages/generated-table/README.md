# generated-table

Build-scoped fixture. Its `build.omg` is intended to read `inputs/table.txt`
and stage generated Omega source under the package build directory once
dependency build execution is wired.

Expected package evidence:

- build-host filesystem read limited to the package source tree;
- build-host filesystem write limited to the package build/staging directory;
- generated runtime source carries no build-time authority.

