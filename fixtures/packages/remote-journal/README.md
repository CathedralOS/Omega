# remote-journal

Retained-authority fixture. Its public API combines the exact toolchain-owned
filesystem boundary with a package-local network boundary. A canonical network-
danger fixture still depends on a toolchain-owned network surface.

Expected package evidence:

- declared and realized reach include both filesystem and network authority;
- declared and realized synchronous invocation include both services;
- an update from this authority set to the same set still recommends source
  audit because dangerous authority is retained.
