# Omega compiler construction

The production [compilation library](compilation/build.omg) owns the shared
source-to-artifact pipeline and [Alpha emitter](compilation/alpha/emitter.omg).
It uses the sibling [Psi package](../psi) for parsing, resolution, typing,
checking and target-neutral lowering. Applications own input and publication;
tests do not supply a second compiler pipeline.

Two applications have independent package roots:

- [construction](application/construction/build.omg) accepts a supplied closed
  source set and an explicit entry identity, then calls the compilation library.
- [OCREQ](application/ocreq/build.omg) retains the sealed public request boundary.
  Its request still returns `coverage_request_semantics` until the missing
  identity, graph, snapshot, admission and commitment phases are implemented.

The closed constructor is a deliberate early construction interface, not an
implementation of OCREQ admission. Its caller supplies package ownership,
direct import aliases and the trusted toolchain roles. It does not acquire
packages, execute Build declarations, select providers or authenticate those
inputs. The caller is responsible for unique direct aliases and trusted roles;
this internal input is not an untrusted package-graph admission boundary.
Unsupported language and target facilities fail explicitly; these
responsibilities are not silently inferred from filenames or source contents.

## Construct the actual compiler

[construct.py](construct.py) materializes the actual construction application's
source closure: its own units, the compilation library, Psi, standard library
and core library. It preserves each file byte for byte, retaining package and
unit identities and an exact manifest. The selected entry is
`application/construction/main.omg::entry_start`.

```sh
python3 source/omega/construct.py --input build/omega-construction/input.bin
python3 source/omega/construct.py --compiler /absolute/path/to/constructor \
  --output build/omega-construction/omega.tape
```

The first command produces the common construction input and a JSON manifest
mapping every unit to its path, bytes, hash and source-set interval. The second
feeds that same actual closure to an executable constructor. It publishes a
nonempty tape atomically only on success with unchanged source and producer
identities. A failure records the real constructor's status; it never substitutes
a literal or helper program for the compiler source.

The Rust producer may build `application/construction/main.omg` as a development
ramp. Bootstrap variants must eventually construct this same application from
their documented seeds, then use it to rebuild itself reproducibly. No successful
self-compilation or source-only bootstrap of this application is claimed yet.
The current backend remains the checked scalar slice; the actual closure exceeds
that subset and its observed failures determine the next implementation work.

## Closed source-set input

All words are little-endian unsigned 32-bit integers. Seven header words give,
in order: package count, dependency count, unit count, entry unit, entry-name
start, entry-name end, and byte-buffer length. The header is followed by:

- Package rows: name start, name end, role (`0` ordinary, `1` core, `2` std).
- Direct dependency rows: requester package, target package, alias start, alias end.
- Source-unit rows: owning package, source start, source end.
- Exactly the declared byte buffer, then EOF.

All intervals are half-open within that one buffer. Unit bytes are exact source,
not concatenated text with rewritten imports. The supplied unit and entry name
select a parsed declaration; ordinary semantic and safety checks still apply.
Module declarations belong to the source parser. No filename creates a module,
and an unsupported module declaration must be refused rather than ignored.
The current provisions are 64 packages, 256 direct dependencies, 512 units,
4 MiB of retained source and 128 KiB per unit. These are resource limits, not
language meanings. Names and aliases have a 256-byte provision.

Only a completed checking/lowering/emission path publishes Alpha tape. Failure
publishes no tape and exits with the existing construction statuses: lexical
251, parsing 250, resolution 248, typing 249, checking 247, unsupported lowering
or entry 246, capacity 252, malformed framing 253. These are not OCOUT tags.
The independent [request gate](../../tests/product/request/test-request.sh)
continues to test OCREQ framing; the existing
[Alpha execution cases](../../tests/product/alpha/README.md) now call the real
construction application and production library.

Remaining work lives in [TASKS.md](../../TASKS.md) and
[TASKS_BOOTSTRAP.md](../../TASKS_BOOTSTRAP.md). Competing bootstrap variants are
peers; the production compiler is shared outside their directories.
