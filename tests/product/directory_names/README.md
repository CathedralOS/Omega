# Directory-entry name capacity

On Linux x86-64, run:

```sh
python3 tests/product/directory_names/check.py --omega /path/to/omega --receipt /tmp/directory-names.json
```

The driver stages complete private declarations from the current standard
library into an ordinary temporary application package. It does not rewrite
those declarations or implement a second decoder. The receipt records every
staged declaration hash, the producer hash and both CLI observations.

The authored program checks the real Windows record decoder, dot-name filter,
UTF-8 byte copy and portable result conversion in native and Psi execution.
It checks a 765-byte encoding (255 three-byte Unicode characters), the full
12,288-byte encoding admitted by the existing 4,096-unit native record, exact
copied bytes, oversized converted output, malformed record lengths and offsets,
and ordinary/reparse classification. Empty and oversized facade records refuse.

This isolates the pure library code; it does not load the complete standard
library or execute Windows directory/conversion system calls. The native UTF-16
conversion retains its strict flag and original error path, but malformed
UTF-16 execution and real Windows directory listing require a Windows host.
