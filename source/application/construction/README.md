# Omega compiler construction

The construction application reads a closed set of Omega sources, calls the
[compiler driver](../../subsystems/compiler/driver) and writes Alpha tape on
success. The caller supplies package ownership, direct import aliases, trusted
toolchain roles and an entry point. It is responsible for acquiring the
packages, authenticating the inputs and providing unique aliases.

The [public request application](../ocreq) accepts the sealed OCREQ format.
It returns `coverage_request_semantics` for a well-framed request pending the
identity, graph, snapshot, admission and commitment implementations.

## Construct the actual compiler

[construct.py](construct.py) collects every source file the construction
application needs: its own units, the driver, frontend, Alpha backend, wire foundation,
standard library and core library. It copies each file byte for byte, keeps package
and unit identities, and writes an exact manifest. The entry is
`source/application/construction/main.omg::entry_start`.

```sh
python3 source/application/construction/construct.py --input build/omega-construction/input.bin
python3 source/application/construction/construct.py --compiler /absolute/path/to/constructor \
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
`source/application/construction/main.omg`. Bootstrap variants must eventually build
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
apply. The current source-unit row omits the path, so this framing cannot
distinguish two package directories. Implementing
[directory namespaces](../../../wiki/spec/language/modules.md#directory-namespaces)
requires carrying each unit's logical package-relative path from the producer
to the resolver. The current limits are 64 packages, 256 direct dependencies, 512
units, 4 MiB of source and 128 KiB per unit, and 256 bytes per name or alias.
These are resource limits, not language rules.

Alpha tape is written only when checking, lowering and emission all
complete. A failure writes no tape and exits with a construction status:
lexical 251, parsing 250, resolution 248, typing 249, checking 247,
unsupported lowering or entry 246, capacity 252, malformed framing 253.

The [OCREQ application test](../ocreq/tests/request_staging.py) checks the
public protocol's separate outcome framing.

The [compiler task board](../../../TASKS.md) tracks the remaining product work.
