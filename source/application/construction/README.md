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
and unit identities, and writes an exact manifest. Package Build declarations
and core’s default Build are recorded separately as build inputs. The entry is
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

The compiled Epsilon constructor frames its guest output. Use
`--epsilon-compiler` to check the guest exit code and extract the raw tape.
On a compiler refusal the receipt includes the guest status and the source
offset when supplied. Use `--compiler-tape` to run a published Omega
constructor on the audited Alpha seed. With a checkout held fixed and a
compiled Epsilon constructor at `$constructor`, this pair attempts two
generations and keeps their input hashes and results.

```sh
python3 source/application/construction/construct.py --epsilon-compiler "$constructor" \
  --input build/gen1/input.bin --output build/gen1/omega.tape &&
python3 source/application/construction/construct.py --compiler-tape build/gen1/omega.tape \
  --input build/gen2/input.bin --output build/gen2/omega.tape
```

Each `input.result.json` binds the input, producer, executable and full stdout
by hash. A successful receipt also identifies the published tape. Repeat
generation two on the same inputs to check that its tape is reproducible.

During development the Rust compiler may build
`source/application/construction/main.omg`. Bootstrap variants must eventually build
this same application from their documented seeds and then use it to rebuild
itself reproducibly. Neither self-compilation nor a source-only bootstrap of
it works yet. The backend handles a limited subset of checked integer and
storage code, and the compiler's own source still goes beyond it. For example,
the [scalar lowerer](../../subsystems/compiler/source_to_psi/pipeline/checked_trees_to_scalar_operations/scalar.omg)
admits a parameterless entry returning ordinary `u8` or Unit, with an
optional checked `crashes Trap` or `crashes none` ceiling. An explicit
`crash Trap;` emits Alpha’s trap instruction. A matching crash in a private
body with `crashes none` fails checking.
On normal return the Alpha tape exits with the byte result or zero for Unit.
This application’s resultless `entry_start` reads stdin and calls Console
and ProcessExit. The frontend resolves ground service ceilings and checks
them over selected calls. To emit these boundary operations, the compiler
also needs the target’s selected provider and process-entry environment.

## Closed source-set input

The current producer writes the little-endian u32 words `0, 1` followed by
package count, dependency count, unit count, entry unit, entry-name start,
entry-name end and byte-buffer length. The following rows also use u32 words.

- Package rows: name start, name end, role (`0` ordinary, `1` core, `2` std).
- Direct dependency rows: requester package, target package, alias start, alias end.
- Source-unit rows: owning package, source start, source end, path start, path end.
- Exactly the declared byte buffer, then EOF.

All intervals are half-open within that one buffer. Unit bytes are exact
source, not concatenated text with rewritten imports. The given unit and entry
name select a parsed declaration; ordinary semantic and safety checks still
apply. Each path spans its complete package-relative filename, with `/` between
components. Both constructors derive the namespace from the parent directory.
Renaming a file in the same directory preserves its declarations' names. A packet
beginning with a nonzero package count uses the original three-word unit rows
and places each unit in the package root. [Directory namespaces](../../../wiki/spec/language/modules.md#directory-namespaces)

The current limits are 64 packages, 256 direct dependencies, 512
units, a 4 MiB shared buffer for sources, paths and names, 128 KiB per unit,
and 256 bytes per directory namespace, package name or alias.
These are resource limits, not language rules.

Alpha tape is written only when checking, lowering and emission all
complete. A failure writes no tape and exits with a construction status:
lexical 251, parsing 250, resolution 248, typing 249, checking 247,
unsupported lowering or entry 246, capacity 252, malformed framing 253.

The [OCREQ application test](../ocreq/tests/request_staging.py) checks the
public protocol's separate outcome framing.

The [compiler task board](../../../TASKS.md) tracks the remaining product work.
