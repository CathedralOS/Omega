# Whole construction request

The construction adapter collects package rows, dependency rows and all source
units consumed by the Omega construction application. The experiment gives its
exact bytes to the complete Epsilon bootstrap compiler and construction entry.
The evaluator receives both the Epsilon source and the guest stdin in one sealed
EEREQ packet. [Construction input format](../../../source/application/construction/README.md)

## Build from the seed

The same complete Epsilon source can run in the evaluator or go through the
Epsilon-to-Delta compiler. From the repository root, prepare both requests in
a fresh directory. The script packs the source manifests with the repository's
built `xtask`. The Linux launcher constructs the executables from Alpha and Beta.

```sh
work=/tmp/omega-whole-request
python3 tests/bootstrap/independent/whole_request.py "$work" --entry-control
tests/bootstrap/linux_x86_64/bootstrap --delta . "$work/compiled" "$work/compiler.dcreq"
```

The compiled route first turns the Epsilon-written bootstrap compiler and its
construction entry into a Delta module. Frame a successful result with the
runtime selected by the Epsilon compiler, then pass that program to Delta.
These steps keep the intermediate sources available for inspection. The last
command runs only after both compilers have produced their executables.

```sh
set -e
"$work/compiled/successor" < "$work/whole.epsilon" > "$work/module.out"
python3 tests/bootstrap/independent/whole_request.py "$work" --frame-result "$work/module.out"
"$work/compiled/program" < "$work/omega.dcreq" > "$work/omega.gamma"
"$work/compiled/gamma-compiler" < "$work/omega.gamma" > "$work/omega"
chmod +x "$work/omega"
"$work/omega" < "$work/product.bin" > "$work/compiled.eeout"
```

To run the same checked program directly in the Delta interpreter, give it the
`omega.delta` produced by `--frame-result` and the desired product input.
The runner saves the guest's result in `guest.output`.

```sh
python3 tests/bootstrap/interpreted/delta/interpreter.py "$work/direct" \
    --host linux --profile construction --runner "$work/compiled/gamma-compiler" \
    --source "$work/omega.delta" --stdin "$work/product.bin"
```

The interpreted route passes the Epsilon source and the product input together
as one sealed request. Reuse the seed-built Delta and Gamma compilers from
above to build the interpreter.

```sh
set -e
"$work/compiled/program" < "$work/evaluator.dcreq" > "$work/evaluator.gamma"
"$work/compiled/gamma-compiler" < "$work/evaluator.gamma" > "$work/evaluator"
chmod +x "$work/evaluator"
"$work/evaluator" < "$work/whole.eereq" > "$work/product.eeout"
```

Both executables emit the Epsilon guest's outcome before its payload. A successful
constructor returns five zero bytes followed by the Alpha tape.
The optional `--entry-control` also frames the original package and dependency
rows with just the original entry unit in `entry-unit.bin` and seals the same
bytes for the evaluator in `entry-unit.eereq`. Both construction
requests select Delta profile 2, which admits the combined source and product
input and can publish a generated module larger than 4 MiB.

## Verify the Omega self-rebuild

From the checkout that supplied `product.bin`, run the seed-built Epsilon
constructor on the complete construction application. Supply a fresh output
directory. Both subsequent compilations receive the same frozen bytes.

```sh
python3 tests/bootstrap/independent/whole_rebuild.py \
    "$work/product.bin" "$work/omega" "$work/self-rebuild"
```

The first output is an Omega compiler tape. The verifier stamps it on the
checked Alpha seed, runs it on the complete application, and repeats with
the compiler it produced. It compares the second and third tapes byte for
byte. Each producing Omega compiler must also compile two programs differing
by one digit. Their tapes must return 42 and 43 respectively. A trailing
input byte must exit 253. A Boolean returned from a byte entry must fail
typing or checking. Both refusals must produce zero output.

`result.json` records the Epsilon executable digest, input and output digests
and observed exit for each process. Match that first executable digest to
the preceding seed-construction record to establish its origin. To reuse a
completed first generation, add `--reuse-tape PATH` and supply the matching
`product.result.json` from `construct.py --epsilon-compiler` beside the input.
A refusal leaves the recorded stages available for inspection.

## Parser storage for the 305-unit compiler

The complete compiler needs more rows than the public OCREQ V1 profile has
room for in several independent tables. On the 3,994,901-byte product at
`e8ef069cde7` (SHA-256 `4dd77c669bdd7efdc3834ffb37e2072e2ccae832c040f5bdd65c56b4dd570db5`),
the syntax of the compiler sources gives the following lower bounds for the
Epsilon parser's representation at `7baa8d6a64c`.

| Table | Minimum rows in the product | Available rows |
| --- | ---: | ---: |
| Roots | 5,612 | 4,096 |
| Explicit machine states | 5,807 | 4,096 |
| Path members | 17,145 | 16,384 |
| Machine parameters | 25,642 | 16,384 |
| Type nodes in parameter declarations alone | 44,059 | 16,384 |
| Statements | 17,479 | 16,384 |
| Expressions | 20,715 | 16,384 |

The [counter](parser-capacity.rs) reads the exact unit bytes from
`product.bin` and uses the independent V2 lexer and parser. It counted 181
whole-unit syntax trees. For the other 124 units it retried fragments ending
at a top-level semicolon or closing brace. It omitted 568 refused fragments.
Each counted root is a top-level use, data or machine declaration. Each
counted state is a nested machine with a body. Path slots include ordinary
import components, one name per top-level machine, and simple path operands.
Each typed parameter needs a base type row. Reference, array, slice and
qualification wrappers in parameter types need further rows.

The statement count includes local declarations, assignments and transition
arms. The expression count includes binary, unary, index and cast nodes,
plus literal or path operands directly beneath binary, unary and index nodes.
Other forms can increase the storage needed. To reproduce the counts on a
checkout with the same product sources, run these commands from its root.

```sh
python3 source/application/construction/construct.py --input /tmp/omega-product.bin
cargo -Zscript tests/bootstrap/independent/parser-capacity.rs /tmp/omega-product.bin
```

Doubling the type-node arena still leaves it short. The checker also stores
one checked parameter type per syntax parameter. It distinguishes locals
from parameters using the slot number 16,384, and data, references and arrays
using fixed numeric ranges. A construction profile with wider tables needs
disjoint identities and matching checked storage through emission. The public
entry must still report its specified limits and refusal coordinates.
[Parser and checker storage](../../../bootstrap/omega/compiler/scalar_compilation.epsilon) ·
[OCREQ provisions](../../../wiki/spec/build/compiler_request.md)

### Full construction and single-unit controls

With type domains stored directly in their syntax nodes at `7baa8d6a64c`,
the independent Epsilon source model processed the complete request above
and returned `000300000084130700`. Exit 3 identifies a resource refusal at
packed source byte 463,748. The parser requested path member 16,385 of
16,384 while reading `syntax` in the `ShiftRight` call in unit 17, line
1340, column 54. It had retained 14,667 expressions and 9,722 type nodes.
The run used 469.38 CPU seconds and 995,392 KiB peak RSS.
[Frozen source line](https://github.com/CathedralOS/Omega/blob/e8ef069cde7/source/subsystems/compiler/source_to_psi/pipeline/symbol_resolved_trees_to_typed_trees/expressions.omg#L1340)

The same parser also read two original units separately with empty tables.
Both reached `Complete`, including the rest of the 101,821-byte unit that
stopped the whole construction. These controls exercise syntax. Resolving
the imports and compiling the bodies require the larger program.

| Unit | Bytes | Roots | States | Path members | Params | Type nodes | Statements | Expressions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 17, `expressions.omg` | 101,821 | 104 | 246 | 3,847 | 936 | 2,053 | 722 | 3,055 |
| 19, `generics.omg` | 24,839 | 29 | 71 | 905 | 277 | 632 | 183 | 875 |

To replay the full result, pack the library at `7baa8d6a64c` as above and
append its construction entry. The resulting 897,403 bytes have SHA-256
`26a25402b0806db438d059f8736f219aca83c9ea54435fb9a9ed0c87b5dddd65`.
Pass these bytes and the product input to the source model's `observation`
function. The model used here has SHA-256
`758286e8bfea3395414a673a0ba5228c761e94f06e88df7e0f5a889fc4a538c9`.
For either single-unit replay, append the test console and parser control
to the same packed library, then pass the named unit bytes from the product
as stdin. After the five-byte successful exit header, read seven unsigned
little-endian words in table order.
[Model preparation](https://github.com/CathedralOS/Omega/blob/3cd9e720719/wiki/development/bootstrap/README.md#L398-L410) ·
[Test console](epsilon/console.epsilon) · [Single-unit parser control](epsilon/omega_parse_unit.epsilon)

The compiler at `47b757867bb` indexes free scalar machines by file, resolves
all imports, then checks and emits one file at a time against stable labels.
The caller can release its syntax before processing a later dependency. A
three-file forward call produces the same 43-byte Alpha tape as the pooled
compiler and exits 42 on the seed. Removing the dependency edge, making the
callee private or borrowing another file's import yields the original exact
refusal. The actual wire codec and its caller use four scalar parameter homes
across files and produce the same 1,607-byte tape, also exiting 42.
[Directory controls](directory_names.py) · [Product wire control](product_wire.py)

On the 3,994,887-byte construction packet
`a0e745fd4cc419d7bdee25e8590d3898ef3285549efab14b76c62c19c68f26f6`,
the per-unit parser passed the pooled unit-17 capacity limit and reached
`ValueBinding::Unresolved` in unit 31, at absolute byte 700523. The result-arm
change at `980fa9c6a6c` parses the complete original unit 31 and the
payload-bearing result in unit 37.

The source model replayed this same full packet with the typed literal change
at `9ab43b811962`. It stopped in unit 59, at byte 1,366,357, on the period in
`machine_item(...).source_span`. The nine-byte refusal was
`000200000055d91400`. The model used lazily initialized arrays, 1,580.94 CPU
seconds and 63,072 KiB of peak resident memory. Commit `20c71aef41c1` parses
that complete source file in isolation. The later isolated parser also reads
all 494 expressions of unit 66, which contains `$table = saved`. The whole
package still needs type checking and executable construction.

## Compare the complete Omega compiler across routes

The named `omega-constructor` case packs the complete Epsilon-written Omega
compiler with its construction entry. Its 111-byte input contains `main.omg`
in an `app` package. The source is `machine main() -> u8 { 42 }`. The route
requires the compiler to exit successfully, stamps the emitted tape on the
Alpha seed and executes it. A match requires exit 42 and empty stdout and
stderr. The row records the tape's size and SHA-256 in `result.json`.

```sh
python3 tests/bootstrap/route.py /tmp/whole-compiler \
  --gamma native --delta native --epsilon native --case omega-constructor
```

Select another host on any rung, or use `all` on an axis for a matrix. This
case selects the construction profile by default so the Delta source of the
compiled constructor fits. Pass `--profile conformance` to measure that
profile's byte limits.

To run the very same compiler and package on another checkout, supply the
saved Epsilon source and raw package. `--expect-omega-exit` requires the
constructor to return a tape, then runs it on the Alpha seed and checks its
host process status (0–255) and empty stdout and stderr. Adding the saved
observation also checks every emitted byte. The output directory records
both executions.

```sh
python3 tests/bootstrap/route.py /tmp/frozen-compiler \
  --gamma native --delta native --epsilon interpret \
  --source /tmp/whole-compiler/subject.epsilon --stdin /tmp/whole-compiler/stdin.bin \
  --expect-observation /tmp/whole-compiler/guest.eeout --expect-omega-exit 42
```

The following selection uses a Linux Epsilon compiler
to produce the module and a direct Delta interpreter on Linux to run the
compiled constructor. [Route runner](../route.py)

```sh
python3 tests/bootstrap/route.py /tmp/whole-compiler-direct-delta \
  --gamma native --delta native --epsilon interpret \
  --epsilon-interpreter delta --case omega-constructor
```

At `c6d64f0f3c4`, both named routes below built the complete 899,962-byte
Epsilon-written Omega compiler and ran it on the same 111-byte package.
Both produced the same 22-byte Alpha program, which exited 42 with empty
stdout and stderr. Only the host running the Epsilon-to-Delta compiler changed.

| Gamma / Delta / Epsilon | Epsilon-to-Delta compiler | CPU seconds for the 899,962-byte source | Peak KiB |
| --- | --- | ---: | ---: |
| native / native / native | 3,021,878-byte Linux executable | 85.23 | 3,690,508 |
| native / alpha / native | 2,782,676-byte Alpha tape | 624.53 | 3,690,044 |

The Linux-hosted Gamma compiler built its Alpha-targeting backend and used
it to emit the Epsilon compiler tape, SHA-256
`07da9f5c0049797971e256760b9251f9336f393ccd00940830106f9555041dab`.
Stamping that tape reproduced the exact Alpha executable that compiled the
whole source. Its 18,259,583-byte Delta response matched the Linux compiler’s
output byte for byte. Both rows reused the matching translation and final
56,583,468-byte Linux constructor by complete executable, input and output
digests. Each row ran that constructor and the program it emitted. This
comparison checks the Epsilon compiler across hosts. Both routes still
require the Linux backend to build the final constructor.

To repeat the hybrid route, run

```sh
python3 tests/bootstrap/route.py /tmp/whole-compiler-alpha-host \
  --gamma native --delta alpha --epsilon native --case omega-constructor
```

The `native / native / interpret` row also passed at `c6d64f0f3c4`. The
Linux Epsilon evaluator read the exact same 899,962 bytes of constructor
source and the 111-byte package in a 900,125-byte EEREQ, SHA-256
`ca73c3ba79fe0a3bbaacb3492fb1dbdd5c26fa6493b102c97b255ddc1260fd7f`.
The route checked the producer, input and output digests before reusing the
1,959,969-byte evaluator. It then ran the evaluator on the full request.
In 216.92 elapsed seconds it returned the same 27 bytes as the compiled
constructors. The generated 22-byte tape ran on the seed and exited 42 with
empty stdout and stderr. To reproduce, run the first command in this section
with `--epsilon interpret`.

## Running the compiled Omega constructor as Gamma on the seed

The Alpha VM built the complete Epsilon-written bootstrap compiler and ran
it on a 111-byte Omega package. It emitted a 22-byte tape that ran on the
same seed and exited 42. This package contains one Omega machine. Building
the 305-unit Omega-written compiler is a separate, open check on this route.

At `c6d64f0f3c4`, the Epsilon compiler running as Alpha tape compiled the
899,962-byte Epsilon-written Omega constructor. Its 18,259,583-byte output,
including the `compiled-storage` line, matched the Linux Epsilon compiler's
output byte for byte (SHA-256
`465430c3d84dea4b9373469e040e9a3bb43c88b46d0c0c36138563aa70a780f3`).
The Alpha run took 624.53 CPU seconds and peaked at 3,690,044 KiB. The
storage runtime and entry yielded 18,358,296 bytes of Delta. The
Alpha-hosted Delta compiler translated them in 1,213.36 CPU seconds and
peaked at 2,067,836 KiB. Its 26,354,274 bytes of Gamma matched the native
Delta compiler's output byte for byte, SHA-256
`215cd20cbc51fcdff281fb1075c474fc0afe3f4d1b89d141c18a974da6cd19cd`.

From the pinned Linux x86-64 checkout, start in a fresh directory with
`OMEGA_XTASK_BIN` pointing to a built `xtask`. The cold launcher runs the
Alpha seed, reconstructs Beta and Gamma, builds the Delta compiler on Alpha,
and uses it to construct the Epsilon compiler on Alpha.

```sh
set -e
work=$(mktemp -d)
python3 tests/bootstrap/independent/whole_request.py "$work/inputs"
tests/bootstrap/linux_x86_64/bootstrap --alpha --delta . "$work/alpha" "$work/inputs/compiler.dcreq"
"$work/alpha/successor" < "$work/inputs/whole.epsilon" > "$work/module.out"
python3 tests/bootstrap/independent/whole_request.py "$work/inputs" --frame-result "$work/module.out"
"$work/alpha/program" < "$work/inputs/omega.dcreq" > "$work/omega.gamma"
```

Give the generated Gamma to the original interpreter that the launcher built
from Beta. This avoids emitting an Alpha tape for the constructor itself,
which exceeds the seed’s tape capacity. The constructor can still emit an
Alpha tape for a smaller Omega program. The host's `xtask` packs the declared
source members. The shell and Python frame inputs, check the results and
insert tapes into the seed. The seed runs every
language implementation. [Source audit and tape-size bound](../../../wiki/development/bootstrap/source-audit.md#why-the-full-constructor-changes-the-host-choice)

```sh
PYTHONPATH=tests/bootstrap python3 - "$work" <<'PY'
from pathlib import Path
import runpy
import sys
from compiled import execute, gamma_request, stamp

work = Path(sys.argv[1])
encode = runpy.run_path("source/application/construction/construct.py")["encode"]
source = b"machine main() -> u8 { 42 }\n"
packet, _, _, _ = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
(work / "small.product").write_bytes(packet)
request = gamma_request((work / "omega.gamma").read_bytes(), packet)
status, result = execute(work / "alpha/gamma-evaluator", request, timeout=None)
(work / "small.observation").write_bytes(result)
assert status == 0 and len(result) > 5 and result[:5] == bytes(5), (status, result[:40])
tape = work / "small.tape"
tape.write_bytes(result[5:])
guest = work / "small.vm"
stamp(tape, guest)
assert execute(guest, timeout=None) == (42, b"")
PY
```

The original interpreter consumed the freshly emitted Gamma and the Omega
package together as a 26,354,389-byte request (SHA-256
`d41fb115e93b27e6eb9bc72ba577367bb3c4a381beeffb73d0b8e4f955f4f044`).
It took 413.87 CPU seconds, returned host status zero and wrote five zero
bytes followed by the tape. That tape had SHA-256
`25c789ed6ce473847a1dc82854aad7b59d3ff336d4ed73a64a56c540b5d78384`.
The full 27-byte response also matched the Linux constructor's response.
Neither the interpreter nor the final seed execution wrote to stderr.

## Whole compiler result

These measurements use the Omega source and construction entry at `035eddf0c5a`.
The indexed Epsilon compiler compiled all 852,096 bytes of the bootstrap
compiler and its construction entry. With Delta's construction profile, the
Linux executable and the Alpha seed running the 4,324,897-byte compiler tape
each returned status 0, the `compiled-storage` marker and the same
18,269,324-byte Delta module. Adding the declared storage runtime and entry
gave Delta an 18,365,364-byte program. Delta checked that complete source and
published 26,398,215 bytes of Gamma with process status 0 and empty stderr.
The preceding Epsilon compiler, rebuilt with the same construction adapter,
also produced the identical 18,269,341-byte output. The conformance profile
reached its 4 MiB output guard with status 254 on the same whole source.
[Indexed lowering](../../../wiki/development/bootstrap/native.md)

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Epsilon source with construction entry | 852,096 | `01565beaf9f51478a156fa9b9ca997f5808b06ef39db66a41e4baf644ff59225` |
| Epsilon compiler Alpha tape | 4,324,897 | `7154f042d57d71bdc293bac57871d4c3bebbac7825abdea8d43e42d7a04aded1` |
| Epsilon compiler output, including marker | 18,269,341 | `60d0fce9fa52af3464e95c5d691882369e6d7417cb386258f41cde55b207a0f5` |
| Delta source with runtime and entry | 18,365,364 | `f662e1a8007b819bd02fc1571d6314e49c1fac9785247ed58e278a16ca46dcab` |
| Gamma emitted by Delta | 26,398,215 | `2c9641632eb613696428a96903e02d126a6d7fc5fdbd78c9f08f3d3469f29329` |
| Linux construction executable | 56,933,914 | `47c1b9cddde2190a5d2e600640d07c1a86597123077dc9a6a86abd7834b14c3b` |

The compact native Gamma compiler first reproduced a 226,610-byte executable.
After adding an exact-name index for calls, the Linux launcher regenerated
the 226,861-byte compiler byte for byte. It compiled the complete Gamma source
to a 56,933,914-byte Linux executable. The source-built program returned guest
exit 3 for an empty construction request and for an invalid package count.
[Native backend](../../../wiki/development/bootstrap/native.md)

Reconstructing Delta and the Epsilon compiler with the compact backend reproduced the two whole-program
outputs in the table. The Beta-built Gamma interpreter received the complete
Gamma source plus the valid 829-byte wire-package input. Its 480-second
wall limit expired with empty output.

The Linux executable compiled the portable Omega fixture into a 50-byte Alpha
tape and the two-package wire decoder into a 1,607-byte tape. Each matched the
Epsilon evaluator and exited 42 on the seed. On the full 3,955,617-byte
product, a 10 GiB resident-memory guard stopped the native executable after
194.5 seconds with empty output. This run produced neither a product tape nor
a compiler diagnostic. [Wire program and refusal controls](#product-wire-package-through-the-construction-entry)

Two truncated copies of the same product isolate input staging. Each keeps
the original header and all package and unit rows, so the constructor must
consume the declared byte buffer before it can start the Omega parser.
With the first 512 KiB of `product.bin`, the executable returned guest exit
3 at EOF and peaked at 10,400,176 KiB RSS. With the first 576 KiB, it reached
the same 10 GiB guard as the complete product with empty output. These runs
use the whole native executable identified above. To replay the shorter
input, pipe `head -c 524288 "$work/product.bin"` to `"$work/omega"`.
The 14,914-byte entry-only product separately reached its 5 GiB guard.

The construction-profile evaluator also accepted the 4,807,765-byte packet
with the package count changed to 65. The guest returned exit 3 for malformed
framing (`00 03 00 00 00`) with outer process status 0.

## Reading the source arena

After reading the package and unit rows, the [Epsilon constructor](../../../bootstrap/omega/construct_omega.epsilon)
borrows the unread console bytes. Comparing the view length with the declared
arena size detects both truncation and trailing input. Namespace checking and
compilation read directly from that immutable view. The seal on the original
input supplies the backing for its lifetime.

A compiled Linux framing control read a four-byte length, checked the unread
view length and reported the first, middle and last bytes. Using all 3,994,901
bytes of the `c6d64f0f3c4` product packet as its arena, it returned
`00 2a 00 00 00 00 20 67` with peak RSS of 29,408 KiB. Removing or appending
one byte gave `00 03 00 00 00` in both cases. With a 65,536-byte arena cut
seven bytes short, the former eight-byte copy loop used 732,544 KiB and the
view used 13,528 KiB. These controls measure framing through a seed-built
native program. The whole compiler also allocates while parsing and checking.
[Cursor and bounds controls](../conformance/README.md)

## Staging the 304-unit construction input

The earlier constructor copied eight bytes from the console in one machine
state. Its predecessor visited three states for every source byte.
The declared four-mebibyte array bounds every write. Console EOF is stable,
so a missing byte anywhere in a group makes its last read return `-1`.
The constructor discards that group and exits 3 with no payload. It checks
the remaining zero to seven bytes individually and still requires EOF
after the declared arena. [Epsilon Console](../../../bootstrap/epsilon/LANGUAGE.md)

For a matched comparison, freeze `product.bin` from `36cb9c98760` before
rebuilding the constructor. It contains 3,950,880 bytes for seven packages,
seven dependencies and 304 units. Its SHA-256 is
`c570004e5315aa9a838b9d3070fff5cd32ddd513391a3d7cf11481e6dcec1ec0`.
Taking its first 589,824 bytes keeps all 3,872 bytes of header and rows.
The next 585,952 bytes end inside the declared 3,947,008-byte arena, so
guest exit 3 on this input measures staging without invoking the Omega parser.

The grouped constructor finished staging the 576 KiB prefix inside the
10 GiB RSS guard. It returned `0003000000` and process status 0 at EOF.
The original constructor reached the guard on the same bytes with empty
stdout and stderr. The table uses CPU seconds and Linux peak resident
memory. Both executables use the same seed-built Linux Epsilon, Delta and
Gamma compilers. Only the construction entry source differs.

| Frozen input | Original | Eight-byte groups |
| --- | --- | --- |
| Header and rows only (3,872 bytes) | Exit 3, 72,976 KiB, 1.20 s | Exit 3, 72,988 KiB, 1.09 s |
| First 16,384 bytes | Exit 3, 320,912 KiB, 9.13 s | Exit 3, 231,324 KiB, 4.39 s |
| First 589,824 bytes | Guard, 10,485,904 KiB, 259.44 s | Exit 3, 7,487,004 KiB, 211.56 s |

After subtracting the header-only measurement, the grouped constructor
used 12.65 KiB of additional peak RSS per staged arena byte on both
completed prefixes. On the complete frozen 3,950,880-byte product, the
10 GiB guard stopped it after 369.47 CPU seconds with empty stdout and
stderr. The complete run produced no tape or compiler diagnostic.

Both constructors gave the same five-byte refusal on 16 truncations of
the 828-byte wire-package input. Source bytes `0x00`, `0x80` and `0xff`
each produced the same compiler refusal at byte 30. The grouped constructor compiled the complete
input to the existing 1,607-byte Alpha tape, which exited 42.
A source merge with the directory-aware constructor from `60ddbd9c9af`
also passed the directory-import and malformed-directory cases in the published
Epsilon evaluator. Alpha ran the 43-byte directory-import tape to exit 42,
and the malformed directory returned guest exit 1 at byte 37.

| Linux constructor | Bytes | SHA-256 |
| --- | ---: | --- |
| Original | 58,603,133 | `ccc6eb867203f3250e9025e8f616d07e8b2253582b34499201ffe9e4ce4cb21b` |
| Eight-byte groups | 58,746,073 | `13dcd00473a0e54aefdaeb0bce4c1fb40d97090cd3393e7c5ce3c7a1e51d45a1` |

## Named payload bindings

The source-built Linux Epsilon compiler compiled the updated Omega compiler
and construction entry. Delta checked the resulting program and emitted Gamma.
The 226,861-byte Gamma compiler from the seed build produced the executable below.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Epsilon source with construction entry | 871,346 | `6ff7efc90597dc4bdda9eb9f7022f7927a4ddaa85ee3eb049eb278fa368fba88` |
| Epsilon compiler output, including marker | 18,787,693 | `a01cece0a189fbd0b5614db571935987ad0ce586d2b85a7361101f4344142e82` |
| Delta source with runtime and entry | 18,883,716 | `2f015c02827dd72ec1b5a701a512afc35620d3028e9a9b9d1403b03993dded33` |
| Gamma emitted by Delta | 27,145,761 | `6301a32ee1641d4167655f81f87ba825b4a80e3ddd3653270e98ffa2bf9204a6` |
| Linux construction executable | 58,490,251 | `bd83105fa731fbde75b3bd1e788ea3b25bdefd159d8eeed48f91dabf7f2d426b` |

The native constructor matched the source model byte for byte on 24 inputs.
Nine produced tapes that ran on Alpha. The other 15 returned the same refusal
coordinates. The renamed-binding, zero-omission and initializer-order fixtures
produced 1,205, 1,469 and 1,794 bytes respectively and each exited 42.
Each fixture is the only unit in package `app`, with entry `main`. The
published Epsilon interpreter, native constructor and source model also agreed
on the 1,271-byte tape for the nested record/case omission control (SHA-256
`294b4d9893a70a5f9f55fcebcc714cf07808b25a67e08b9e78ecefeaf8b8a0c3`).
The portable program and wire decoder reproduced their previous tapes.
[Fixtures](../conformance/omega) · [Wire decoder](#product-wire-package-through-the-construction-entry)

The same native executable ran on the complete product for 243.9 seconds
before the 10 GiB RSS guard stopped it. Its output was empty.

The independent Epsilon model ran the updated guest on the original
3,955,617-byte product. It passed the renamed binding at byte 87,497 and
returned `00020000000fc60100` at byte 116,239, on `in` in
`[u64 in Trapping; 131072]`. This is line 48 of `scalar.omg`.
[Source location and request identity](../../../bootstrap/omega/compiler/README.md#renamed-payload-bindings-on-the-complete-compiler-product)

## Earlier conformance and private-adapter measurements

At `9d6b86e82d0` the packet contained 852,096 bytes of Epsilon source and
3,955,617 bytes of product input covering seven packages and 304 source units.
The 52-byte EEREQ header brought the total to 4,807,765 bytes, SHA-256
`dbcf638d9ec5e1b6a968129b031aead6aad3cfc9a08291cfb48aaa3c43d48b59`.

The published native evaluator exited 253 with empty stdout and stderr in
5.611 seconds. The Alpha-hosted evaluator returned the same status and empty
output in 10.27 seconds. The generated Gamma entry returns that status before invoking the Epsilon evaluator when
stdin exceeds 4,194,304 bytes. The private input check allows 8,388,608 bytes.
The 4 MiB output check and the enclosing Gamma runtime’s 131 MiB sealed-input
limit both retain their original values.
[Generated entry](../../../bootstrap/delta/compiler/support/adapter.gamma)
and [Gamma runtime](../../../bootstrap/gamma/compiler/compiler.gamma)

To reproduce the private adapter used for these measurements, add
`--receipt /path/to/profile-1/evaluator.gamma` when preparing a new directory.
The script verifies that receipt and writes `epsilon-8m.gamma`.

The measured private build also regenerated the receipt through Delta. It used
the widened adapter and replaced its rolling checksum in the Delta producer's
support-member table. The resulting receipt matched the script's
`epsilon-8m.gamma` byte for byte. The 25,223,480-byte native executable differs from the published evaluator at just offset 25,214,183
(zero-based), where the bound immediate changes from `40` to `80` in hex.
Each boundary test padded a zero-length guest and stdin with trailing bytes. Once the adapter admits
that input, the evaluator reports the trailing data in a 40-byte diagnostic.

| Input provision | Input at the limit | Input one byte above the limit |
| --- | --- | --- |
| Published 4 MiB | 40-byte diagnostic, process 0 | Process 253, empty stdout |
| Private 8 MiB | 40-byte diagnostic, process 0 | Process 253, empty stdout |

The private evaluator also returned the 40-byte diagnostic at 4 MiB + 1 byte.

On the exact full request the 8 MiB evaluator was still running after 480
seconds. The wall-clock watchdog terminated it at 480.739 seconds, with
2,656,232 KiB peak RSS and empty stdout and stderr. The 12 GiB resident
limit and 4 GiB cgroup reserve did not trigger. The run produced no compiler
outcome or Alpha tape.

The entry-unit control retained all seven packages and seven edges, with the
one unchanged 14,509-byte `source/application/construction/main.omg` unit.
The 867,062-byte EEREQ had SHA-256
`b75989e139d6a7cd6b4f43b30a9ef2f0b67c4622fc2a33cca8941ca48f0424e0`.
The same wall-clock watchdog stopped it at 480.299 seconds, with 3,141,308 KiB
peak RSS, empty stdout and empty stderr. A source coordinate remains unknown
for both the full request and this reduced control.

Changing only the package count from seven to 65 preserves the full input
length. The private evaluator ran that control in 59.278 seconds, returning
process status zero and `0003000000`. The guest constructor had read its
header and returned exit 3 for malformed framing.

| Artifact | SHA-256 |
| --- | --- |
| Canonical evaluator Gamma receipt | `1d3a700d970acda04ee4699c19bf03a3654eb481a46755399d0daf759fe030af` |
| Private evaluator Gamma receipt | `e3854c5f93a5368980f5cfdc86aa64516c5de12a7355247a0fa72acf11f0bcc7` |
| Native Gamma compiler | `32c681f83f4ceafb775de7ce5e24a4fa00df8417daaf0f50ca5905ae9f760394` |
| Published native Epsilon evaluator | `80226b25de03c6608638a1c6a35f4d0fcbc23a3dd1445aa474fb9a34194907f8` |
| Private native Epsilon evaluator | `36c91efef706b00107a5f2127d5c6a698ffd850f9826f0b0dd18e7d11e0ed7b8` |

## Product wire package through the construction entry

The ordinary constructor compiled the 323-byte product wire decoder as a separate
package. Its caller imports `wire::u32_from_le_bytes` and checks zero, mixed
bytes and all bits set. The two source units and the dependency edge go through
the same row encoder as the full compiler product.
[Wire decoder](../../../source/foundation/wire/codec.omg) and [caller checks](../conformance/omega/wire_word.omg)

```sh
python3 tests/bootstrap/independent/product_wire.py /tmp/omega-product-wire --evaluator "$work/evaluator"
```

Pass the compiled construction executable to replay the same three raw packets.
Both routes save the framed results and the linked application's Alpha tape.

```sh
python3 tests/bootstrap/independent/product_wire.py /tmp/omega-native-wire --constructor "$work/omega"
cmp /tmp/omega-product-wire/wire.tape /tmp/omega-native-wire/wire.tape
```

Omitting both executable options writes `linked.stdin`, `missing-edge.stdin`
and `trailing-byte.stdin`.

The 56,933,914-byte native bootstrap compiler produced the same 1,607-byte
tape and both exact refusal records as the interpreted route. The linked
tape exited 42 on the audited Alpha seed. The native executable's SHA-256 is
`47c1b9cddde2190a5d2e600640d07c1a86597123077dc9a6a86abd7834b14c3b`.

At `e45acdff0a9` the stock Linux-native Epsilon evaluator produced 1,607 bytes
of Alpha tape. The Alpha seed ran it to exit 42 with empty output. The tape's
SHA-256 was `f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`.
Removing the dependency from the same two source units returned Exit 1 and
coordinate 11, the start of the caller's import. An extra byte after the
linked input returned Exit 3 with empty guest output.

## Source staging for the entry-unit control

Delete the final byte of `entry-unit.eereq` and subtract one from the
four-byte little-endian stdin length at packet offset 16. This preserves
all seven packages, seven dependencies, the unit row and the declared
14,678-byte arena. The stock evaluator reached the missing byte after
staging 14,677 bytes and returned the constructor's framing failure
(`0003000000`) in 95.4 seconds, with 552,376 KiB peak RSS. The exact packet
SHA-256 was `581349db3ec656406f4e85fab26a2fc21e38091aa0a36c9df5ea33e6c7b70436`.

The independent Epsilon model from `8092cfd9c0a5` ran the complete, unchanged
entry-unit packet and returned `0001000000a9000000`. This is Exit 1 at packed
byte 169, the first `use` in the unit. Only the construction package supplies
a source unit in this control. The earlier 8 MiB evaluator run on that packet
stopped at its 480-second deadline.

Two further controls preserved every package, source range and packet length.
They changed a single byte of the entry source in guest stdin and ran the
stock Linux-native Epsilon evaluator with the same compiler and constructor.

| Entry-source change | Stock evaluator observation |
| --- | --- |
| First byte replaced with `0xff` | Exit 1 at packed byte 169 in 121.7 s wall, 63.9 s CPU, 565,244 KiB peak RSS. |
| Final newline replaced with NUL | Empty output after 257.3 s wall and 190.6 s CPU. The supervisor stopped the process at 2,081,020 KiB peak RSS to retain 2 GiB of shared memory headroom. |

The parser validates UTF-8 and tokenizes the entire unit before parsing its
first declaration. The last-byte NUL therefore measures work before syntax
parsing. Its complete runtime and refusal coordinate remain unmeasured.
[Parser entry](../../../bootstrap/omega/compiler/parser.epsilon)

## First unsupported construct in the full product under the source model

The independent Epsilon model from `8092cfd9c0a5` ran the unchanged guest
and stdin extracted from `whole.eereq` (SHA-256 `dbcf638d9ec5e1b6a968129b031aead6aad3cfc9a08291cfb48aaa3c43d48b59`).
The model receives the guest and stdin separately. The original conformance
adapter limited its combined packet to 4 MiB. The 304-unit product returned `0002000000c9550100`, Exit 2 at packed
byte 87,497. This is byte 45,324 of unit 4, line 673 column 37 of
`expressions.omg`, at the `as` in `Selection::Machine { handle as callee }`.
Omega permits this spelling to rename a payload binding. The comparison took
164.7 seconds wall and 146.9 seconds CPU with 1,004,548 KiB peak RSS.
[Product expression](../../../source/subsystems/compiler/source_to_psi/pipeline/checked_trees_to_scalar_operations/expressions.omg)
and [payload bindings](../../../wiki/spec/language/patterns.md)
