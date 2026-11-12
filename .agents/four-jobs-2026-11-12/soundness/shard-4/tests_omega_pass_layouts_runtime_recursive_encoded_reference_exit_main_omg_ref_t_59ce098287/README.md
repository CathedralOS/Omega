A ranked tail-recursive helper returns the same exclusive encoded reference
through four backedges. Two calls write -128 and 127 through the returned
reference, observe signed decoding, and preserve the adjacent byte 85.
There is no carrier-width copy or delayed copy-out.
Another ranked loop calls an ordinary store helper with 127 on each arrival;
that actual proves fit inside the cycle, not just at the first outer call.
Recursive selection also returns either the encoded byte or an ordinary i64.
The selected ordinary reference reads 257 and writes 127, distinguishing a
full-width read/store from an accidental one-byte codec. Stores fit both
declared candidates; selection does not erase the union's fitting obligation.

From this directory, run `omega run --both main.omg`. From the repository root,
run `python3 tools/corpus_gate.py --both --filter
layouts/runtime_recursive_encoded_reference_exit`.
