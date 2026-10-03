# Shared subslice results

A helper returns bytes 1 through 2 of caller-owned storage. Two ordinary calls
forward that descriptor before helpers check its length and read its
first byte. Native execution and checked interpretation must both exit 70.

The offset, extent, and backing must survive each return; returning the original
whole view or a callee descriptor address cannot satisfy the exit oracle.
