# Omega bootstrap checked IR v18

CKIR18 is the focused, entry-bearing fixed-buffer slice for full-width `u64`.
It preserves the CKIR16 header and row shapes and assigns wire major `18`,
minor `0`, target `1`. Major 17 is a separate platform-neutral Console
relation and is not accepted as an alias. The selected CKIR18 profile has no
sums, cases, payloads, public constants, case arms, or static byte views.
Flags are exactly `1` and the entry machine ID is present; a library envelope
with `flags=0` and `entry=NO_ID` is not a CKIR18 publication.

Type kind `8` is ordinary unqualified `u64`. Its flags byte and reserved word
are zero. The four type words are the inclusive interval
`lower-low32, lower-high32, upper-low32, upper-high32`, ordered as an unsigned
64-bit integer. It has size and alignment eight and remains a scalar through
fields, frame slots, loads, stores, exact call parameters/results, block
parameters/edges, and returns. Opcode `1 Const` uses immediate 0 for the low
half and immediate 1 for the high half.

Opcode `4 IndexPlace` is selected only with a kind-8 index and a fixed array
whose element is the exact canonical unqualified `u8 [0..=255]`. The result is
an element place. The array length is at most 65,536 and indexing traps unless
the complete unsigned qword index is strictly below that length. An array
length of 65,537 is malformed status 251; exhausted public table or byte
ceilings remain status 252.

Opcode `8 Add` is selected only with two visible kind-8 operands and a kind-8
result. Its immediates are zero. It traps defensively on unsigned qword carry
and when the result is outside the declared result interval. For admitted
source, the authored `length + 1` is Exact and is justified by the true edge of
`length < N`; the checked-IR traps are required defense but are unreachable.
Opcode `9 Less` remains direct, pure, unsigned and same-carrier. A CKIR18
module must contain at least one selected kind-8 IndexPlace, Add, and Less.

Source `u64 in Trapping` index custody may be consumed while lowering this
specific profile because every selected use remains in partial IndexPlace or
pure Less. This is not permission to erase arithmetic policy generally. In
particular, the selected Add represents authored Exact arithmetic, not an
authored Trapping Add.

The canonical profile owns `SourceUnit { bytes: [u8;65536], length:
u64[0..=65536], last_retained: bool }` and an entry-bearing `Main`. Its four
machines are ordered `clear, append, byte_or_nul, run`; their block counts are
`1,3,3,1`. `run` clears, appends byte 70, reads index 0, forces the full-buffer
false path, observes the absent-index false path, and returns 70. This relation
does not claim Console/provider admission, a syscall ABI, or broader source
authority.

## Conservative x86-64 artifact

The focused Delta backend is
`omega-bootstrap-checked-ir-v18-to-elf.alp`. It accepts only major 18 and omits
historical version multiplexing, sums/cases, static views, public constants,
constructors, and unrelated arithmetic families.

IndexPlace preserves the array base in `R10`, loads the full index in `RAX`,
loads the array length in `R9`, performs `cmp rax,r9; jae trap`, then adds the
validated index to the exact-u8 base. Add performs qword `add` followed by
`jb trap`, then checks the declared inclusive interval using qword compares.
Less is one qword `cmp` followed by `setb`; the processor comparison already
observes both halves. Kind-8 loads, stores, calls, edges, parameters and returns
all use exact eight-byte transport. Every dynamic destination interval is
checked before publication.

The canonical artifact is 8,192 bytes with SHA-256
`83d5c09e1da6543a59514d0b1cff13e087032e3caafba39452268993a92ad0ce`.
This identity is evidence for the frozen handcrafted CKIR18 fixture, not a
claim about a future source producer.
