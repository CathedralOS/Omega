#!/usr/bin/env python3
# Materialize a hand-assembled binary from a commented hex listing.
#
# This is intentionally trivial and auditable (it is NOT part of the trust root any
# more than `xxd -r` would be): it concatenates bytes. Tokens per line, `;` ends a
# comment:
#   XX      one byte, two hex digits
#   zN      N zero bytes (decimal N) — for header padding runs
# The hex listing (vm.hex) is the hand-assembled trust-root artifact; everything
# downstream is produced by running the resulting VM on checkable tapes.
import sys

def materialize(path):
    out = bytearray()
    for raw in open(path):
        line = raw.split(';', 1)[0]
        for tok in line.split():
            if tok[0] in 'zZ':
                out.extend(b'\x00' * int(tok[1:]))
            else:
                out.append(int(tok, 16))
    return bytes(out)

if __name__ == '__main__':
    data = materialize(sys.argv[1])
    with open(sys.argv[2], 'wb') as f:
        f.write(data)
    sys.stderr.write(f"{sys.argv[2]}: {len(data)} bytes\n")
