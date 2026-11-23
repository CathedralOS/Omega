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
and core’s default Build are recorded separately as build inputs. The capture skips
`.git` directories, each package’s root `build/` output, and the example packages
under its `tests/fixtures/`. A nested source file such as `tools/build/reader.omg`
keeps its complete package-relative path. The entry is
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
The [standard library Build](../../library/std/build.omg) selects `NativeProcessExit`
and `AlphaConsole` on `alpha_bootstrap`. The standalone entry adapter must
establish the process domain and supply the [read and write operations](../../library/std/targets/alpha_bootstrap/console.omg).
The current closed source-set packet records the Build inputs separately and
carries no installation of those capabilities.

## Closed source-set input

By default the producer writes the little-endian u32 words `0, 1` followed by
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
components. The Epsilon constructor uses the full path to resolve a file import.
The Omega-written constructor still passes the parent directory to its resolver. A packet
beginning with a nonzero package count uses the original three-word unit rows
and places each unit in the package root. [File identity](../../../wiki/spec/language/modules.md#file-identity)

### Including authored Build sources

Pass `--with-build-sources` to capture the Build files in the same request.
Version 2 starts with `0, 2` and the seven version-1 header words. One
additional word gives the number of Build files. The package, dependency and
Product unit tables follow in the same order. Two more tables precede the
shared byte buffer.

| Table | Rows | Words in each row |
|---|---|---|
| Selected Build source | One per package, in requester order | Requester ordinal, Build file ordinal |
| Build files | One per captured file | Supplying package, source start, source end, path start, path end |

Build file ordinals index only the Build table. Paths use package-relative
POSIX spelling. The constructor checks file ranges, unique supplier/path
pairs and one selection per requester. A package selects its own root
`build.omg`. A package whose captured inventory lacks that file selects
`defaults/build.omg` from the unique core supplier. The requesting package
remains the scope for this invocation of the default Build. All spans refer
to the original bytes, including whitespace. Product coordinates stay stable
when adding Build files to a given Product capture.

The caller must supply a complete inventory and establish the trusted core
identity. The two tables carry source provenance for subsequent checking.
They do not establish a returned Build configuration, a physical process
domain or stream handles. The Omega application exits 253 when a package selects a Build
file it does not own, except for the trusted core default described above. A
valid inventory exits 246 because the application cannot yet execute the
selected Build files. Both paths write zero tape bytes. The public OCREQ protocol has its own acquisition and
admission requirements. [Compiler request](../../../wiki/spec/build/compiler_request.md)

The current limits are 64 packages, 256 direct dependencies, 512 Product
units and 512 Build files, an 8 MiB shared buffer for sources, paths and names, 128 KiB per unit,
and 256 bytes per directory namespace, package name or alias.
These are resource limits, not language rules.

Alpha tape is written only when checking, lowering and emission all
complete. A failure writes no tape and exits with a construction status:
lexical 251, parsing 250, resolution 248, typing 249, checking 247,
unsupported lowering or entry 246, capacity 252, malformed framing 253.

The [OCREQ application test](../ocreq/tests/request_staging.py) checks the
public protocol's separate outcome framing.

The [compiler task board](../../../TASKS.md) tracks the remaining product work.
