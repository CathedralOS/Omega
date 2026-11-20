# Compiled VM experiment

This peer removes the Gamma interpretation layer after constructing a Gamma
compiler from source. [`../../../bootstrap/gamma/compiler/compiler.gamma`](../../../bootstrap/gamma/compiler/compiler.gamma) and
[`../../../bootstrap/gamma/compiler/frontend.gamma`](../../../bootstrap/gamma/compiler/frontend.gamma), with the
[`Alpha backend`](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma), compile Gamma to independent Alpha tapes.
Their first customer is the existing Gamma-written Delta compiler, which
constructs the Delta-written Epsilon evaluator. Its Gamma receipt is compiled
to another independent tape and runs the Epsilon-written bootstrap compiler.
No variant is selected by this experiment.

The source construction starts from the checked-in native
[Alpha seed](../../../bootstrap/alpha/README.md), the separately admitted
[Beta compiler tape](../../../bootstrap/beta/AUDIT.md), and the host OS. It stamps that tape
into Alpha, reconstructs Beta from its source, and uses the reconstructed Beta
compiler to construct the Gamma evaluator from Beta source. Both tapes must
match their bound artifacts exactly. The constructed Gamma evaluator then runs
the authored Gamma compiler on its own source to construct `compiler0`.
`compiler0` and its successor each rebuild that source; all three resulting
tapes must be identical. Beta self-reconstruction is evidence about the admitted
seed; it does not derive the initial Beta tape from nothing.

## Run

From the repository root on Linux x86-64, construct the Alpha-tape Gamma compiler in a new output directory:

```sh
tests/bootstrap/linux_x86_64/bootstrap --alpha . /tmp/omega-gamma-alpha
/tmp/omega-gamma-alpha/gamma-compiler < source.gamma > program.tape
```

The native driver feeds the seed and each reconstructed compiler directly. [Linux construction and inputs](../../../tests/bootstrap/linux_x86_64/README.md)

### Run the public Omega lexer on Alpha

The public Omega lexer now fits in the audited Linux x86-64 Alpha seed. Two
compiler changes can be measured independently on the same 58,733-byte
Epsilon program. The Epsilon compiler shares the code that propagates exits,
traps and internal failures. The Gamma Alpha backend shares the code that
pushes and pops a value on its evaluation stack.

| Epsilon emission | Gamma source bytes | Original Gamma backend | Compact Gamma backend |
| --- | ---: | ---: | ---: |
| Original | 2,827,892 | 32,158,739 bytes of calculated layout; refused | 15,449,892-byte tape |
| Shared outcome handling | 1,594,506 | 15,120,299-byte tape | **7,342,855-byte tape** |

Each published tape ran the original 148-byte batch and returned the expected
465 bytes for all 23 cases. The observation includes Epsilon exit 42 and agrees
with both the evaluator and the native executable. The smallest tape leaves
9,434,357 bytes in the seed. [Lexer cases and expected bytes](../../../tests/bootstrap/independent/epsilon/lexer_scan_test.py)

A separate cold run copied only `bootstrap/` into a new source root and ran the
external Linux launcher with an empty `PATH`. From the admitted seed it built
the native Gamma, Delta and Epsilon compilers. A byte packer read the Epsilon
compiler's own source and runtime lists. The compiled Epsilon program used
its standard entry. The cold Gamma compiler also built the Alpha-targeting
compiler that emitted the 7,342,855-byte tape. Stamping that tape into the seed
reproduced the 465-byte observation. [Source lists and commands](../../../bootstrap/epsilon/compiler/README.md)

The old encoding repeated a 69-byte stack push at 209,820 sites and a
32-byte pop at 179,121 sites. The Alpha backend now calls three leaf routines
with nine-byte Alpha calls. One pushes the current word and its tag. The other
two pop into the left or right operand register pair. The push still checks the
frame bound before writing. Each routine returns before a source call, a tail
jump or the next stack operation, so the maximum remains one temporary return
address in addition to the active Gamma calls. Native code still inlines these operations.
[Alpha assembler](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma)

| Public lexer artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Original Gamma input to either backend | 2,827,892 | `fa5dedc6e8b95c533ae9208d0c9fcc7d07fc98b20c29c199ccd4f6efe2c4e0b7` |
| Compact backend, original Epsilon emission | 15,449,892 | `983501625a9d9fe4458d0386ff0808ecfca172c0831e0ed9dbb65d963f1f4ddf` |
| Gamma input with shared outcome handling | 1,594,506 | `267f3b49246d8593e9fc467bee36bc3b82715dedb3e3e397e7757e02234cd8d3` |
| Original backend, shared Epsilon outcomes | 15,120,299 | `c0cf4b55c846f46910f32ed5061416e68b66916e4e500acc09c13496721c50d6` |
| Compact backend, shared Epsilon outcomes | 7,342,855 | `f445ec71096c3be69d78a00aba2402130f196998248e977466865fcc924e5e30` |
| Lexer output | 465 | `7d7a561d217ec1ce599a203fb8290b907c2582883c98dc71541c5bd0633f31aa` |

Starting from the admitted Beta tape, the launcher constructed the 227,471-byte
Gamma compiler tape and reproduced it twice (SHA-256
`76d58ec432aa1f0d2e1c4471f5a99a7240ea900af672722bcde317f4e32cfca2`).
It passed the 47 portable Gamma checks. At 131,072 active source calls the
test published both expected bytes. One more call returned 250 with empty
output. The 17-case string scanner also ran on Alpha in 4,025,521 tape bytes.

To rebuild this customer from its Epsilon source and execute the lexer
checks, run `python3 tests/bootstrap/independent/successor_test.py --compile-epsilon`.
The route writes the public Gamma receipt, Alpha tape and observations under
`build/independent-successors/compiled-epsilon/`.

### Running the whole Omega constructor on one tape

The cold launcher used the original Gamma evaluator to compile the
43,315-byte Alpha-targeting compiler source, SHA-256
`afee5b7c75679386d30e483d4a627aeeca617f9c3de2dd637adc0474277cc007`.
It produced a 207,681-byte compiler tape. Running that tape on its own
source and repeating with its successor gave three byte-identical tapes,
SHA-256 `c3274d349b53511e4a8bcd8624927d1d2aee3052f1212ed113234255f650f7e1`.

The Gamma-to-Alpha compiler assembled the 25,733,081-byte Gamma constructor
from `bcf71f0bbfc` into one 11,976,860-byte tape, SHA-256
`172bd421b5c2cf334a7a5688c843e1e9dc24cefe11d7bb2b25a9d514ad17d491`.
A seed-built Linux executable of the Gamma-to-Alpha compiler emitted this
tape. The Alpha seed then ran the resulting constructor and both product
tapes below. The uncompressed instructions occupy 57,004,893 bytes. The stamped program
starts with a 340-byte Alpha decoder and a 3,840-byte dictionary. The seed
runs the decoder itself. Programs that fit the seed directly keep their
plain Alpha instructions. The Alpha backend grew from 3,681 to 11,814 bytes.
Across the entire Gamma-to-Alpha compiler, authored source grew from 35,082
to 43,315 bytes. Its own Alpha tape grew from 176,772 to 207,681 bytes.
Both the old and new compilers emitted the same tape for the old compiler
source at `980fa9c6a6c` (SHA-256
`940396e8eb92b1599996a5b1d9ad1df3bf04b5d6e9ceae10a984de0472d4f4c8`). Using the original Gamma interpreter to
run the final constructor avoids this packing algorithm.

The seed ran the packed constructor on a 111-byte Omega package. It
produced the expected 22-byte Alpha tape, SHA-256
`25c789ed6ce473847a1dc82854aad7b59d3ff336d4ed73a64a56c540b5d78384`,
which exited 42 on the seed. On the 828-byte two-package wire decoder the
same constructor produced a 1,607-byte tape, SHA-256
`f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`.
That tape exited 42 with empty stdout and stderr. Removing the package edge
returned guest exit 1 at byte 11. Appending a byte returned framing refusal 3. [Same package on original Gamma](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#running-the-compiled-omega-constructor-as-gamma-on-the-seed)

For a large program the compiler takes up to 240 instructions seen twice
among the first 4,096. Each becomes one byte in the encoded stream. Tokens
1 through 11 introduce that many literal instruction bytes, and tokens
16 through 255 select a 16-byte dictionary record. The first byte of each
record is the instruction's original length. Only the compiler builds
the dictionary. The seed runs ordinary load, store and branch instructions
to copy the indicated bytes into `[0x28000000, 0x30000000)`. The output
buffer ends at the first address and the pair heap starts at the second.
The decoder checks the stream end, dictionary indexes, copy lengths and
destination end before restoring its registers and jumping to the program.

The compiler resolves each branch and call for this interval while preserving
literal integers, including integers equal to an old branch address. An
independent expansion of the measured tape matched the original instructions,
with exactly the placement offset added to all 3,484,036 branch and call
addresses. The complete encoded tape must fit the original stamp even when
the expanded instructions fit the separate code interval. Source validation
still precedes publication, including functions that are never called.
[Alpha encoder and decoder](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma) ·
[Seed instructions](../../../bootstrap/alpha/SEMANTICS.md#5-transition-rules)

To build and run the constructor through the normal Alpha launcher, use the
`omega.gamma` produced by the whole-constructor construction and an empty
output directory. The genuine standalone wire customer checks the resulting
tape and its missing-edge and framing controls.

```sh
tests/bootstrap/linux_x86_64/bootstrap --alpha . /tmp/omega-packed-alpha /path/to/omega.gamma
python3 tests/bootstrap/independent/product_wire.py /tmp/omega-packed-wire --constructor /tmp/omega-packed-alpha/program
```

To exercise the packed entry without assembling the Omega constructor, run
`python3 tests/bootstrap/compiled_vm/source_test.py --compiler-only --large-alpha`.
The control compiles 700 unused arithmetic definitions and a forward call.
Its 21,467,587 ordinary instruction bytes produced a 5,030,606-byte tape.
The stamped program matched byte output, a guest refusal and an arithmetic
trap. The compiler rejected an invalid call in the last declaration. [Source](../../../tests/bootstrap/compiled_vm/source_test.py) ·
[Whole-constructor input](../../../tests/bootstrap/independent/WHOLE_REQUEST.md)

#### Comparing the same constructor on Alpha and Linux

At `980fa9c6a6c` the Epsilon-written Omega compiler handled typed
forward-call parameters and qualified sum cases in transition result arms. Packing its
manifest and appending the normal construction entry reproduced its complete
934,574-byte source. Seed-built Linux-hosted tools produced the Gamma input.
The Alpha and Linux backends then assembled that input for their respective
runtimes.
[Compiler manifest](https://github.com/CathedralOS/Omega/blob/980fa9c6a6c/bootstrap/omega/compiler/omega_compiler.epsilon.sources) ·
[Construction entry](../../../bootstrap/omega/construct_omega.epsilon)

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Epsilon constructor source | 934,574 | `9384d52f1fafbb94c2c4229e3ddc4ea4d5d6ede68f30f0615c6ea1ede4490611` |
| Generated Gamma source for both targets | 26,574,759 | `5452dfddad14b49a8451a0e080cb237723d5268f1a78394b58d164628ee173f1` |
| Packed Alpha tape | 12,359,478 | `554b7ee19a07cfe50a6d8eed1cfc56911768553b1a19c7c2c6080047d63f88c3` |
| Linux x86-64 executable | 56,615,863 | `2fe8ba0c5e1fa254c2c83e673c3f7495f4e673cfc5fc4fe8cde52593c981bdfe` |

The independent tape reader expanded 58,805,184 instruction bytes and
rebased 3,596,175 jump and call targets to a code base of zero, giving SHA-256
`a6c236eb3db6002e3622b4b987f32cdf808bb6c525338882c098e857fcf77577`.
The Alpha seed ran the packed constructor on the same 111-byte package as
the Linux executable. Their 27 output bytes matched, including the 22-byte
Alpha tape `25c789ed6ce473847a1dc82854aad7b59d3ff336d4ed73a64a56c540b5d78384`.
The resulting tape exited 42 on the seed with empty stdout and stderr. The
Alpha constructor returned the exact five-byte guest exit 3 for the
trailing-byte wire package.

#### Checking the expanded instructions

The external reader walks the tape, checks instruction widths against the
Alpha instruction set and subtracts the code base only from absolute branch
and call targets. The seed runs above exercise the shipped decoder. On the
11,976,860-byte tape the reader recovers 57,004,893
ordinary bytes, including 3,484,036 targets, with SHA-256
`bc0af6bd305ffc11ab70a416de08811e849a2ddefdb54525472e425ec9fdf66a`.

```sh
python3 tests/bootstrap/compiled_vm/packed_tape.py /tmp/omega-packed-alpha/program.tape
```

To compare every instruction against a separate straight assembly, make a
temporary copy of the Alpha backend that publishes the ordinary instruction
list at address zero. Its oversized output serves as a measurement file.
Use the same Gamma customer for both assemblies.

```sh
audit_dir=$(mktemp -d)
PYTHONPATH=tests/bootstrap python3 - <<'PYCODE' > "$audit_dir/measure.gamma"
import sys
from compiled import gamma_backend_source
signature = b"(def i_publish ((fields Int) (size Int) (labels Int)) Int"
source, _ = gamma_backend_source("alpha").rsplit(signature, 1)
sys.stdout.buffer.write(source + signature + b"\n  (i_output fields size labels size))\n")
PYCODE
tests/bootstrap/linux_x86_64/bootstrap . "$audit_dir/native" "$audit_dir/measure.gamma" /path/to/omega.gamma
python3 tests/bootstrap/compiled_vm/packed_tape.py /tmp/omega-packed-alpha/program.tape "$audit_dir/native/producer.output"
```

<a id="can-the-whole-omega-constructor-fit-in-alpha"></a>

### Earlier single-tape size limits

The 27,177,694-byte Gamma constructor (SHA-256
`b2160c526041d7d695bbe133f0ef4cb9d3b2f403a5cf901e0b9cd625e5f9454f`)
exceeds the audited seed’s **16,777,212-byte tape limit** with the Alpha
backend at `43e334bc2b0`. Its 245,493 `pair` expressions alone need
**35,350,992 bytes**. Each expression emits 117 bytes to check the heap,
store both tagged operands and return a tagged pair, plus three nine-byte
stack-helper calls. The Linux-hosted Alpha compiler produced 750 bytes for
`(def main () Int 0)` and 923 for `(def main () Int (pair 0 0))`. The
173-byte difference includes one extra 29-byte literal. [Alpha backend](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma)

To reproduce the lower bound on that exact Gamma file, count the expression
heads after removing comments. This artifact contains no quoted character
literals after comment removal.

```sh
python3 - "$OMEGA_GAMMA" <<'PYCODE'
import pathlib, re, sys
source = re.sub(rb";[^\n]*", b"", pathlib.Path(sys.argv[1]).read_bytes())
assert b"'" not in source
pairs = len(re.findall(rb"\(\s*pair(?=[\s)])", source))
print(pairs, pairs * 144)
PYCODE
```

Counting all emitted instruction widths across 7,768 definitions gives a
**calculated layout of 125,445,123 bytes**. The same accounting matched
published customer, Delta and Epsilon tapes of 16,422, 638,297 and
3,798,810 bytes. The production backend checks the limit before publication.
On the audited Linux seed, a padded tape of exactly 16,777,212 bytes exited
42. Changing only its length word to 16,777,213 triggered `SIGILL` before
its first instruction, with empty output. [Seed loader](../../../bootstrap/alpha/alpha_x64_linux.s)

The Linux-native compiler built from that same Gamma source accepted an
828-byte linked Omega request. Its 1,607-byte Alpha product
(`f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`)
ran on the audited seed and exited 42 with empty output.

### Sharing Alpha pair allocation

At #4577 the Alpha backend emitted the 117-byte allocation body once,
followed by a one-byte return. Each `pair` site called the body for nine
bytes. It popped the right operand, popped the left operand and pushed the
result at the call site, so the allocation and stack helpers each used one
return address. A tape with `N` pair expressions changed by `118 - 108 × N` bytes.

| Gamma program | Pair sites | Old tape bytes | New tape bytes |
| --- | ---: | ---: | ---: |
| Delta compiler | 260 | 638,297 | 610,335 |
| Epsilon evaluator | 2,492 | 3,798,810 | 3,529,792 |
| Public Omega lexer (original Gamma input above) | 27,626 | 15,449,892 | 12,466,402 |

The smaller public lexer tape (SHA-256
`37a9e1d5cfa497496f6fbfb70437ef93abd1ffd43d4063b74f14a42c87fab92f`)
ran all 23 cases on the audited seed and reproduced the same 465 bytes.

For the frozen whole constructor above, the calculated layout falls from
125,445,123 to **98,931,997 bytes**. A lower bound needs just two expression
counts from the same source. Its 245,493 `pair` sites cost 36 bytes each
and its 221,110 `second` sites cost 63 bytes each. Together they require
22,767,678 bytes before their operands, already above the seed limit.

Three cold Gamma compiler generations produced the same 208,153-byte
Alpha tape (SHA-256
`4bf3d6225a5bb8639ae773ed53ac022762915ddccdcd94a9ecb25dd15ef4673d`).
The Linux-hosted emitter reproduced those bytes. The resulting compiler
passed 34 Gamma observations and rejected 16 invalid programs. Pair
allocation at the last admitted call frame succeeded. One additional
frame returned 250 and discarded the buffered output. The Epsilon
evaluator matched seven wire observations, including stdin and guest traps.

### Pair projections and frame slots on Alpha

On the linked public Omega lexer from #4558, sharing all three pair
operations and the frame-slot loads and stores reduced the tape again.

| Alpha backend on the same 1,575,418-byte Gamma source | Published tape bytes |
| --- | ---: |
| Shared pair allocation (#4577) | 5,970,717 |
| Single call for `pair`, `first` and `second` | 4,650,419 |
| Also share tagged frame-slot loads and stores | **3,718,385** |

Each tape ran all 23 lexer cases and returned the same 465 bytes on the
audited seed. The final tape's SHA-256 is
`536bf79af57a78bb2cf5872a987e5d08a50a0f1694ac3085d74136681b811ea6`.
The pair helpers pop their operands, allocate or check the pair, then jump
to the existing stack push. Its return goes straight to the caller.
The frame-slot helpers take a full byte offset in register 10. They read
or write both the value and its tag. Emitting the shared routines grows
the scalar-only `(def main () Int 0)` tape from 868 to 1,129 bytes. [Alpha assembler](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma)

For the frozen 27,177,694-byte constructor above, the calculated layout
falls from 98,931,997 to 74,702,984 bytes with the pair changes, then to
**60,276,870 bytes** with the frame-slot changes. Sharing more helpers can
reduce that further. Even if every literal, pair operation and local slot
read or binding used just one nine-byte Alpha call, those 1,904,960 sites
alone would occupy 17,144,640 bytes. Fitting the 16,777,212-byte seed
would require some of those source operations to take fewer bytes or to
share a single call across multiple operations.

Starting from Alpha and Beta with an empty PATH produced three identical
178,112-byte Gamma compiler tapes (SHA-256
`4f56a6e5b1004595c91dc89120c3d5f266d679168e0ed169ee42b4f46252e1a2`).
A Linux-hosted compiler reproduced the same bytes. The new compiler ran
both projections at the last admitted Gamma frame and preserved tagged
local reads after a tail call with 258 arguments.

The Alpha-hosted Epsilon evaluator also ran the 58,917-byte Epsilon-written
Omega lexer from the #4558 measurement directly. Rebuilding each evaluator
tape and interpreting the 148-byte batch were timed separately. Both used
the same 726,582-byte evaluator Gamma source and the same fixed seed.

| Alpha backend | Evaluator tape bytes | Emit CPU seconds | Emit peak KiB | Run lexer CPU seconds | Run lexer peak KiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shared allocation (#4577) | 3,529,792 | 10.63 | 236,540 | 31.91 | 52,796 |
| Shared pair operations and frame slots | 2,267,080 | 8.70 | 192,528 | 17.41 | 52,028 |

The same evaluator also interpreted the live 58,127-byte lexer from
`0a0b233a152` in 21.05 CPU seconds (52,220 KiB peak RSS). All 23 cases
matched, including distinct refusal coordinates for malformed escapes.
The 465-byte result had SHA-256
`7d7a561d217ec1ce599a203fb8290b907c2582883c98dc71541c5bd0633f31aa`.
[Lexer source and cases](../../../tests/bootstrap/independent/epsilon/lexer_scan_test.py)

### Compile the product wire decoder through Epsilon on Alpha

The Alpha-hosted Epsilon evaluator interpreted the complete 873,205-byte
Omega construction program and compiled a two-package Omega application.
Its 323-byte `wire/codec.omg` decoded zero, mixed bytes and all bits set.
The caller's 1,607-byte Alpha tape ran on the audited seed and exited 42
with empty output. Removing the dependency edge preserved both source units
and returned Exit 1 at byte 11, the caller’s import. [Product codec](../../../source/foundation/wire/codec.omg) · [Caller](../../../tests/bootstrap/conformance/omega/wire_word.omg)

The constructor and Epsilon source list came from `d3a79047ce09` (#4542).
The retained constructor SHA-256 was
`8bdbb90999c6b62a04754a3d452966b4903af74179fbc113fdc910a628d27401`.
Repacking that revision reproduced both EEREQs byte for byte, including
the evaluator closure digest
`7b214d058c086ffcf5e5481e5dd08e4e6f47275ae07a4cb2cb6b4d26764b1bae`.
At `d00a570048e` (#4589), emitting the 2,267,080-byte evaluator tape
from its Gamma source took 8.70 CPU seconds and 192,528 KiB peak RSS.
The following measurements start at evaluator invocation.

| Omega input | EEREQ bytes | EEREQ SHA-256 | CPU seconds | Peak KiB |
| --- | ---: | --- | ---: | ---: |
| Linked, 828 bytes | 874,085 | `83cdc63dfcecb7470adf783c2d4ddf6113ef0e17264d1b3f283626177de16fd8` | 866.64 | 1,576,788 |
| Missing edge, 808 bytes | 874,065 | `c6bf88ab9e069e7275e5da6f21b075805ccf9183dfeb9a7c9f4b75ad13e6507a` | 705.31 | 1,195,000 |

The linked tape had SHA-256
`f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`.
The missing-edge observation was `00010000000b000000`. A separate replay
through the frozen Linux-native constructor, SHA-256
`ccc6eb867203f3250e9025e8f616d07e8b2253582b34499201ffe9e4ce4cb21b`,
produced the same two observations byte for byte. [Wire test and earlier Linux results](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#product-wire-package-through-the-construction-entry)

### Compare a production request operation across evaluator hosts

The [Request.clear test](../../../tests/bootstrap/independent/request_clear.py)
takes the production `Request` declaration and `clear` machine byte for byte
from [the request package](../../../source/subsystems/compiler/request/request.omg).
Its caller sets both metadata words and the last byte of the 64 KiB request
buffer. After calling `clear`, it checks both words are zero and uses the
preserved byte as the exit code.

Build the Linux and Alpha Epsilon evaluators from the same Delta request,
then pass both executables to the test. [Linux construction](../../../tests/bootstrap/linux_x86_64/README.md#run-the-compiled-alpha-tape-route)

```sh
python3 tests/bootstrap/independent/request_clear.py /tmp/omega-epsilon/successor \
    --compare-with /tmp/omega-epsilon-alpha/successor --timeout 1200
```

The test sends the same sealed request to both evaluators, prints executable
and observation identities, compares every output byte, then runs the common
Alpha tape and requires exit 42 with empty output. The timeout applies to
each evaluator separately.

At `e488ba404bf` on Linux x86-64, the checked-in Linux construction driver
built the Alpha-hosted evaluator from the seed and admitted Beta tape in
192.870 seconds. All three Gamma compiler tapes matched. The constructed
Delta compiler emitted the same 726,601-byte evaluator Gamma source as the
native route. Its SHA-256 was
`1d3a700d970acda04ee4699c19bf03a3654eb481a46755399d0daf759fe030af`.
The Alpha backend compiled it into a 7,844,855-byte evaluator tape.

| Epsilon evaluator | Executable SHA-256 | Elapsed for the same request |
| --- | --- | ---: |
| Freshly seed-built Alpha host | `fefb42b13bf9ba1a3eb87e7d810f9441d2f73ed0119544438eb910db33c7bace` | 664.705 s |
| Reused native Linux host | `80226b25de03c6608638a1c6a35f4d0fcbc23a3dd1445aa474fb9a34194907f8` | 118.177 s |

Both evaluators received the same 842,218-byte request, SHA-256
`3f534a4a2c566fae357899a4fbb15994314bac581e278a5d1b5b717779a946a8`.
Each published the same 1,355-byte observation, SHA-256
`e267db6d2e9b3a39e3cb1207452ce08d74345037892aa723ba162c160c021b45`.
The 1,350-byte program in that observation ran on Alpha with exit 42 and empty
output. The elapsed times cover one invocation per evaluator on a shared
machine. The customer exercises `clear` and its caller. Request staging and
decoding have separate tests.

The bootstrap compiler with checked word arithmetic occupies 845,978 bytes.
Repeating this comparison with that compiler changed the request to 848,897
bytes (SHA-256 `8f344dc65c26b3bbc388acb53ed32085ec942160f8fc3809f0ec03f16bcfc928`).
Both evaluators again published exactly the 1,355-byte observation above.
The resulting 1,350-byte program exited 42 on Alpha.

The [whole construction input](../../../tests/bootstrap/independent/WHOLE_REQUEST.md)
exercises all product sources and the evaluator’s sealed-input provision.

### Stage and read a byte with the native Omega constructor

For the complete request storage unit, pass a compiled construction executable.
The test frames `main.omg` and the unchanged `request.omg` as two source files
in one package.

```sh
python3 tests/bootstrap/independent/request_clear.py --staging-constructor /path/to/constructor --timeout 1800
```

The caller stages 42 and reads byte zero. Changing only the read offset to one
produces an Alpha Trap (Linux SIGILL). Changing only the staged byte to `true`
produces a compile-time rejection at that token. On the 2,082-byte product
unit, the two executable cases each produced 2,071 bytes of Alpha tape and the
rejection pointed at arena byte 143. The test recomputes the token coordinate
from the current caller and construction frame. A fourth caller starts at full
capacity. Staging one more byte must advance `observed` to 65,537, keep
`length` at 65,536 and preserve the last stored byte.

The Linux x86-64 constructor built from Omega compiler revision
`807099e8790` compiled the complete 2,082-byte request unit (SHA-256
`bdc293370165c8cc6b92de1e17e77d261956f2a70bc6683e276dc47f15dde4c1`).
The 883,526-byte Epsilon source includes the public construction entry. Its
SHA-256 was `6fcc6d17e45ab53205f964f16cbfb0d21591812339fa411ff773c46b6ed13031`.
The Epsilon compiler and storage runtime from the head of PR #4584
(`d7b67e70954`), the Delta construction profile and the Gamma Linux backend
produced the 55,348,424-byte constructor (SHA-256 `381c4b2bc372ac0f0e23ad001a3e89f541cb6f80cc434934ab1f8912105ac175`).
[Build a native constructor](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#build-from-the-seed)

For the `stage-first` caller, the constructor read a 2,382-byte packet
(SHA-256 `b355eff899789463165edc9a2188016808eb1aa3d27b41d19793a140ec1a2cfb`).
It published 2,076 bytes (SHA-256
`42ce29ec6fd6873db56be0da0599f1b7d26e563e529f58d4e8ba34cce5e2a2ec`).
The 2,071-byte tape following the five-byte success header had SHA-256
`955589ac5af9b1a49df301d6f0953e6cb03b20e6208476a0de2815b82e214c27`.
The checked-in Alpha seed ran it and exited 42 with empty output.

Changing the staged argument from `42` to `true` gave the constructor a
2,384-byte packet (SHA-256
`a03975bc9eed13c327c855ec3622816d1b198de029fb0b97c66142215ce40717`).
It returned exactly `00010000008f000000`, a rejection at byte 143, the first
byte of `true`. The independent Epsilon model ran the same compiler source
and matched both complete observations byte for byte.

| Native constructor input | CPU seconds | Peak RSS (KiB) |
| --- | ---: | ---: |
| Stage and read 42 | 96.75 | 2,323,252 |
| Stage a Boolean | 91.88 | 1,908,136 |

## Host checks

On an executable Alpha seed host, Python and shell drive the differential tests. The [Python Gamma compiler](../../../tests/bootstrap/compiled_vm/gamma.py) supplies an independent implementation for comparison and for the explicit `--accelerated` test mode.

```sh
# Seed construction, compiler fixed point, Delta/Epsilon, and an Omega program.
python3 tests/bootstrap/compiled_vm/source_test.py --omega

# Recheck construction/rebuilds/controls without repeating larger customers.
python3 tests/bootstrap/compiled_vm/source_test.py --compiler-only

# Explicit Python-assisted development construction of the same compiler.
python3 tests/bootstrap/compiled_vm/source_test.py --accelerated --omega

# Native differential controls and the real Delta Bytes compiler customer.
python3 tests/bootstrap/compiled_vm/test.py

# Also construct the complete Epsilon evaluator and run its Console boundary.
python3 tests/bootstrap/compiled_vm/test.py --epsilon

# Also feed the complete bootstrap compiler its existing Omega fixture.
python3 tests/bootstrap/compiled_vm/test.py --omega --omega-timeout 1800

# Exercise the existing changed-source, rejection, and reset customer too.
python3 tests/bootstrap/compiled_vm/test.py --omega-controls --omega-timeout 1800
```

Source construction artifacts and the exact source/request bytes go to ignored
`build/source-gamma-reference/`; the accelerated source run uses
`build/source-gamma-accelerated/`. Host-compiler tests use `build/compiled-vm/`.
`source_test.py --seconds N` sets a positive watchdog for compiler construction,
self-rebuilds, source compilation and Omega execution. These invocations wait
for completion by default.
A refused source compilation publishes no bytes. Malformed source deliberately
traps with status 249; negative controls require that status and empty output.
Other runtime statuses fail the gate.

Tests use the existing closure packer, identity checks, and platform seed
stamper. The host frames byte requests and compares observations;
Gamma, Delta, Epsilon, and Omega execution happens in the generated tapes or the
explicitly identified reference evaluator. The short Beta reconstruction shell
commands wait for completion. A timeout makes no language judgment. The Omega allowance is
adjustable because checking the complete compiler is a materially larger
workload.

The constructed Gamma compiler writes raw Alpha code. Stamp the tape using
[`seed_env.sh`](../../../tools/bootstrap/alpha/seed_env.sh). The executable accepts
only the source program's sealed input, without the reference Gamma evaluator's
four-byte source-length envelope. Invalid source leaves an existing output
artifact untouched. Successful compilation replaces it atomically.

## Meaning and storage

[`../../../bootstrap/2_gamma/LANGUAGE.md`](../../../bootstrap/gamma/LANGUAGE.md) governs the source language.
The compiler validates every function and both branches before publishing code.
It supports wrapping words, checked scalar boundaries, immutable pairs,
sequential grouped bindings, mutually visible functions, proper tail calls,
sealed byte input, and atomic buffered publication. Primitive names can be
ordinary bindings; list heads retain their primitive meanings. Numeric and
character tokens are literals, not supported declaration names. The reserved
application declaration must have its documented nullary form and cannot be
called as a user function. Unsupported or malformed source fails compilation.

Every value occupies a word plus an unforgeable scalar/pair tag. Pairs occupy
four words. A function frame holds its arguments, saved caller frame pointer,
and statically assigned lexical slots; expression temporaries follow the frame.
Non-tail calls use Alpha's return stack. A tail call evaluates all arguments,
moves them over the old frame, restores the caller frame pointer, and jumps to
the callee. Its return address and call depth remain unchanged, including calls
between functions of different arities. Pair allocation is monotonic; there is
no garbage collector.

The generated runtime owns disjoint regions of Alpha's 128 GiB address space:

| Region | Half-open address range | Boundary behavior |
| --- | --- | --- |
| Code | `0 .. 0x1000000` | Tape limited to the seed's 16 MiB hole minus its length word |
| Frames and temporaries | `0x02000000 .. 0x08000000` | Check every push and frame reservation |
| Alpha return stack | `0x0feffff8 .. 0x10000000` | At most 131,072 active source calls plus one runtime helper (1,048,584 bytes), above the frame region |
| Sealed input | `0x10000000 .. 0x18300000` | Check ingestion and every source read |
| Buffered output | `0x20000000 .. 0x28000000` | Check every append; flush only after valid main result |
| Expanded code (packed tape only) | `0x28000000 .. 0x30000000` | The decoder checks each copy and verifies the final length |
| Immutable pairs | `0x30000000 .. 0x2000000000` | Check every 32-byte allocation |

Ordinary authored traps exit 2; storage limits exit 3. Application traps and
frame/heap/input/output limits use 249/250/252/253/254. Pair handles cannot be
forged by arithmetic or read from input; scalar operators reject tagged words.
The limits are this implementation's physical profile, not a claim to reproduce
the reference evaluator's resource envelope. Physical host memory remains an
execution dependency, just as it is for the reference seed.

## Evidence and remaining work

On Linux x86-64 at repository base `c930a0075d7`, the focused `--epsilon` run
passed 30 byte-exact native differential observations and 15 rejected-source
controls. These include 100,001 tail swaps, mixed-arity and pair-valued tail
calls, caller-frame preservation, scalar/tag traps, overflow division, lexical
refusals, and atomic application publication. Nine additional native controls
exercise exact and adjacent storage boundaries under a reduced compiled test
profile. CLI refusal also preserves an existing output artifact.

The 147,840-byte Delta compiler closure produced a 1,184,046-byte Alpha tape.
It compiled a real Delta Bytes duplication program into a 3,082-byte Gamma
receipt identical to the reference evaluator's result. The receipt compiled to
Alpha and duplicated empty, ASCII, and all-byte inputs correctly.

The same compiled Delta compiler reconstructed the complete canonical Epsilon
evaluator receipt: 729,060 bytes, SHA-256
`bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368`.
That is the existing bound receipt identity, not a newly substituted artifact.
Compiling it produced 7,876,070 Alpha bytes. Its Console exit, stdin, trap with
stdout prefix, rejected-source, and malformed-envelope observations passed.
Reconstruction took 11–17 seconds in contended functional runs; these are not
comparative performance benchmarks.

A separate cross-variant run used the exact 622,933-byte request from a fresh
reference reconstruction (SHA-256
`6386d0d5729a320e73b9865809751ae2962865f4ebc1b3472f2e828de9ab7f76`).
Compiled Delta produced the same 721,484-byte Epsilon execution-adapter receipt,
SHA-256 `71a016f53f63501760e3a10632d86c9561aa0e8387b794b074d98ce98a823082`.
This is actual byte correspondence, not a comparison between the two different
Epsilon entry receipts. Its contended 8.708-second execution and the reference's
354.968 seconds are not a controlled speedup measurement.

The complete 591,392-byte bootstrap compiler plus existing OCREQ entry then ran
under compiled Epsilon. The existing 180-byte Omega fixture compiled to 50 Alpha
bytes; executing that tape exited 42 with empty stdout and stderr. The first
60-second observation allowance was insufficient; a 300-second run completed in
95.329 seconds. This demonstrates an accelerated construction through a real
Omega executable. No Omega compiler self-rebuild has been demonstrated here.

The then-current `tests/bootstrap/conformance/omega/controls_b.epsilon` customer also
passed through compiled Epsilon with the complete bootstrap compiler (575,227
source bytes). It then accepted six changed modulo, shift, and bitwise programs,
rejects the out-of-range literal 256 while asserting an unfinalized empty tape,
then successfully recompiles 42 on the same compiler instance. The published
observation matched the existing expected `C` plus 22-byte Alpha tape exactly;
that tape also executed with exit 42 and no output. This eight-invocation run
completed in 159.692 seconds under a 300-second allowance. Later review found
that those anonymous operator acceptances were wrong Omega semantics; this
historical passing observation is not correctness evidence for them. The current
compiler refuses those forms, and its corrected customer passed in 139.663
seconds through source-built Epsilon with the same exact reset artifact and
exit 42. That observation used bootstrap compiler SHA-256
`f765c26d704c41552739e32fc592232edcf814f5d57ee3990e92660aacb6152f`. The default smaller-customer Omega watchdog
is now 1,800 seconds; `--omega-timeout` retains an explicit experimental override.

## Source compiler implementation and evidence

The frontend retains immutable syntax nodes and source spans for names. It
uses an explicit list stack with the same 255-list authored nesting boundary
as the reference implementation. Decimal words accumulate modulo 2^64;
comments end at either CR or LF; character literals preserve the language's
restricted escapes. All source bytes, including comments, are validated.

The backend first censuses declarations and parameter scopes. It counts lexical
slots by traversing expression positions, then compiles through an explicit
action stack. Every function and branch is visited, including unreachable code.
The lowering calls named target operations such as `i_add` and `i_jeq`. Their
encoders prepend little-endian fields. Reading them backwards gives each
label its distance from the code end, which the compiler saves in an AVL map.
After validation and the capacity check, the writer encodes jump targets and
outputs the fields in program order. Function and local name lookup compares
exact source spans in lists.

The small integer tags in `c_kind` name syntax, not source shapes:
`1=def`, `2=Int`, `3=main`, `4=$application`, `5=if`, `6=let`,
`7..11=+,-,*,/,%`, `12=eq`, `13=lt`, `14=pair`, `15=read`, `16=write`,
`17=first`, `18=second`, `19=input`. All other names remain source spans.
The Alpha assembler writes the existing Alpha opcodes. A label marks a
position between emitted fields. Label identifiers 1..13 belong to runtime
helpers, function identifiers start at 100, and subsequent labels follow them.

In the original Alpha construction, the two source files totaled 33,329 bytes
(851 lines). One joining newline brought the executed source to 33,330 bytes, SHA-256
`7b439ef26f2cb8a27bb9c0a2b4d5bfca6e7efeed0a5bdf9d798c04b65370b946`.
On Linux x86-64 its actual reference construction completed in 62.362 seconds.
It produced a 457,252-byte compiler tape, SHA-256
`71f6650f8201294e6654217f38fcd431fd8daaa233e0330c5da29e9819ad7e71`.
Both subsequent self-rebuilds were byte-identical, as was the independently
Python-produced tape. The source-built compiler passed 32 native differential
controls and 15 source refusals, including a legal binder named `let` that
caught an earlier incorrect traversal of binding records as expressions.

The downstream execution below used an initial 31,890-byte spelling with decimal
ASCII operands in keyword tests (source SHA-256
`584d3d94e903998defa96d417f65ff43a98630b55211c66a0ca01fa46287c79b`).
Its reference construction took 62.386 seconds. Rewriting those operands as
readable character literals preserved the parsed program, and the final actual
reference construction reproduced the same compiler tape exactly. Thus the
following customers consumed the same executable as the final source produces.

The source-built compiler constructed the real Delta compiler tape and the
complete Epsilon evaluator tape. Both were byte-identical to the independently
Python-produced tapes described above. The Delta compiler took 9.705 seconds to
compile from Gamma source; it produced the exact 3,082-byte Bytes customer
receipt. It reconstructed the bound Epsilon receipt in 9.875 seconds, and the
compiled Epsilon exit, stdin, trap, rejection, and malformed-envelope controls
passed. The complete 591,392-byte bootstrap compiler then compiled the existing
180-byte Omega fixture to a 50-byte Alpha program; executing it exited 42. This
last stage took 105.255 seconds. These are contended functional timings, not
comparative benchmarks.

Completing the bootstrap compiler's needed Omega coverage and obtaining
reproducible Omega compiler self-rebuilds remain delivery work. The Gamma
compiler fixed point does not establish an Omega compiler fixed point.

The parameter/local/call customer can be requested with
`python3 tests/bootstrap/compiled_vm/source_test.py --omega-values`. It executes the
current manifested bootstrap compiler, verifies source-owned refusals and
executes the actual emitted tapes. The older modulo/shift/bitwise observation
above is historical only: those anonymous forms lacked the required carrier
meaning and are now explicit coverage refusals. It is not evidence that the
older compiler preserved their Omega semantics.

An earlier values-customer snapshot ran through the source-built Epsilon tape in
1,050.206 seconds. Nine emitted Alpha executables returned the expected values;
30 source-owned refusals checked empty, unfinalized publication and reset.
The complete Epsilon customer was 601,461 bytes, SHA-256
`0465ac3c9ee18d7f59b62a4cf1ca7d8323a5c0324e9ac7f6fdeaea76cad64c32`.
The separately reconstructed Epsilon tape retains SHA-256
`5f12d53066179df5e92929ab7a41933a2ed3c0234400d9c7b3cda8718a469c12`.
These are functional observations under concurrent machine load, not controlled
performance comparisons. The bootstrap compiler still cannot construct the
Omega compiler source closure or demonstrate its self-rebuild.

The changed legacy `controls`, `controls_c`, and `controls_d` bodies completed
with their exact reset tapes in a shared invocation (603,476-byte customer,
SHA-256 `64f98ca7aff4e1f651a3a347ea4cc3f955d74795b50370d2eb0088fa43e75a36`).
Each emitted tape executed with exit 42. That invocation then exposed the
previously unsupported ungrouped terminal call in `controls_i`; it was not a
successful complete batch. The repaired parser reused call-expression retention
instead of changing that witness. The unchanged `controls_i` body, four
structural-overload/duplicate refusals, a bare nested parameterized call and
reset then passed in 420.248 seconds. This 604,432-byte customer had SHA-256
`601f5013b8ff23c35f934fe5da184ea85e10ed4af187b4439f609b29754adf41`;
its three emitted artifacts executed with exits 42, 47, 43.

After restoring each body's diagnostic anchor following declaration census,
the final 598,333-byte compiler closure has SHA-256
`0c402370392af73910cf6caa05b0198dd7ccf0820397e26b85367a1dfcec9f72`.
Its focused early-body refusal checked canonical source coordinate 0 and empty
publication, then reset and emitted an actual executable returning 43. That
600,677-byte customer, SHA-256
`20ffacd689a6712f78070ccde506f14a4abe5f9edaaa3f4f9189893b05a620ed`,
passed in 76.013 seconds. The older observations above retain their own input
identities; the entire matrix was not rerun after this diagnostic-only repair.


## Compiler-source lexer customer

`--omega-control` constructs and executes the real lexer identifier predicates,
with their bodies and signatures unchanged, through the source-built Epsilon
tape above. The wrapper uses explicit `as +Trapping` casts and tests both
predicates across ASCII boundaries and the upper half of `u32`.

The source-built runtime control runs to completion by default. It accepts an
optional `--seconds` limit. The development route uses `--omega-timeout`
and defaults to 7,200 seconds for this control. A timeout leaves the
Omega result open. Additional
source-owned cases check staged parallel transfers, skipped looping Boolean
operands, nested calls/comparisons, domain casts, refusals and compiler reuse.
This extends the bootstrap compiler toward an actual successor source unit;
it does not construct the complete Omega compiler.

A canonical explicit-domain smoke completed in 478.167 seconds and emitted a
2,153-byte Alpha program that exited 42 without output. Its Epsilon source was
644,808 bytes, SHA-256
`449d795abd68930a95fc0f08ee190caa3bfb2519465851e96b693370f8a09b7a`,
including bootstrap compiler
`7f0540c05d1c31033f3164dc1fb0c552d41ae2e57764451387c2618e47de46e3`.
The exact 1,341-byte Omega customer had SHA-256
`d26ea6de810d23de785af5b4cfc9a7490b865431ddedf181b878343879268117`;
its tape had SHA-256
`58a5d2ff0e45242e9382c7f940fdfede2089742152d8bb747a61b633e7ccf0b7`.
This observation predates the final public crash-ceiling and visibility checks.

An earlier grouped run failed at its short-circuit call-expression customer
(Epsilon exit 12 after 1,114.986 seconds). Its two preceding tapes executed,
but the earlier lexer wrapper also omitted required explicit domain casts;
that acceptance is not conformance evidence. The repaired parser preserves
expression context around nested calls instead of weakening the looping-RHS
witness. Historical observations remain bound to their own source bytes.


The later boundary/control invocation used a 653,357-byte Epsilon source,
SHA-256 `6863d967b67ff9311dd6f121348cb9e725e287bcfd325ce54c0d4e46b923c621`,
including compiler `6459924a185619f637f16166b10b1f29e274692c3c4f7580fd7bc1e2032d22bf`.
All 25 source-owned refusal checks completed, including public crash-ceiling
propagation in both declaration orders and empty/unfinalized output. Its first
six emitted tapes were separately executed with exits `42, 42, 43, 44, 45, 46`
and empty output. These cover the real lexer boundaries, parallel transfers,
skipped infinite Boolean operands, upper-u32 comparisons, nested calls and
explicit domain casts. The 4,352-byte real lexer customer has SHA-256
`169002a5c460e9af180c4f3954fc28fd601132b131164483d459fe3cbda38c33`;
its 6,095-byte tape has SHA-256
`925bed6ed031e0d3ed5ddbb0c4f21fa2771cb7912e002c73755076784a924f59`.

That invocation is still a **failed complete matrix**: its final public/plain
positive exposed a missing call-primary path for a transition subject and
returned Epsilon exit 90. The whole 9,063-byte observation has SHA-256
`4253b9bb07f6debeb0f1db968f55ce7fe699667aafc222be23b21708139f4bbe`.
Elapsed time was 2,644.870 seconds, including roughly 208 seconds paused to
avoid exceeding the shared 32 GiB memory budget; it is not a benchmark.
The exact positive was retained while the shared parser path was repaired.

Separately, the 35-invocation parser customer reached its complete `A` marker
before the legacy batch stopped at an unchanged leading-`!1` refusal witness.
That 677,942-byte Epsilon source has SHA-256
`7b0c364de404419b64eedc5f5b502635ede0a00af78ba62347756fa084e060cd`;
the failed batch took 623.514 seconds and returned Epsilon exit 2 plus `A`.
The repaired parser routes statement-leading unary prefixes through the shared
expression machinery. Neither failed batch is promoted to evidence for later
source bytes; the following focused observations establish the final repairs.


The repaired parser and legacy acceptance batch passed in 2,011.789 seconds:
37 parser invocations and 65 bootstrap compiler invocations completed with
native and Epsilon exit 0 and empty stderr. It used the 644,910-byte compiler
`5609861565dfbc77b8fb5586189f8dee2d490b267627b1bb9d35520ecbd682f7`
and a 686,637-byte complete Epsilon source,
`887c48bba803aeb7f08670a56c2eaf3b193b5b075d15864e57b497ac615fb5fd`.
The original parser, runtime prefix/public checks and baseline/legacy fixture
bodies were bound to Git `6710c426bccfff359e4f283d42d2e739c2547c6e`.
Its 728-byte observation has SHA-256
`b58537980b677917b92bbac2ca570ffc2c6aef1823086b955543c46d6880c44b`.
All nine emitted programs executed without output: the terminal-prefix and
skipped-loop program exited 43, the public/plain transition-call program exited
47, and all seven baseline/legacy reset tapes exited 42. The public checks also
covered transitive Trap ceilings, refusal coordinates, retired visibility syntax
and reuse after refusal. This was focused validation of the repaired paths,
not a rerun of the earlier complete boundary matrix.

A final refusal-classification repair distinguishes an unsupported visible
global terminal target from a genuinely missing target. The current 645,486-byte
compiler has SHA-256
`ec5caf34674af271f408e03bdbe77608b445c306956a0dc1064a41792141f377`.
Its three-invocation focused customer passed in 166.294 seconds: the visible
global target returned Incomplete, the missing target returned Reject, both
retained exact target coordinates and empty/unfinalized tape, then the reused
compiler emitted a 22-byte program that actually exited 49 without output.
The 647,792-byte complete Epsilon source has SHA-256
`cd0bad694b6a6c4622c2dc8578b1faa2ae930ec0b647ed640038bf1523b7a299`;
the tape has SHA-256
`5d6986842b024c50356357ef47eb431838ba94157e40091d2b998e7046db3373`.
The retained runtime fixture now includes that refusal as its 26th case. Earlier
full and focused observations retain their own compiler identities; they are
not reported as reruns on these final bytes.

## Records and scoped references toward SourceUnit

`--omega-storage` compiles the actual SourceId, SourceUnit and
`source_unit_clear` declarations, followed by a small caller, and executes the
result. The bootstrap compiler now has checked record storage, copy-value
transport, field writes, scoped shared/mutable reference parameters, independent
Unit completion and a bounded default-domain solver. Its detailed supported
semantics and refusals remain in [the compiler contract](../../../bootstrap/omega/compiler/README.md).
That observation closed one actual successor helper; the complete Omega
compiler remained unfinished. The later append/read customer below extends it.
The integrated compiler source grows
from 645,486 to 706,044 bytes (60,558 bytes, about 9.4%), including the shared
parser extensions. This is the audit cost of generic record/place support, not
a claim that the complete construction is now smaller or optimal.

The first successful source-owned clear observation took 234.069 seconds. It
used compiler 699,956 bytes,
`de2a5daf3598b488d91fdd5581a29a9354bdacad974fd1e0c0d467345b32f236`,
and complete Epsilon source 702,129 bytes,
`3f153e4867db19bfe543e3881129061a798ff51b64fe62826823f73b7c1173cd`.
The exact 780-byte Omega program had SHA-256
`dd163843c7ab778f828e1ae384a2bbb2e78fef125b036045b9c36ba70c6ed2d0`.
Its 1,206-byte Alpha tape,
`9226c731f5b9ede9eab633ffd240f3f07b9fe7dfea50fa406676f6478c8eb1a6`,
executed with exit 42 and empty output. The caller first establishes a SourceUnit,
sets length/retention fields, then checks the helper's three field updates.
It does not yet execute a nonzero array-element preservation check.

A separate focused storage invocation passed in 698.231 seconds: shared reads,
copy independence under field mutation, zero/mutable scalar locals, thirteen
refusal cases and compiler reuse. Its exact Epsilon source was 704,912 bytes,
`5d505a3a9ba0b39e0887aad3810c4991141703082abb25be2737c29aec21e0cb`.
All five tapes executed with exits 44, 45, 0, 2 and 59 and empty output.
The 1,382-byte observation,
`73fe5a3f8a033b3a2d5bb5293a54805a97cc83ca7fedffa2e7dd0ce7ed8fff2e`,
also matched the existing independent Epsilon model byte for byte. That model
was a development comparison, not part of the source-owned construction.
Both observations used the already source-constructed Epsilon tape
`5f12d53066179df5e92929ab7a41933a2ed3c0234400d9c7b3cda8718a469c12`.
Later nested-literal parsing and additional refusal repairs have separate source
identities; these observations are not promoted to validation of later bytes.

Failures remain useful evidence: an initial Epsilon source check caught a local
name shadowing a machine parameter; a later Omega refusal at byte 243 exposed
an inherited scalar-only parameter flag check on `&mut SourceUnit`. Both were
repaired without rewriting the actual helper. A prior unlocated refusal does
not establish a cause. The Rust development comparator independently refused
`SourceId { value: 77 as +Trapping }` because its domain pass did not use the
field's expected carrier. This is a comparator limitation against the specified
contextual landing rule, not evidence authorizing implicit qualification; the
exact source remains retained while that separate compiler repair proceeds.

On the later 706,044-byte compiler
`9f6abccac3ceab338541d78a77e912388146a51e3d60f0ebd326013cec670baa`,
the independent Epsilon model completed the retained storage fixture in 19.222
seconds. Its complete source was 714,065 bytes,
`77f05d47606f51828a04fc98e21999c13d540d15d7acfb0882b59684fe77d2e1`;
the 5,162-byte observation had SHA-256
`abea3e5f8b8e983f909974ccbaee614979c531027682a942eb54fb0cfc64726d`.
All nine development-produced tapes executed with the expected exits
42, 43, 44, 45, 46, 0, 2, 48 and 59. Sixteen refusals include a later overlapping
read, nested invariant violation, carrier-invalid requirement bound, unsupported
array equivalence and compiler reset. This is development evidence only;
source-owned execution of this final expanded storage fixture is recorded separately.


The final 59-invocation parser customer passed through the same source-built
Epsilon executable in 2,396.104 seconds on Linux x86-64. The exact 744,089-byte
source had SHA-256
`b5c761c1711dbf9c210d5aab44fc4255c8e8bff85b787f79377ce4c1872df40c`,
joining compiler `9f6abccac3ceab338541d78a77e912388146a51e3d60f0ebd326013cec670baa`
with the 38,045-byte entry
`50ec5053fd956e9291355246b64d741da8914130767538877e25599b33112385`.
Native status was zero, stderr empty and the exact observation was
`000000000041` (Epsilon Exit(0), then `A`). The retained checks include actual
SourceUnit rows, borrow-before-cast ownership, nested and returned records,
field calls preserving outer arguments, bounded-context refusal and reset.
All compiler and fixture bytes were checked against Git
`d1ebbb8e459e879f296a0abbb95ed963934aee77`; captured transport/runtime identities
were unchanged after execution. No interrupted run or timeout supplied this result.

Final source-owned storage acceptance also passed on that frozen compiler and
Git revision: 1,750.770 seconds for the Epsilon invocation, native status zero,
empty stderr, and the exact 5,162-byte observation shown above. Every retained
refusal asserted an empty, unfinalized tape; all nine published artifacts then
executed with their expected exits and empty output. The actual SourceUnit
customer remained the same 780 bytes throughout. These wall times describe
individual runs on a shared host, not a controlled performance comparison.

The separately repaired Rust development comparator
`37b4f2188d46c0e12096a3dcfc6af629d124706d6c6050dacfb9a99bc6e61518`
subsequently accepted that unchanged SourceUnit source through checked lowering
and CanonicalTerminalArtifact in 320.101 seconds. This supplies independent
admission/lowering correspondence, not native/Psi runtime evidence or a semantic
stage in the bootstrap chain.


The compiler source is composed in order from `../../../bootstrap/gamma/compiler/compiler.gamma`,
`../../../bootstrap/gamma/compiler/frontend.gamma`, and `../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma`. Shared frontend and lowering remain
one semantic owner; `../../../bootstrap/gamma/compiler/linux_x86_64/native.gamma` is an alternative realization,
not an additional stage in this peer. Source composition changes function IDs
and therefore compiler bytes; earlier source/tape identities above remain
historical evidence for their exact recorded inputs. The Alpha backend retains
the audited seed-hole limit; native output has its own buffered-image limit.

After that split, a fresh cold construction used the 33,605-byte composed
source (`8cc6f6ce3d29ff38a7f94c48857ac6036f0fe09110a442087ebbbb98d48cee86`)
to produce a 458,402-byte tape
(`25b8c1875dd866f885654c5344a16153f3a434eff9555196657f5f0249ef1af1`).
Both Alpha-executed rebuilds matched exactly, and the 32 semantic/15 refusal
controls passed. Reference construction took 89.270 seconds on a shared host;
it is a functional observation. The independent peer's native executable of
this same compiler source separately emitted the identical Alpha tape.


## Explicit storage contracts through actual append/read helpers

The expanded storage customer compiles the actual SourceUnit clear/append/read
and ModulePath append declarations. Their private helper requirements now state
the existing guarded upper bounds, and shared/mutable array-shell views express
the intended plain-byte access. The executable checks empty/final/full append,
absent reads, retained flags and bytes, ModulePath capacity, and clear's canary.
The generic controls retain both a disjoint mutation that preserves the bound
and an earlier-argument mutation that invalidates it within one expression.

The final source-owned Epsilon observation passed in 3411.948 seconds,
with all 13 emitted Alpha executables producing their expected result and empty
output. All 36 refusal/reset checks passed without publishing a partial tape.
The complete 12,666-byte observation matches the independent model byte-for-byte:
`2cea12d28e8fad007802cac92f6c2838e387b387f9cbe22d68cbb1e627a5a7a6`.
The model took 17.808 seconds; it is a development comparison, not a construction
stage. Captured execution inputs remained unchanged.

The bootstrap compiler is 749,173 bytes,
`4b94fda7d672b3b27173077cdf57075824c0ecc6c56275338936a9c65b39a896`;
the complete Epsilon customer is 763,256 bytes,
`ab66dc787783d86a11e9705352f1843efac5ffd77a9c1db7f7b217b66662d42c`.
The exact extracted Omega program is 3,729 bytes,
`1db5ce53884cb7e02bc4bb2968ff985875448425562eefd1eae47cabbfedaac4`;
its 6,099-byte tape is
`03b0acfdf8ba67b365856bc7ac3667d841c27a1e0e4e843cc2c0242001bf8d93`.
This uses the already source-constructed 16,789,856-byte Epsilon executable
`09e43db0cfe48e8217fec1d571d5e8f9e5e15ed0ef29d4aea884d77acf473eab`
and 617,354-byte Epsilon source
`4a8c97f9ad8f3ef5bae6c2f9a1c72f3433405e6e79610169b03b03a74217fd8e`.
It is actual reference execution, not a fresh cold Gamma reconstruction.

The compiler closure grew 34,773 bytes from the preceding indexed-storage
milestone. The cost covers shared requirement parsing, checked arrival bounds,
exact-range mutation tracking, bounded addition and safe incoming-reference
transfer. It does not establish full package construction or reproducible
Omega self-compilation; record/enum arrays, larger storage and broader language
facilities remain necessary for the complete compiler source. The separate
103-invocation parser model suite passed on identical parser inputs in 7.584
seconds; this receipt does not claim a source-owned rerun of that entire suite.

The subsequent directory relocation was integrated without changing any
compiler member, parser fixture, actual SourceUnit declaration or runtime byte.
The relocated model invocation passed the same 13 executables and 36 refusals
in 18.921 seconds, producing the identical complete request, observation and
tapes. Closure, identity and path checks were rerun after relocation. The
source-owned receipt above retains its original pre-relocation input custody;
it is not presented as a second reference execution. An earlier interrupted
invocation produced no completed receipt and supplies no acceptance claim.

### Native Epsilon correspondence for the same storage customer

The independent peer's already source-constructed native Epsilon evaluator ran
the exact frozen PR2722 request in 907.570 seconds. All 13 emitted Alpha
executables and 36 refusal controls passed; the complete observation and every
tape matched the Alpha-VM receipt above byte-for-byte. This checks an actual
Omega-compiler source helper customer, not complete Omega product construction.

The 767,037-byte request has SHA-256
`1bce7413e7085408fd052a0cb786ff08f3f2f99bee2e69e6a8a5f633e1a81048`. Compiler, customer and
Delta-closure identities are the same as above. The reused native executable
is 26,119,826 bytes,
`b1cc4441a52ec91feae60d958ada4c400122ce070a2ec66bdd8982c16bbfe2e0`,
constructed in [PR2719](https://github.com/CathedralOS/Omega/pull/2719). Its
729,060-byte canonical evaluator-entry Gamma receipt is
`bec9011e5216557a59ba701ac2a4112774e5f48240c729b95ffc8297f704c368`,
identical to the earlier Alpha-VM evaluator's receipt. This invocation reused
that constructed runtime; it is not another cold construction.

Both realizations use the shared monotonic 32-byte pair allocation with no
garbage collection. A nonstopping sample observed a native high-water RSS of
16,436,004 KiB and 16,803,601,088 allocated pair bytes; the final maximum RSS
was not recorded. No resource failure, budget change or source mutation occurred.
The 907.570-second native and 3,411.948-second VM observations ran on a shared
host at different times; these are functional timings, not a controlled
performance benchmark. Stronger assurance and the remaining native construction
work stay explicit in [the independent peer](native.md).


## Actual UTF-8 decoder through checked arithmetic

The bootstrap compiler constructs the real `decode_scalar` declaration with its
SourceUnit append/read dependencies. Its emitted Alpha program passes 28 UTF-8
boundary cases, including malformed continuations, overlong sequences,
surrogates, truncation and the Unicode ceiling. Checked Trapping arithmetic
preserves operand order and realizes overflow/underflow as an abnormal Trap;
ordinary incoming-reference forwarding retains argument loans. Named value
variants replace the touched numeric realization tags.

The final source-owned run uses the same already constructed native Epsilon
runtime described above, with four fresh evaluator processes. All 13 emitted
artifacts match the independent model byte-for-byte and execute independently:
five terminate with Alpha's Linux SIGILL Trap, the other eight return their
expected values. All 14 refusal controls and the final reset canary pass. Only
batch zero compiles the full decoder customer; the other batches use small
generic sources. Batch three retains a success, every refusal and the final
successful reset in one compiler instance. Each invocation checks the complete
Epsilon source before execution; previous-source tapes alone are not acceptance
for this corrected compiler.

| Batch | Emitted artifacts | Native Epsilon seconds | Maximum child RSS (KiB) |
| --- | ---: | ---: | ---: |
| 0 | 1 | 970.332 | 21,031,988 |
| 1 | 7 | 165.265 | 3,502,628 |
| 2 | 3 | 324.005 | 6,488,740 |
| 3 | 2 | 153.049 | 3,270,436 |

Native Epsilon time totals 1612.651 seconds. These are functional observations
on a shared host, not a controlled benchmark or a fresh cold reconstruction.
Inputs stayed unchanged and no final batch needed a resource interruption.
The independent model passed in 46.762 seconds. The storage model regression
passed in 15.500 seconds with all 13 prior tapes and the complete observation
unchanged; that regression is a model result, not another native storage run.
Compiler identity/corruption controls and four source-closure tests also passed.

Frozen source checkpoint: `8f27a2b7f65f4f8554a2a96d3c4fe014cbfd1408`.
The compiler is 764,081 bytes,
`2d7602e4e1e63822be65b950ccb4170c9dfddcc6de8344f9ab0729209133a681`;
the complete Epsilon customer is 775,834 bytes,
`8107bea7fa2f8c6370d706037f1cb2418d99493c1239a0201452c957320a9434`.
The extracted Omega source is 12,406 bytes,
`7f89108d7c2191eb7a932159dcec418943b37d672beef81843342787e45e01f8`;
its 31,279-byte decoder tape is
`ea7052722e423ccd4f6133432482879dd725b6f878d896a649181028dcd92af8`.
The validation driver retains the selector and request framing in
[test.py](../../../tests/bootstrap/compiled_vm/test.py); all batches use that same compiler and Epsilon customer.
The existing native successor invocation now exposes this customer with
`python3 tests/bootstrap/independent/successor_test.py --omega-utf8 --seconds 14400`.
Its default route reconstructs predecessors first; the measurements above reused
the already constructed evaluator and do not claim a new cold invocation.

- Batch 0 observation: `169b7abb2b2e44dddb65606f27b3568ed40a6e8116c69e79ee230ae3ba404e58`.
- Batch 1 observation: `f9d78d1b0218c548e48a566feb882deba731ae6364c38a9516254f2d634cb1fb`.
- Batch 2 observation: `209d389076b3fc9dd63512d1b941a512c319cc5caa6ac8b03ce4e0fdb6db962d`.
- Batch 3 observation: `6cdc45313a524911f6d744ba70e0f2211811d3804cc72fa628cb935e79207c07`.

An earlier monolithic driver invocation on the superseded 760,804-byte compiler
`9b2ed3c5c7921e504f672324ca1b6510bf4be519d260d8506be369e1b79f8ece`
was gracefully interrupted after 1,387.703 seconds near the shared 32 GiB host
limit. Its child maximum RSS was 32,207,012 KiB, with no OOM event and no completed
observation. This was a host-resource interruption, not a semantic rejection or
completed bootstrap. That old compiler subsequently completed the isolated
decoder batch in 933.007 seconds at 21,028,664 KiB maximum RSS. Those receipts
remain historical, separate from the final four-batch acceptance above.
Fresh processes bound cumulative validation allocation; they do not guarantee
that any larger single compiler-source customer will fit this host.

The corrected compiler adds 14,908 source bytes over the prior storage milestone.
Unsupported indexed carriers remain coverage refusals rather than invented byte
mismatches, and a coverage fallback cannot overwrite a diagnosed source error.
The actual decoder uses canonical casts and explicit literal qualification.
This is construction of an actual compiler-source component, not complete Omega
compiler construction or reproducible self-rebuild. Full modules, sums and
broader compiler-source facilities remain delivery work.

## Following an instruction through both assemblers

For a register addition, the shared lowering calls `i_add`. Alpha writes
opcode 3 and two register indices in one three-byte field. Linux x86-64
loads the same two slots, appends `48 01 d8` for `add rax,rbx`, then stores
the result. Each field adds its own width to the label positions used by
later jumps. A jump holds the label identity until publication.

For `(def main () Int 42)`, the generated input loop checks whether the read
returned EOF. At Alpha tape offset `0x48`, the bytes `10 00 01` compare
registers 0 and 1 for equality. The next eight bytes encode the destination
`0xa1` in little-endian order. In the ELF, `48 39 d8` compares the same values. The
instruction at `0x40019e` is `0f 84 d6 00 00 00`, a jump on equality.
Adding its signed displacement of 214 to the following address, `0x4001a4`,
gives `0x40027a`. Both destinations start the call after the input loop.
The loop's backward jump encodes `e9 c4 fe ff ff` at `0x400275`.
Its displacement of −316 returns to the read at `0x40013e`, corresponding
to Alpha tape offset `0x3c`.
See the [Alpha assembler](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma) and [native assembler](../../../bootstrap/gamma/compiler/linux_x86_64/native.gamma).

At `a59d45e8bd77`, field layout still emitted absolute native jumps. Its
source-built native compiler was 1,543,898 bytes,
SHA-256 `04c864bd27c5ae5fb5870b2fb1339c63d7caf29541d53af3fbb8dc0f2e3eea89`.
Two native self-rebuilds produced those same bytes. It compiled the
35,123-byte Alpha-targeting compiler source into a native executable. That
executable built the 456,334-byte Alpha compiler tape, and the tape rebuilt
itself byte for byte (SHA-256 `d9620222d49ba7b5604bfd516751fe93b5ddfb78521fc5aff0538c88518e5629`).

Both assemblers reproduced the existing Delta compiler and Epsilon evaluator
images from the Gamma sources constructed at `b95769558667`. The table gives
each output size and the peak resident memory Linux reported for its compiler
process. Each run used an already constructed native compiler executable.
Both versions compiled the same input bytes on the same host.

| Gamma source | Alpha tape bytes | Peak KiB, old → new | Linux ELF bytes | Peak KiB, old → new |
| --- | ---: | ---: | ---: | ---: |
| Delta compiler (151,525 bytes) | 1,219,139 | 101,248 → 66,560 | 3,984,546 | 104,064 → 120,320 |
| Epsilon evaluator (729,060 bytes) | 7,876,070 | 645,504 → 417,408 | 26,119,826 | 663,552 → 771,584 |

The native assembler often records several fields for one operation. Its
higher compiler memory use on both sources is a cost of this representation.
Both Delta images produced the same 3,082-byte receipt. Both Epsilon images
ran the frozen bootstrap compiler on the Omega fixture, produced the same
50-byte Alpha program, and that program exited 42. The full product request
exceeded the evaluator's 4 MiB input limit and returned 253 with empty output.


### Direct Linux branches

The native assembler can jump on the condition the Gamma lowering requested.
A conditional transfer uses one six-byte x86 instruction after the register
comparison. A direct call or unconditional jump takes five bytes. The
earlier encoding used an inverse condition followed by a twelve-byte
absolute jump. The field writer now computes signed distances for x86 and
continues to compute absolute offsets for Alpha.

The following paired measurements used already constructed Linux compiler
executables. Delta had identical source at `b95769558667` and `124129cd2d6`.
The two Epsilon versions had different source. Every Alpha output matched
the earlier assembler byte for byte.

| Gamma source | Absolute ELF bytes | Relative ELF bytes | Peak compiler KiB, absolute → relative | Branch and call destinations checked |
| --- | ---: | ---: | ---: | ---: |
| Delta | 3,984,546 | 3,856,065 | 120,320 → 116,396 | 16,387 |
| Epsilon at `b95769558667` | 26,119,826 | 25,322,912 | 771,584 → 746,924 | 100,900 |
| Epsilon at `124129cd2d6` | 26,075,468 | 25,280,528 | 770,176 → 745,516 | 100,648 |

Across these executables the largest backward and forward displacements
were −25,319,803 and +25,312,592 bytes. The direct native compiler reproduced its own
1,484,898-byte ELF from 40,323 source bytes. The extra target-width and
target-value functions brought the Alpha compiler source to 35,337 bytes
and its own tape to 457,463 bytes, an increase of 1,129 tape bytes.
Both native Epsilon versions ran their corresponding bootstrap compiler
fixture under each encoding and produced the same 50-byte program. It
exited 42 in all four runs.

## Earlier nominal-case construction interruption

The candidate bootstrap compiler at `501411f37fe18b55b4e068811c36258f318b002b`
adds nullary copy sums for the unchanged source-Psi `NumericBase` declaration and
`lexer_digit_in_base` helper. Its 6,797-byte lexer customer checks all 256 raw
bytes against four independently authored base predicates, alongside identifier
boundaries. The independent model produced and executed 17 Alpha tapes and
passed 36 refusal/reset controls in 21.620 seconds. The storage model regression
retained all 13 earlier tapes and the complete observation in 18.887 seconds.
These are model results, not completed source-owned construction evidence.

The reused native Epsilon evaluator `b1cc4441…fe2e0` began the same complete
793,158-byte Epsilon customer, with compiler closure 778,742 bytes,
`f250f98315032419393d2a297cf5ff892bcfddb95346cf62f2cc32a2019659c5`.
Its request was `aacf5003762b373cad3cb5dc6c52a21cd834eb25be0a0c7fe586a0fb8d12c5c3`.
The observation was interrupted after 1,009.487 seconds when estimated
unreclaimable headroom on the shared 32 GiB host crossed the predeclared 1.5 GB
guard. The child maximum RSS was 22,525,132 KiB; no observation was published
and no OOM event occurred. This is a host-resource interruption, not a source
rejection, completed native result, or evidence that the architecture is
infeasible. No fresh cold construction was attempted.

Review also corrected one negative from unsupported brace syntax to a parsed
payload declaration. The corrected payload-layout refusal and subsequent default
reset passed a separate 11.655-second model run, asserting one parsed payload
field and the declaration-anchored refusal. The interrupted native request kept
its original bytes and cannot establish that corrected layout boundary.
The subsequent bounded native invocation completed on the actual classifier,
corrected payload/default reset and two-package source-set construction. Its
current source identities, execution results and measured whole-compiler refusal
are recorded in [the bootstrap compiler](../../../bootstrap/omega/compiler/README.md#executed-nominal-values-and-source-set-construction).
The interrupted observation above remains tied to its original bytes.
