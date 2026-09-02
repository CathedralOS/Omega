# Beta language and encoding

This document defines the source language whose deterministic encoding produces
an Alpha bytecode payload. [`source/alpha/SEMANTICS.md`](../alpha/SEMANTICS.md)
defines execution of that payload; Beta defines only the
text-to-bytes correspondence.

The encoding is a partial function `assemble(P) = T`. `P` must be a
well-formed assembly source below. `T` is the raw platform-independent bytecode
payload stored in a `.tape` file. Stamping a payload into a native seed adds its
four-byte length and host container; neither is part of `assemble` or compiler
identity.

## Lexical form

Assembly source is a byte stream in the bootstrap textual-ASCII envelope. The
only admitted source bytes are horizontal tab (`0x09`), line feed (`0x0A`),
carriage return (`0x0D`), and printable ASCII (`0x20..0x7E`). NUL, DEL, bytes
above `0x7F`, and every other control byte reject before tokenization at their
byte offset. There is no source decoding, BOM, Unicode classification, or
locale-dependent character rule. Outside a quoted `db` string:

- space, tab, CR, LF, and comma separate tokens;
- `;` begins a comment through the next CR, LF, or end of source; and
- separators and comments otherwise have no meaning.

The grammar is:

```text
program     := item*
item        := label-definition | instruction | data
label-definition := IDENT ':'
instruction := MNEMONIC operand*
data        := 'db' WHITESPACE+ STRING

IDENT       := [a-z_][a-z0-9_]*
REGISTER    := 'r' HEXDIGIT | 'r' HEXDIGIT HEXDIGIT
HEXWORD     := '0x' HEXDIGIT{1,16}
HEXDIGIT    := [0-9a-f]
WHITESPACE  := space | HT | LF | CR
STRING      := '"' string-byte* '"'
string-byte := printable-ASCII-except-'"'-and-'\\' | ESCAPE
ESCAPE      := '\\0' | '\\\\' | '\\"'
```

Leading zeroes are permitted. A `HEXWORD` denotes one unsigned word in
`0..2^64-1`. A `REGISTER` denotes one register in `0..255`. Hexadecimal digits
are lowercase only; uppercase `A..F`, decimal words without `0x`, bare `0x`,
and words wider than sixteen digits reject. Every label definition is unique
and every label operand resolves to one definition. A label such as `r100x` is
an identifier, not a register; only a complete one- or two-digit hexadecimal
form after `r` is a register. Only whitespace may occur between `db` and its
opening quote; a comma or comment there rejects.

The decoded string bytes for `\\0`, `\\\\`, and `\\"` are respectively `0`,
`92`, and `34`. Every other permitted string byte is emitted unchanged. `db`
bytes are data, not implicitly decoded instructions; ordinary Alpha control
flow must jump around embedded data when it is reachable by address order.

The source envelope does not restrict assembled data. `db` may produce control
bytes through its closed escapes, and instructions may compute or write any
byte. It only forbids embedding non-ASCII or invisible control bytes raw in the
audited assembly source.

## Instructions

Operand kind `r` encodes a hexadecimal register. Operand kind `x` accepts
either a `HEXWORD` or a label and encodes an eight-byte word/address.

| Mnemonic | Opcode | Operands | Width |
| --- | ---: | --- | ---: |
| `halt` | `0x00` | `r` | 2 |
| `imm` | `0x01` | `r x` | 10 |
| `mov` | `0x02` | `r r` | 3 |
| `add` | `0x03` | `r r` | 3 |
| `sub` | `0x04` | `r r` | 3 |
| `mul` | `0x05` | `r r` | 3 |
| `div` | `0x06` | `r r` | 3 |
| `mod` | `0x07` | `r r` | 3 |
| `loadb` | `0x08` | `r r` | 3 |
| `storeb` | `0x09` | `r r` | 3 |
| `load` | `0x0a` | `r r` | 3 |
| `store` | `0x0b` | `r r` | 3 |
| `jmp` | `0x0c` | `x` | 9 |
| `jz` | `0x0d` | `r x` | 10 |
| `jnz` | `0x0e` | `r x` | 10 |
| `jlt` | `0x0f` | `r r x` | 11 |
| `jeq` | `0x10` | `r r x` | 11 |
| `read` | `0x11` | `r` | 2 |
| `write` | `0x12` | `r` | 2 |
| `call` | `0x13` | `x` | 9 |
| `ret` | `0x14` | none | 1 |

Each instruction begins with its one-byte opcode. An `r` operand is its
one-byte register number. An `x` operand is its value as exactly eight bytes,
least significant byte first. A label operand's value is the absolute byte
offset of its definition from byte zero of the raw output payload.

## Deterministic two-pass encoding

Pass one starts `pc = 0` and processes items in source order:

1. a label records `labels[name] = pc` and contributes zero bytes;
2. an instruction advances `pc` by the fixed width in the table; and
3. `db s` advances `pc` by the number of decoded string bytes.

Pass two processes the same item sequence and concatenates:

1. no bytes for a label;
2. the opcode followed by each encoded operand for an instruction; and
3. the decoded bytes for `db`.

The pass-two output length must equal the final pass-one `pc`. Because label
names are unique and all references are defined, both the label map and every
fixup value are unique. Consequently a well-formed source has exactly one
encoded payload.

Malformed text, unknown mnemonics, invalid operands, duplicate or unresolved
labels, arithmetic overflow, and implementation capacity exhaustion are not
assembly programs and produce no `assemble(P) = T` judgment. Tool-level failure
carriers and private resource profiles are specified separately; accepting a
malformed input does not extend this language.

The admitted compiler profile retains at most `0x100000` source bytes,
`0x10000` label rows, and `0xffffc` output bytes. It checks each extent before
advancing and returns nonzero without publishing an artifact on exhaustion.

## Canonical assembler reconstruction subject

The current exact subject is small enough for total checked reconstruction:

| Subject fact | Value |
| --- | ---: |
| Source bytes | 17,019 |
| Source lines | 602 |
| Encoded payload bytes | 2,706 |

An admission certificate must bind the raw source and tape outside the proof
producer, partition every source item and output byte exactly once, reconstruct
every label value, and prove the pass-one extent equals the pass-two extent and
the persisted tape length. Byte equality then gives identical Alpha initial
programs under the same input and resource profile. By deterministic Alpha
semantics, defined output, halt, trap, resource, and divergence observations
are preserved in lockstep; no stuttering argument is needed for this encoding
edge.
