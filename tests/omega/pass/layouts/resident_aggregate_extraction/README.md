# Aggregate resident extraction

Export `attempt` using `omega inspect-terminal --machine attempt --target
uefi_x86_64 --output <artifact> main.omg`. The embedding supplies an admitted
vacant extent at base 4096 with 48 actual bytes initialized to `0xa5` and a Stable
read/write profile.

The source initializes and extracts a nominally linear Word containing a scalar,
a two-element array and a nominally linear nested record. It checks 17, 23, 29
and 31 respectively, then returns the vacant extent and intact Word. The result
must carry two distinct whole claims: the original extent and the original Word.
Word's nominal claim retains its nested obligation; it is not a claim minted from
the decoded bytes. The host checks the original allocation identity, geometry,
origin and lineage, the 32-byte encoded image and the unchanged 16-byte tail.

This is interpreter lifecycle evidence through the public embedding API, not a
native application or Cathedral boot claim. Cathedral's separate native witness
exercises a scalar record; it does not establish this aggregate's native oracle.
