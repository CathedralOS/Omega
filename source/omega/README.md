# Omega compiler construction

This is the Omega compiler written in Omega. The [compilation library](compilation/build.omg)
holds the shared source-to-artifact pipeline and the [Alpha emitter](compilation/00_scalar_operations_to_alpha.omg).
It calls the sibling [Psi package](../psi) for parsing, resolution, typing,
checking and target-neutral lowering. Applications read input and publish
output; tests do not have a second compiler pipeline.

Two applications have independent package roots:

- [construction](application/construction/build.omg) takes a closed source set
  and an explicit entry, then calls the compilation library.
- [OCREQ](application/ocreq/build.omg) keeps the sealed public request format.
  Its request returns `coverage_request_semantics` until the identity, graph,
  snapshot, admission and commitment phases are implemented.

The construction application is a deliberately early interface, not OCREQ
admission. Its caller supplies package ownership, direct import aliases and
the trusted toolchain roles, and must keep aliases unique and roles trusted.
It does not acquire packages, run Build declarations, select providers or
authenticate its inputs, so it must never receive an untrusted package graph.
Unsupported language and target features fail explicitly; nothing is guessed
from filenames or source contents.

## Construct the actual compiler

[construct.py](construct.py) collects every source file the construction
application needs: its own units, the compilation library, Psi, the standard
library and the core library. It copies each file byte for byte, keeps package
and unit identities, and writes an exact manifest. The entry is
`application/construction/main.omg::entry_start`.

```sh
python3 source/omega/construct.py --input build/omega-construction/input.bin
python3 source/omega/construct.py --compiler /absolute/path/to/constructor \
  --output build/omega-construction/omega.tape
```

The first command writes the construction input and a JSON manifest that
maps every unit to its path, bytes, hash and interval in the source set. The
second feeds those same files to an executable constructor. It writes a
nonempty tape, atomically, only when construction succeeds and the source and
producer identities are unchanged. A failure records the constructor's real
status; the script never substitutes a literal or helper program for the
compiler source.

During development the Rust compiler may build
`application/construction/main.omg`. Bootstrap variants must eventually build
this same application from their documented seeds and then use it to rebuild
itself reproducibly. Neither self-compilation nor a source-only bootstrap of
it works yet. The backend handles a limited subset of checked integer and
storage code, and the compiler's own source still goes beyond it. The
refusals it reports decide the next implementation work.

## Closed source-set input

All words are little-endian unsigned 32-bit integers. Seven header words give,
in order: package count, dependency count, unit count, entry unit, entry-name
start, entry-name end, and byte-buffer length. The header is followed by:

- Package rows: name start, name end, role (`0` ordinary, `1` core, `2` std).
- Direct dependency rows: requester package, target package, alias start, alias end.
- Source-unit rows: owning package, source start, source end.
- Exactly the declared byte buffer, then EOF.

All intervals are half-open within that one buffer. Unit bytes are exact
source, not concatenated text with rewritten imports. The given unit and entry
name select a parsed declaration; ordinary semantic and safety checks still
apply. Only the source parser creates modules, from module declarations; no
filename creates one, and an unsupported module declaration must be refused,
not ignored. The current limits are 64 packages, 256 direct dependencies, 512
units, 4 MiB of source and 128 KiB per unit, and 256 bytes per name or alias.
These are resource limits, not language rules.

Alpha tape is written only when checking, lowering and emission all
complete. A failure writes no tape and exits with a construction status:
lexical 251, parsing 250, resolution 248, typing 249, checking 247,
unsupported lowering or entry 246, capacity 252, malformed framing 253. These
are not OCOUT tags. The separate [request gate](../../tests/product/request/test-request.sh)
tests OCREQ framing; the [Alpha execution cases](../../tests/product/alpha/README.md)
call the real construction application and compilation library.

Remaining work is on [TASKS.md](../../TASKS.md) and
[TASKS_BOOTSTRAP.md](../../TASKS_BOOTSTRAP.md). Competing bootstrap variants are
peers; this compiler lives outside their directories and they share it.
