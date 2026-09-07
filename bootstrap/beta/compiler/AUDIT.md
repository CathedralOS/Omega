# Admitted Beta compiler audit

This document audits the exact Alpha tape that implements the trusted Beta
compiler. It is a finite subject, not a claim that self-reconstruction proves
compiler correctness.

## Bound subject

| Subject | Size | SHA-256 |
| --- | ---: | --- |
| `beta_compiler.beta` | 12,536 bytes / 383 lines | `2f9a9f55a2c708731567367380521bfe35b1c13a00a367febd1f9f654c25f320` |
| `beta_compiler_bytecode.tape` | 1,773 bytes | `4b1572a5ce406fc5b194e047ac6c5c601c1860261933381443a08c9041b77361` |

The tape partitions exactly into 1,613 instruction bytes at
`0x000..0x64c` and 160 mnemonic-table bytes at `0x64d..0x6ec`. Its readable
source contains 52 exact address assertions, 253 instructions, and 20 `dw`
words. Two names deliberately assert `0x123`: `scan_program` and its entry
loop are the same block.

## Decoded instruction inventory

Every instruction byte decodes under Alpha's written 21-opcode table. No
unknown or truncated opcode occurs.

| Instruction | Count | Instruction | Count | Instruction | Count |
| --- | ---: | --- | ---: | --- | ---: |
| `halt` | 4 | `imm` | 37 | `mov` | 55 |
| `add` | 24 | `sub` | 7 | `mul` | 1 |
| `div` | 0 | `mod` | 0 | `loadb` | 16 |
| `storeb` | 1 | `load` | 1 | `store` | 1 |
| `jmp` | 26 | `jz` | 5 | `jnz` | 3 |
| `jlt` | 24 | `jeq` | 19 | `read` | 2 |
| `write` | 3 | `call` | 12 | `ret` | 12 |

The counts total 253. Starting at `0`, following branch targets, both arms of
conditionals, call targets, and call continuations reaches all 253 instructions.
Every one of the 89 encoded branch/call targets is an instruction boundary
below `0x64d`; none enters an operand or the data table.

## Control-flow reconstruction

| Range | Routine | Beta responsibility |
| --- | --- | --- |
| `0x000..0x122` | entry/read loop | Initialize the fixed profile, validate and retain the complete source byte envelope, invoke the scan, and halt 0. |
| `0x123..0x1e3` | program scan | Select address assertion versus mnemonic, emit opcode/operands, account output, and dispatch `dw`. |
| `0x1e4..0x352` | tokenizer | Skip admitted separators/comments and return an ordinary token, address assertion, or source end. |
| `0x353..0x3d0` | hexadecimal word | Require `0x` and one through sixteen lowercase hexadecimal digits. |
| `0x3d1..0x416` | hexadecimal digits | Accumulate the checked token into the private word scratch. |
| `0x417..0x47e` | hexadecimal nibble | Map `0..9` and `a..f`; reject every other byte. |
| `0x47f..0x54f` | mnemonic lookup | Compare the token with the closed table and return opcode plus operand widths. |
| `0x550..0x631` | operand emission | Require a one/two-digit `r` register or a hexadecimal word and emit its exact little-endian bytes. |
| `0x632..0x64c` | `dw` emission | Account and emit one word through the common operand path. |
| `0x64d..0x6ec` | immutable table | Twenty-one Alpha mnemonics in opcode order plus `dw`, each followed by its NUL-terminated `1`/`8` width string. |

The call graph is acyclic. Its deepest route is entry → program scan → `dw` →
operand emission → hexadecimal word → hexadecimal digits → hexadecimal nibble:
six live Alpha return addresses, or 48 bytes. Loops use branches, not recursive
calls.

The shared hexadecimal-word routine checks the token length before reading
its first two bytes and requires both `0` and `x`. Address assertions, control
operands, immediates, and `dw` therefore use the same complete prefix check.
Keeping the former check only in the word-operand caller left address assertions
accepting malformed prefixes such as `qx0:`. That redundant caller block is
removed. Register operands retain their separate `r` prefix check.

Completion returns status 0. Malformed operands and malformed numeric address
tokens return 7; an unknown mnemonic returns 8; an invalid source byte,
source/output exhaustion, or an unequal address assertion returns 9. A late
failure may already have streamed a prefix, but only status-zero invocation
publishes an artifact.

## Memory and ceilings

| Region/value | Exact extent | Reason it is contained |
| --- | --- | --- |
| compiler tape | `0x000000..0x0006ed` | Fixed 1,773-byte admitted subject. |
| numeric scratch | `0x080060..0x080068` | One private eight-byte word, disjoint from code and source. |
| source buffer | `0x100000..0x4100000` | Exactly 67,108,864 bytes; each store follows `pointer != end`. |
| successful emitted tape | stdout only | Counted around every opcode/operand/data extent; maximum `0xfffffc` = 16,777,212 bytes. |
| Alpha return stack | down from `0x10000000` | At most six 8-byte entries; no evaluator allocation approaches it. |

At the source boundary, exactly 67,108,864 bytes are retained and an EOF probe
is performed; one additional byte returns 9 before any out-of-range store.
Source token pointers never leave that retained half-open span. The mnemonic
table and numeric scratch are fixed in-range addresses. The output count is
advanced by the complete width before operand and `dw` emission; equality with
`0xfffffc` is admitted and any greater operand/data extent returns 9 before
those bytes are written. Opcode emission writes its one-byte prefix, advances,
and then checks the same upper bound, so a failing raw process stream can contain
one byte beyond the successful-artifact ceiling. Status-gated invocation never
publishes that prefix.

The compiler executes no `div` or `mod`, has no computed control target, and
writes semantic memory only in the checked source span and eight-byte scratch.
It therefore cannot reach Alpha's arithmetic traps or undefined out-of-range
memory behavior under this profile.

## Source-to-tape correspondence

The retained checks establish complementary facts:

1. `root-audit.py` binds both hashes, independently parses the Beta source,
   checks all 52 assertions, partitions every emitted byte once, decodes the
   raw tape from Alpha's opcode table, validates the mnemonic table, and proves
   complete control-flow reachability.
2. `compiler-diamond.sh` runs the independent source-derived Beta relation over
   the compiler and example corpus and obtains byte equality with the admitted
   compiler.
3. `reconstruction.sh` runs the admitted tape itself on the canonical source
   and obtains the admitted 1,773 bytes exactly.
4. `register-address-regression.sh` pins the strict grammar, numeric control,
   exact/adjacent source and output bounds, and status-gated publication.
5. `word-prefix.sh` exercises 736 exact first-prefix-byte controls across
   initial and late assertions, `jmp`, and `dw`, each at EOF and before newline.
   It checks status and the exact successful or rejected-prefix output bytes.

The Python decoder/reference is diagnostic and not a bootstrap premise. The
authority is the finite source/tape pair, this published correspondence, the
written Beta and Alpha semantics, and audit of the admitted Alpha
realizations. Self-reconstruction supplies identity; it does not excuse review
of the 253 decoded operations above.
