# Omega bootstrap checked IR v20

CKIR20 is the focused entry-bearing `TokenStream::push` successor. It composes
CKIR5's pure copyable-sum layout, semantic `Copy`, constructors, structural
calls, and exhaustive `CaseDispatch` with CKIR19's guarded full-width `u64`
record-array places, scalar field stores, exact count `Add`/`Less`, wide pure
calls, and 2 MiB private-owner ceiling. It assigns major `20`, minor `0`, target
`1`, flags `1`, and selected entry machine `2`; other majors are not aliases.

The inherited 100-byte header and all CKIR5 row widths/table ordering are
unchanged. Opcodes `1..=14` are admitted only as selected below. Constant DAGs,
static byte views, public layout, sums outside the exact five families,
noncopy structural arguments, effectful or trapping arguments, and later
opcode families are excluded. This carrier demonstrates one private checked
execution relation, not the complete lexer, a public ABI, source admission, or
compiler authority.

## Exact declarations and private layouts

Nominal record types `0..7` and records `0..7` are, in order:

```text
SourceId [copy] { value: full u32 in Trapping; }
Span [copy] { start: full u64; end: full u64; }
SourceSpan [copy] { source: SourceId; span: Span; }
Token [copy] { kind: TokenKind; source_span: SourceSpan;
               decoded_start: full u64; decoded_length: full u64; }
LexDiagnostic [copy] { code: LexDiagnosticCode; source_span: SourceSpan; }
TokenObservation [copy] { four u8; full u32; four full u64; }
TokenStream { [Token;16384], [TokenObservation;16384], u64[0..=16384],
              [u8;65536], full u64, LexDiagnostic, bool, bool; }
Main { stream: TokenStream; }
```

Nominal sum types `8..12` and sums `0..4` are `NumericBase` (4 cases),
`KeywordKind` (30), `PunctuationKind` (42), `TokenKind` (9), and
`LexDiagnosticCode` (20). All are `[copy]`. `TokenKind` cases are exact:

```text
Identifier;
Integer(NumericBase,bool,bool);
Float(bool,bool,bool);
StringLiteral;
Keyword(KeywordKind);
Punctuation(PunctuationKind);
Whitespace; LineComment; BlockComment;
```

The other sums have no payload. There are 105 cases and eight payload fields.
Every active payload is semantically copied; inactive payload bytes and padding
remain unobservable. Copy stages the selected tag and all active semantic
leaves before committing any destination leaf, including an aliased source and
destination.

The independently derived `(size,alignment)` pairs are:

```text
SourceId (4,4), Span (16,8), SourceSpan (24,8), TokenKind (12,4),
Token (56,8), LexDiagnostic (32,8), TokenObservation (40,8),
TokenStream (1638456,8), Main (1638456,8).
```

`TokenStream` field offsets are `0,917504,1572864,1572872,1638408,
1638416,1638448,1638449`. No producer offset is trusted. The selected owner is
below the exact 2 MiB ceiling; a validated larger owner selects 252 before
publication. The deterministic ELF uses a page-rounded 1,642,496-byte BSS.

Kind 8 remains ordinary flags-zero CKIR `u64` with four positional endpoint
words. Authored `in Trapping` source custody is consumed only at selected
checked indexing/scalar transport sites. Count `Add` is source-Exact under the
true edge of `count < 16384`; defensive carry and result-range traps remain in
the checked carrier/backend and must be unreachable for admitted source.

## Selected machines and execution

Machine `0`, mutable `push`, has ten explicit parameters:

```text
SourceId, TokenKind, u64, u64, u64, u64, u8, u8, u8, u8
```

Its entry stores `last_retained=false` and branches on full-width
`token_count < 16384`. The true block independently reconstructs every
runtime-selected record-array place and performs exactly fifteen data
assignments:

- semantic whole-sum `Copy` into `tokens[count].kind`;
- semantic nested-record `Copy` into `tokens[count].source_span.source`;
- four scalar Token stores;
- nine scalar TokenObservation stores.

The `source.value` observation is loaded through the just-copied nested
SourceId place, not invented as a second scalar argument. The block then
performs one exact count `Add`, stores the result, and sets status true. The
false block leaves status false. Every selected IndexPlace repeats its runtime
bound and address-overflow checks despite true-edge custody.

Machine `1`, shared `read_kind`, has one full-u64 index. It first guards
`index < token_count`, then derives `tokens[index].kind` and performs a
place-mode exhaustive nine-arm `CaseDispatch`. CKIR5 requires every active
payload be bound, so Integer, Keyword, and Punctuation use bounded discard
blocks rather than reading inactive bytes. Float binds its three Booleans and
accepts only `true,false,true`; the retained state independently indexes
`observations[index]`, loads its tag, and returns it. Other paths return zero.

Machine `2`, mutable zero-argument `run`, is the entry. It constructs
`SourceId { value:4 }` with opcode 13 and `TokenKind::Float(true,false,true)`
with opcode 14, then calls `push` with u64 values `5,6,7,8` and observation
bytes `70,1,2,3`. It calls `read_kind(0)` through a second real receiver call
and returns 70. Every argument is already materialized, pure, and nontrapping;
no evaluation-order claim extends beyond this tuple.

The machine-parameter ceiling is 16; the selected maximum is ten. Block
parameters retain the inherited ceiling seven. The exhaustive dispatch uses
eleven block parameters and eight selected-case argument rows across distinct
blocks; no block exceeds three parameters.

## Conservative x86-64 artifact

The focused backend is `omega-bootstrap-checked-ir-v20-to-elf.alp`. It accepts
only this selected major/profile and derives records, sums, active payload
leaves, field offsets, array strides, frames, call scratch, branches, and ELF
extents independently.

For a record-array IndexPlace it loads the complete qword index, compares it
unsigned against the exact array length, traps at or beyond the bound,
multiplies by the derived record stride with signed-overflow defense, and adds
the result to the base with carry defense. Scalar qword Const/Load/Store/Add,
range checks, call installation, and return transport use the conservative
CKIR18/19 templates. Sum construction stores the validated tag plus active
payload leaves. CaseDispatch validates the runtime tag before selecting an arm.
Semantic Copy snapshots the tag and selected active leaves before any commit;
it never performs a blind byte copy of inactive payload or padding.

The canonical fixture is 13,704 bytes with SHA-256
`ee418b04a4c661d329fe55198ae2b1063c86f5ed421711b9b0ab88f5eff6351a`.
Counts are types 24, records 8, fields 29, sums 5, cases 105, payload fields 8,
machines 3, machine parameters 11, blocks 14, block parameters 11, operations
183, operands 180, terminators 14, case arms 9, case-arm arguments 8, values 67,
and places 118. Both constant tables are empty.

Native/self parity and independent template checking freeze the canonical ELF
at 16,384 bytes with SHA-256
`194bfd8efcc1bb8f48d1347de64f53331967247a6eba69c63a52163fa779935a`.
Its RX file extent is 16,384 bytes and its page-rounded zero-file TokenStream
BSS is 1,642,496 bytes. Source production remains a separate lowering claim.

## Failure and evidence

Malformed/profile failure selects 251 with no publication. Validated table,
byte, frame, text, owner/BSS, or traversal exhaustion selects 252 with no
publication. The focused corpus covers wrong major/EOF/entry, forbidden
opcode/type families, declaration/copy flags, payload type/arm drift, missing
or mistyped Copy, missing/duplicate store, call target/arity/signature, high-half
and exact-bound runtime index traps, qword call transport, full-buffer control,
copy aliasing, invalid runtime tags, 65,536/65,537 array controls, owner just
over 2 MiB, and operation exhaustion.

Evidence is conjunctive: independent decode/layout/profile validation,
independent interpretation, native/self Delta backend parity, exact ELF and
template checks, runtime trap controls, and separate lower-rooted artifact
reconstruction. No output survives status 251 or 252.
