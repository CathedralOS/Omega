# SHA-256 known-answer vectors

`vectors.tsv` records the exact empty, `abc`, and 56-byte FIPS 180-4 SHA-256
known-answer inputs as lowercase hexadecimal bytes and their 32-byte digests.
The focused gate decodes these pins; it does not derive their expected values
from the host hash library.

Padding-boundary and maximum-extent cases are deterministic differential
controls generated beside the gate. The existing two-unit-import fixture owns
the separately pinned canonical `OMGCOMP1` envelope digest.
