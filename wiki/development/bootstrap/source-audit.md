# Reading the Gamma and Epsilon bootstrap sources

Three changes made a reviewer’s comparisons more direct. The Epsilon wire entry and Gamma compiler got smaller. Spelling out the Epsilon keywords cost 894 bytes of evaluator source and made all 17 spellings and token codes visible together.

| Change | Before → after | What was checked |
| --- | --- | --- |
| [Epsilon evaluator boundary, #4542](https://github.com/CathedralOS/Omega/pull/4542) | A byte selector begins `(if (eq index 0) 69 …)`. The entry now says `"EEREQ"`, spells the refusal marker `"EEOUT"` and embeds one hexadecimal SHA-256. Entry source fell from 10,563 to 6,359 bytes. | Both entries passed 49 envelope cases, including the first differing byte at every marker and digest position. Seventeen Console cases checked the output prefix, line ending and distinction between a guest byte-range trap and an impossible view element. |
| [Gamma scanner and compiler, #4546](https://github.com/CathedralOS/Omega/pull/4546) | `(eq byte 40)` became `(eq byte '(')`. Syntax words are grouped by length, e.g. `(c_chars2 at 'i' 'f')`. The two source files fell from 33,737 to 31,662 bytes. | Both targets rebuilt themselves byte for byte. The Alpha compiler fell from 230,666 to 212,403 bytes and the Linux x86-64 compiler from 226,861 to 226,310. Both classifiers passed 137 spelling and near-miss inputs. |
| [Epsilon keyword table, #4548](https://github.com/CathedralOS/Omega/pull/4548) | `epsilon_span_is_8 … 98 111 117 110 100 97 114 121` became `(EpsilonKeyword "boundary" 10 …)`. Packed evaluator source grew from 617,122 to 618,016 bytes. Its generated Gamma source grew from 726,582 to 729,482. | The chosen table takes 421 Gamma pairs per ordinary source view and allocates none per lookup. Both Epsilon compilers emitted the same 1,106,536-byte public lexer module. Its 23 cases returned the expected 465 bytes. |

## Follow a Gamma operation into the Epsilon compiler

At `c6d64f0f3c4`, we ran the same 152,036-byte Delta compiler two ways
to build the Epsilon compiler. Both runs began with the Linux x86-64 Alpha
seed and the 1,773-byte Beta tape. Beta rebuilt itself and assembled the
48,303-byte Gamma interpreter. From there the reviewer follows different
implementations of Gamma.

| How Delta's Gamma source runs | Additional work to audit at this stage |
| --- | --- |
| Interpret it directly | Follow the Beta interpreter's scanner, declaration check, evaluation stack, call frames and heap. The interpreter reads the Delta compiler plus its data and writes the result directly. |
| Compile it to Alpha | The interpreter first builds the 34,932-byte Gamma compiler from its frontend, lowering and Alpha emitter. The compiler reproduces its 176,220-byte tape, then emits a 490,236-byte Delta compiler tape. Both tapes execute on the same seed VM. |

Direct interpretation has fewer Gamma components to review through the point
where Delta publishes Gamma source. If the next stage compiles that source to
Alpha, the full route still includes the Gamma compiler and its emitter.
Both routes also rely on the seed VM, the Beta implementation and the same
Delta source. Agreement checks how Gamma executed that source. Testing what
the resulting compiler accepts and rejects requires separate inputs.

For a concrete example, Delta's `gamma_kind` reads the first field of an
internal node. In the Beta dispatch table, `first` is the comment beside
`dw 0x7473726966`. Read that word from the low byte upwards to verify the
spelling. The interpreter's `eval_first` checks the pair tag, heap bounds,
alignment and marker, then loads the first value and its tag.
The compiler recognizes the spelling `'first'` in `c_short_kind`. Follow
kind 17 into the Alpha emitter's `alpha_project`, then into `e_project`.
The emitted instructions check the operand tag and load the two words.
Here the reviewer must account for who can create a tagged pair. On both
implementations, calling the production getter with `(pair 42 9)`
returned byte 42. Calling it with the scalar `42` trapped with status 2
and empty output. [Delta getter](../../../bootstrap/delta/compiler/implementation/representation/gamma/expressions.gamma) · [Beta implementation](../../../bootstrap/gamma/interpreter/gamma_evaluator.beta) · [Gamma lowering](../../../bootstrap/gamma/compiler/compiler.gamma) · [Alpha emitter](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma)

The construction request contains 628,952 bytes of Epsilon compiler source
written in Delta, 4,301 support bytes and the 16-byte header (SHA-256 of the
whole request `4fc83fba765b1e3b9b704f0cbbc9754e02733233b713dfbf33c88cc475a45f4c`).
The original interpreter and the Alpha- and Linux-hosted Delta compilers
produced the same 807,984 bytes of Gamma, SHA-256
`152059ad58aa048d20abfec74f0de9ea1ff20bd23d17a20d669365b4f9ca36fc`.
Delta took 596.72 CPU seconds through the interpreter and 37.13 as an Alpha
tape. Both had unlimited wall time. Changing profile 2 to profile 0 in that header left the source untouched.
Each Gamma route returned status 1 and the identical 40-byte `DCOUT`
refusal at request byte 8.

The Alpha-hosted route took the comparison through the bootstrap compiler’s
public scanner. Its 58,229-byte Epsilon source compiled to a 3,574,572-byte
Alpha tape. On 23 inputs the tape returned the expected 465 bytes, including
exit 42. Passing the source buffer as the scan offset gave the exact compiler
response `rejected 18 58106`. These are construction-profile results for
the quoted scanner. [Scanner and independent observations](../../../tests/bootstrap/independent/epsilon/lexer_scan_test.py)

The Epsilon view writer provides a separate Delta check. We took its
`epsilon_console_is_byte` definition and called it with the sum of the two
input bytes minus one. Both ways of running Delta produced the same 4,415
bytes of Gamma. Its Alpha tape returned 0, 1, 1 and 0 for −1, 0, 255 and
256. Passing the byte buffer directly to that integer parameter produced
the same 40-byte Delta type rejection on both routes (code 15 at byte 157).
[View writer](../../../bootstrap/epsilon/interpreter/execution/console/output.delta) · [Delta rejection format](../../../bootstrap/delta/LANGUAGE.md#compiler-boundary-family)

<details>
<summary>Replay the Epsilon compiler construction on both Gamma hosts</summary>

Start from `c6d64f0f3c4` on Linux x86-64 with a built `xtask` selected by
`OMEGA_XTASK_BIN`. The launcher snapshots the Delta compiler's source in
`program.gamma`, compiles it to Alpha, then feeds it the Epsilon compiler
request. To interpret Delta's Gamma source directly, prepend the length and
the source itself to the same request.

```sh
set -e
work=$(mktemp -d)
python3 tests/bootstrap/independent/whole_request.py "$work/inputs"
tests/bootstrap/linux_x86_64/bootstrap --alpha --delta . "$work/alpha" "$work/inputs/compiler.dcreq"
python3 - "$work" <<'PY'
from pathlib import Path
import sys
work = Path(sys.argv[1])
source = (work / "alpha/program.gamma").read_bytes()
request = (work / "inputs/compiler.dcreq").read_bytes()
(work / "direct.request").write_bytes(len(source).to_bytes(4, "little") + source + request)
PY
"$work/alpha/gamma-evaluator" < "$work/direct.request" > "$work/direct.gamma"
cmp "$work/direct.gamma" "$work/alpha/producer.output"
"$work/alpha/successor" < "$work/inputs/whole.epsilon" > "$work/alpha.module"
```

</details>

### Why the full constructor changes the host choice

The same checkout produced 26,354,274 bytes of Gamma for the whole
Epsilon-written Omega constructor, SHA-256
`215cd20cbc51fcdff281fb1075c474fc0afe3f4d1b89d141c18a974da6cd19cd`.
Its decimal literals and single-name bindings alone account for **19,065,212
bytes of Alpha tape** with this emitter. The seed accepts 16,777,212.

| Counted source form | Sites | Emitted bytes per site | Minimum bytes |
| --- | ---: | ---: | ---: |
| Decimal integer expression | 364,404 | 29 (two 10-byte immediates and a 9-byte push call) | 10,567,716 |
| Single-name `let` binding | 303,482 | 28 (9-byte pop call, 10-byte frame offset, 9-byte store call) | 8,497,496 |

The count skips the application marker and every source line containing a
comment or quoted literal. Binding counts exclude the initializer, whose
literals are counted separately. As a check, the cold-built Alpha compiler
emitted 1,129 bytes for `(def main () Int 0)`, 1,186 with one unused
`let x Int 1`, and 1,243 with a second such binding. Each added literal and
binding cost 57 bytes. [Emission rules](../../../bootstrap/gamma/compiler/compiler.gamma) · [Tape encoding and limit](../../../bootstrap/gamma/compiler/alpha/alpha_backend.gamma)

The original interpreter accepts a 137,363,456-byte framed input. At
30,349,179 bytes the generated Gamma, its length and the 305-unit product
fit that input boundary. This source has 8,005 declarations against a limit
of 2,097,152 and at most 148 nested lists inside a declaration against a
limit of 255. The interpreter scans tokens in the source buffer. It executed
the whole generated constructor on empty input in 21.09 CPU seconds and
returned the same five bytes as native (guest exit 3). With the freshly
Alpha-produced Gamma and a 111-byte Omega package, it emitted the 22-byte
tape that exited 42 on the seed. The 305-unit execution on the original
interpreter remains open. [Evaluator limits](../../../bootstrap/gamma/interpreter/EVALUATOR_PROFILE.md) · [Replay the compiler and small package on the seed](../../../tests/bootstrap/independent/WHOLE_REQUEST.md#running-the-compiled-omega-constructor-as-gamma-on-the-seed)

<details>
<summary>Count the two source forms on the generated omega.gamma</summary>

```python
import re
from pathlib import Path

literals = bindings = 0
integer = re.compile(rb"(?<![^\s(])-?[0-9]+(?=[\s)])")
single_let = re.compile(rb"\(let [^\s()]+ Int\s")
with Path("omega.gamma").open("rb") as source:
    for line in source:
        if b";" in line or b"'" in line or line.startswith(b"(def $application "):
            continue
        literals += len(integer.findall(line))
        bindings += len(single_let.findall(line))
print(literals, bindings, literals * 29 + bindings * 28)
```

</details>

## Reading punctuation in the Omega scanner

The scanner now reads a block-comment opener as `source[cursor] == '/'`
followed by `source[cursor + 1] == '*'`. The previous code compared those
bytes with 47 and 42. A 213-byte addition to the Delta parser sends quoted
bytes through the existing integer-pattern checker. Writing both `'/'` and `47` in one transition
reports a duplicate at the second arm. [Epsilon grammar](../../../bootstrap/epsilon/LANGUAGE.md) · [Scanner change](https://github.com/CathedralOS/Omega/pull/4646)

| Public lexer measurement | Decimal bytes | Quoted bytes |
| --- | ---: | ---: |
| Authored scanner source | 44,647 bytes | 44,749 bytes |
| Emitted Delta module | 1,061,558 bytes | 1,061,558 bytes |
| Generated Alpha tape | 3,561,692 bytes | 3,561,692 bytes |
| Generated Linux executable | 3,403,587 bytes | 3,403,587 bytes |

The 104 edits cover 83 transition arms, 18 comparisons and three other
operands. We aligned the two Epsilon sources token by token, decoded each
quoted byte and checked it against the original integer. In the two emitted
Delta modules, 15,370 numeric fields changed. Every change mapped an old
source coordinate to the corresponding new one. All remaining generated
text matched byte for byte.

Both generated targets ran the rewritten public scanner on all 256 one-byte
inputs. Their 5,125-byte observations matched the numeric scanner, including
its lexical diagnostics. Passing the source view as the cursor produced
type error 18 at the changed call, byte 58,106. [Route fixtures](../../../tests/bootstrap/independent/epsilon/lexer_scan_test.py) · [Parser and refusal controls](https://github.com/CathedralOS/Omega/pull/4643)

The complete Epsilon-written Omega constructor exhibited the same pattern.
The same Linux compiler compiled both its 899,558-byte numeric source and
its 899,660-byte quoted source. Their Delta modules were 18,248,084 and
18,248,100 bytes. Each of the 369,378 differing numeric fields
was one of 37,200 corresponding source coordinates. All other generated
bytes agreed. [Whole constructor](README.md#current-305-unit-product)

## One byte-view writer for both Epsilon variants

The 45-kind punctuation customer exposed a difference between Epsilon
implementations. The interpreter printed every spelling. The compiler
accepted the source types and then returned `unsupported 54517` at the
`Console.write_line` declaration. The compiler now calls the interpreter’s
existing view-output helper. Both generated targets printed the same 311
bytes on 90 scans, including the extent and spelling of every selected
punctuation kind. A bad slice retained the preceding output and trapped
before writing a newline. [Compiler change and receipts](https://github.com/CathedralOS/Omega/pull/4648)

Linking the shared helper also has a cost on programs that never call
`write_line`. The existing storage customer emitted the same 98,638-byte
module. Its linked Delta source grew by 2,015 bytes, the Linux executable
by 7,776 bytes and the Alpha tape by 8,451 bytes. All four old and new
executables returned exit 42 and byte `A` on input `02`.
[Storage customer](../../../tests/bootstrap/independent/epsilon/storage_main.epsilon)

## Writing the same bytes under both Delta profiles

On the original Gamma interpreter, the 256-byte classifier took 703.15 CPU
seconds with indexed output and 383.27 seconds with construction-profile
traversal. Both returned the same 3,077 bytes and refused the twin at byte
2,032. The smaller example reverses the result. Repeatedly doubling a
shared, empty-wrapped value produced exactly 1 KiB. Indexed output took
0.445 CPU seconds and 128 KiB of memory on Linux. Traversal took
0.965 CPU seconds and 32,640 KiB. The representation of the output can
therefore change which implementation is cheaper to run.
[Source fixture and measured profile comparison](https://github.com/CathedralOS/Omega/pull/4650)

## Which routes agreed

- **Four Gamma host and target pairs.** The #4546 Gamma compilers ran on Alpha and Linux and generated both targets. All four produced the same 726,582-byte Gamma source for the **#4542 Epsilon evaluator**. On either host the Alpha evaluator was 3,798,810 bytes and the Linux evaluator 1,985,896. Each passed five portable observations and two syntax refusals. [Host/target gate](../../../tests/bootstrap/gamma_host_matrix.py)
- **Nine small-program routes.** With the earlier 226,861-byte Linux Gamma compiler fixed, all three Delta choices crossed with all three Epsilon choices produced `00 5a 00 00 00 5a` on the same 339-byte stdin program and one input byte `Z`. The six bytes encode exit 90 and console output `Z`. The reported rows reused the cold Linux host with SHA-256 `249ffad4eba5d86a28427a259f58dd8aecb2b9bb10e2091157c0f81cc0cbdd13`. Those nine reports cover this one program. [Route selector](../../../tests/bootstrap/route.py)
- **One frozen Omega constructor.** On the 873,205-byte #4540 Omega compiler and construction entry, the Epsilon compilers before and after #4548 published the same 18,816,197-byte Delta module observation, SHA-256 `454b89f25d59fe14224d2cf3019acbdb2b390c6780e39dffeff5b5f6e41ec267`. The #4548 measurement ends at module publication. [Construction procedure](../../../tests/bootstrap/independent/WHOLE_REQUEST.md)
- **One linked product and two refusals.** The #4542 evaluator and a separately compiled 58,603,133-byte Linux executable of the #4540 constructor received the same 828-byte, two-package wire decoder input. Their full 1,612-byte observations matched. The 1,607-byte Alpha tape exited 42 on the seed (SHA-256 `f5ac1bf6ddc9158a4164a644c9342f73cc6c6698c93773e32dfe035176e3a371`). Removing the dependency gave guest exit 1 at coordinate 11. Appending one input byte gave guest exit 3 with empty payload. [Product and refusal controls](../../../tests/bootstrap/independent/product_wire.py)

These runs use fixed revisions. The four host/target tests predate the 729,482-byte keyword-table evaluator. The wire tape exercises one Omega product; the complete Omega-in-Omega rebuild is still open.

## Public OCREQ on Alpha and Linux

The same Epsilon-written Omega compiler ran on the fixed Alpha seed and on Linux x86-64. Each compiled `machine main() -> u8 { 42 }` through the public OCREQ entry. Both emitted the same 22-byte Alpha tape, which exited 42 on the seed with empty stdout and stderr. Replacing byte 3 of the OCREQ marker with `!` made both report that exact byte. [Public compiler entry at #4634](https://github.com/CathedralOS/Omega/blob/f552e5b8274d73b1435d803a15b1743e581b81e1/tests/bootstrap/conformance/omega/main_ocreq.epsilon)

| OCREQ input | Full Epsilon observation on both hosts | SHA-256 of observation |
| --- | --- | --- |
| Valid, 188 bytes | 27 bytes · guest exit 0 and tape | `405b922ca60e8bc110ec662ee5aee0353bb3d4c2ca4a7fce5d8ade1bbdef23bd` |
| Bad identity, 188 bytes | 45 bytes · guest exit 1 and malformed OCOUT at byte 3 | `abab215911641d6d1035806c9c9102da44a4e896514be94aca2252b1054c3868` |

All four evaluator processes exited 0 with empty stderr. Their complete observations also matched the independent Epsilon source model. The emitted tape had SHA-256 `25c789ed6ce473847a1dc82854aad7b59d3ff336d4ed73a64a56c540b5d78384`.

<details>
<summary>Host identities, request digests and replay</summary>

At `f552e5b8274d73b1435d803a15b1743e581b81e1`, the packed Omega compiler library plus public entry was 901,333 bytes, SHA-256 `07fed33d92c9ab84c910726e9184dda526d9961271ed1936ac32cac1d05775f8`. The valid OCREQ digest was `4cd8ca519b2e3519ccb19c25ba9ab773be14e465d0472fe90c077c2e906d6d0b` and the changed one was `bd3c30f4994808c730ed3c5d4083ff312e559cbf62c8f541997a2379ac85e9a1`. Both outer EEREQs used wire profile 1. Each bound the Delta library of its own evaluator.

| Evaluator | Packed library SHA-256 | Executable SHA-256 |
| --- | --- | --- |
| Alpha · `d3a79047ce09` library, conformance Delta profile, `d00a570048e5` Gamma backend | `7b214d058c086ffcf5e5481e5dd08e4e6f47275ae07a4cb2cb6b4d26764b1bae` | `5bd3a4ed76e12931add218666ccd55d76c9d38559a4c44509a764290e43f1c0c` (stamped seed) |
| Linux x86-64 · `9df6f79be79` library, construction Delta profile | `c5392b0a71cbe58de5090fb888b379fa5cb3b4b598ba38d6f1a568e326ffa0ff` | `5b17b0fb5fcf8ce8006861a62f68adbb68f494e07c0e04f88e2522043bddd33f` |

| Host | Case | EEREQ SHA-256 (901,573 bytes each) | Evaluator CPU seconds | Peak KiB |
| --- | --- | --- | ---: | ---: |
| Alpha | Valid | `80208f95ff8f7a79418c2639e426e6907f2949d4d6abe8567db9d7ee934fd198` | 275.61 | 306,292 |
| Alpha | Bad identity | `e0604df9aec3fff970cfd33ec20f298ed2f11925da0a7f0de5645133e8a6cfc2` | 226.14 | 241,096 |
| Linux | Valid | `5bceb29acfadca422425e8134fa22b9467354788b9e6b2ebae5f2b3ecb540513` | 32.39 | 303,736 |
| Linux | Bad identity | `63ecb63bc7efbd7c9d119989b3ab83533c534a88d6fd007d20a79f1680aa21bc` | 28.86 | 238,456 |

At the #4634 checkout root, set `EVALUATOR` to the absolute path of either executable above and `EVAL_TREE` to the checkout of its library revision. Set `OMEGA_XTASK_BIN` to a built `xtask` executable by absolute path. [Alpha evaluator build](compiled-alpha.md#compile-the-product-wire-decoder-through-epsilon-on-alpha) · [Linux construction-profile build](README.md#whole-omega-compiler)

```sh
work=$(mktemp -d)
"$OMEGA_XTASK_BIN" source-closure bootstrap/omega/compiler/omega_compiler.epsilon.sources "$work/library"
cat "$work/library" tests/bootstrap/conformance/omega/main_ocreq.epsilon > "$work/public"
(cd "$EVAL_TREE" && "$OMEGA_XTASK_BIN" source-closure bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources "$work/evaluator-library")
PYTHONPATH=tests/bootstrap python3 - "$work" <<'PY'
from pathlib import Path
import sys
from conformance.vectors import epsilon_request, ocreq_request
w = Path(sys.argv[1])
valid = ocreq_request(b"machine main() -> u8 { 42 }", b"program.omg")
for name, req in [("valid", valid), ("bad-identity", valid[:3] + b"!" + valid[4:])]:
    frame = epsilon_request((w / "evaluator-library").read_bytes(), (w / "public").read_bytes(), req)
    (w / (name + ".eereq")).write_bytes(frame)
PY
for name in valid bad-identity; do
  "$EVALUATOR" < "$work/$name.eereq" > "$work/$name.out" || exit 1
done
sha256sum "$work"/*.eereq "$work"/*.out
PYTHONPATH=tests/bootstrap python3 - "$work" <<'PY'
from pathlib import Path
import sys
from compiled import execute, stamp
w = Path(sys.argv[1])
(w / "tape").write_bytes((w / "valid.out").read_bytes()[5:])
stamp(w / "tape", w / "product")
assert execute(w / "product") == (42, b"")
PY
```

</details>

## Quoted ASCII integers in Delta

Delta lets the Epsilon lexer say `(ascii_between c 'A' 'Z')`. The shared
checker reads one quoted byte, the direct interpreter returns its integer
value and the compiler emits a decimal Gamma integer. The change added
1,015 bytes to the packed Delta compiler source relative
to #4572 (152,036 to 153,051 bytes). The Linux compiler grew from 507,409 to
511,856 bytes. [Literal grammar](../../../bootstrap/delta/LANGUAGE.md#program-form)

In a comparison on the same pinned evaluator entry, 146 byte constants in
Epsilon's lexical checker and token reader were respelled. Their generated
Gamma differed in names derived from Delta source offsets. Both built the
same 2,004,942-byte Linux evaluator, SHA-256
`023d970589e30d0806ddd7f38041a7e81660224e46fc8700b363a4d13d96721a`.
Both executed the 256-byte Omega classifier and rejected its wrong-type twins
at offsets 2032 and 2991. To exercise quoted delimiters, escapes and bad
source coordinates on both Delta profiles, run the direct interpreter and
compiled literal checks. [Direct tests](../../../tests/bootstrap/interpreted/delta/interpreter.py) ·
[Compiled tests](../../../tests/bootstrap/independent/epsilon/literal_controls.py)

<details>
<summary>Reproduce the measurements from pinned sources</summary>

Run on Linux x86-64 with Python, Git and a built repository `xtask`. Use fresh output directories. From the repository root, prepare the snapshot with the earlier Gamma compiler, the snapshot with the revised Gamma classifier, and the later Epsilon lexer.

```sh
export OMEGA_XTASK_BIN="$PWD/target/debug/xtask"
audit=$(mktemp -d)
git worktree add --detach "$audit/early" 9063f0819b5
git worktree add --detach "$audit/gamma" 43e334bc2b0
git worktree add --detach "$audit/keywords" 46d91bbc777
```

Run the small stdin control and the four host/target constructions.

```sh
cd "$audit/early"
python3 tests/bootstrap/route.py "$audit/nine" \
  --gamma native --delta all --epsilon all --case stdin --reuse-stages --seconds 360

cd "$audit/gamma"
tests/bootstrap/linux_x86_64/bootstrap --alpha . "$audit/alpha"
tests/bootstrap/linux_x86_64/bootstrap . "$audit/linux"
python3 tests/bootstrap/gamma_host_matrix.py --epsilon \
  "$audit/alpha" "$audit/linux" "$audit/four"
```

For the focused envelope and keyword checks, run the portable evaluator gate against a retained four-host receipt, then the Epsilon compilation controls at the keyword-table revision.

```sh
cd "$audit/gamma"
python3 tests/bootstrap/interpreted/epsilon/portable.py "$audit/envelopes" \
  --profile envelope --receipt "$audit/four/linux-to-linux-epsilon.gamma"
cd "$audit/keywords"
python3 tests/bootstrap/independent/successor_test.py --compile-epsilon
```

For the module comparison, the early snapshot packs the same Omega compiler and entry as #4540. Feed those exact bytes to both Epsilon compilers. The SHA-256 of `early-input/whole.epsilon` is `8bdbb90999c6b62a04754a3d452966b4903af74179fbc113fdc910a628d27401`. The request builder selects the Delta construction profile.

```sh
for rev in early keywords; do
  cd "$audit/$rev"
  python3 tests/bootstrap/independent/whole_request.py "$audit/$rev-input" --entry-control
  tests/bootstrap/linux_x86_64/bootstrap --delta . "$audit/$rev-build" \
    "$audit/$rev-input/compiler.dcreq"
  "$audit/$rev-build/successor" < "$audit/early-input/whole.epsilon" > "$audit/$rev.module"
done
cmp "$audit/early.module" "$audit/keywords.module"
sha256sum "$audit/early.module"
```

For the linked product, build the early module’s executable and replay the same customer on the evaluator from the four-host check. The historical Linux constructor had SHA-256 `ccc6eb867203f3250e9025e8f616d07e8b2253582b34499201ffe9e4ce4cb21b`. Both commands also execute the emitted Alpha tape and check both refusals.

```sh
cd "$audit/early"
python3 tests/bootstrap/independent/whole_request.py "$audit/early-input" --frame-result "$audit/early.module"
"$audit/early-build/program" < "$audit/early-input/omega.dcreq" > "$audit/omega.gamma"
"$audit/early-build/gamma-compiler" < "$audit/omega.gamma" > "$audit/omega"
chmod +x "$audit/omega"
python3 tests/bootstrap/independent/product_wire.py "$audit/compiled-wire" --constructor "$audit/omega"
python3 tests/bootstrap/independent/product_wire.py "$audit/evaluated-wire" \
  --evaluator "$audit/four/linux-to-linux-epsilon.bin"
cmp "$audit/compiled-wire/wire.tape" "$audit/evaluated-wire/wire.tape"
```

</details>
