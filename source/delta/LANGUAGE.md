# Delta v1 language-contract draft — DESIGN-BLOCKED OWNER Q7

Delta is the last implementation language in the audited bootstrap spine. It
is a small, deterministic, C-like language for writing the compiler that
directly builds the first Omega compiler. Delta is not Alpha with nicer
spelling, an Omega subset, or an alternate definition of Omega.

This document is non-authoritative decision material until OWNER Q7 is resolved. It
records a candidate source and execution contract, including alternatives that
are still internally inconsistent; no compiler or conformance suite may treat
those choices as Delta v1. OWNER Q7 must select and reconcile the result taxonomy,
keywords, optional domains/contracts, builtin resolution, Console/string ABI,
scalar-transition miss behavior, and source-closure presentation. After that
ruling, this file must be rewritten as one self-consistent normative contract.

The eventual canonical Delta compiler is written in Gamma and emits Alpha
tape. Neither that compiler, the superseded Beta translator, a sample corpus,
nor Omega documentation defines or amends the language contract.

## 1. Subjects and results

A Delta judgment has four explicit inputs:

```text
DeltaV1(source_bytes, stdin_bytes, language_profile, execution_profile)
```

`source_bytes` is one complete Delta translation unit. `stdin_bytes` is a
finite sealed byte sequence. The language profile is exactly `delta-v1`; it has
no target-dependent switches. The execution profile supplies finite evaluator
resources but cannot change parsing, values, control flow, I/O, or any other
Delta rule.

The judgment has exactly one of these results:

```text
Exit(code, stdout)
Reject(diagnostics)
Trap(kind, stdout_prefix)
Exhausted(kind, stdout_prefix)
Incomplete(owner, reason)
Diverges(stdout_prefixes)
```

- `Exit` is normal Delta termination. `code` is an `i32`; `stdout` is the exact
  byte sequence written before termination.
- `Reject` is a source result. It is produced only before execution and has no
  artifact bytes.
- `Trap` is a language-level dynamic fault. Output already written remains the
  observable prefix.
- `Exhausted` is reserved for a source-visible resource whose operation is part
  of Delta. Delta v1 has fixed source-declared storage and no implicit heap, so
  the core language has no built-in allocation-exhaustion operation.
- `Incomplete` is not a Delta result. A compiler, checker, evaluator, or proof
  producer returns it when one of its private budgets is
  insufficient. It grants no semantic verdict and publishes no artifact.
- `Diverges` denotes an infinite Delta execution. A bounded evaluator that
  cannot decide whether more steps terminate reports `Incomplete`, not
  `Diverges`.

`Reject` diagnostics are exactly one ASCII line:

```text
delta-v1 reject CODE OFFSET\n
```

`OFFSET` is the unsigned decimal byte offset of the first token that makes the
program invalid, or the source extent when the missing token is at end of
input. `CODE` is the first applicable member of this ordered list:

```text
Lexical Parse Duplicate Entry Name Type Arity Control Exhaustiveness Unsupported
```

The checker processes declarations and each body in source order and applies
the static rules in their numbered order, making the one line deterministic.
There is no partial artifact or additional diagnostic text. Trap, exhaustion,
normal exit, and divergence have empty diagnostics. An implementation-specific
`E2G-...` marker is never a Delta diagnostic.

## 2. Source text and lexical rules

Delta source is a finite sequence of bytes from:

```text
HT LF CR SP ! through ~
```

CRLF is normalized to LF before tokenization; a bare CR is rejected. NUL,
non-ASCII bytes, and unterminated strings or comments reject. Horizontal tab,
space, LF, and `//` through the next LF or end of input are trivia.

Identifiers match `[A-Za-z_][A-Za-z0-9_]*` and are case-sensitive. Decimal
integer literals contain one or more ASCII digits. A negative value is written
with unary `-`; the literal magnitude needed to spell `-2147483648` is admitted
only in that expression. Character literals denote one ASCII byte. String
literals contain raw printable ASCII plus `\n`, `\r`, `\t`, `\"`, `\\`, and
`\xHH`; their value is the decoded byte sequence. Unknown escapes reject.

Reserved words are:

```text
boundary trait data case machine state transition let return
true false self mut in terminates by
```

## 3. Surface grammar

The grammar below uses `?`, `*`, and `+` conventionally. Punctuation is
literal. Newlines are trivia and semicolons terminate declarations and
nonterminal statements.

```text
program       := declaration+
declaration   := boundary_decl | data_decl | machine_decl | use_decl

use_decl      := "use" path ";"
path          := IDENT ("::" IDENT)*

boundary_decl := "boundary" "trait" IDENT "{" boundary_machine* "}"
boundary_machine
              := "machine" IDENT "(" parameters? ")" return_type? ";"

data_decl     := attributes? "data" IDENT "{" data_member* "}"
attributes    := ("#[" attribute_text "]")+
data_member   := IDENT ":" type ";"
              | "case" IDENT payload? ";"
payload       := "(" parameters? ")"

machine_decl  := "machine" qualified_name "(" receiver_and_params? ")"
                 return_type? contract_clause* block
qualified_name:= IDENT | IDENT "::" IDENT
receiver_and_params
              := receiver ("," parameters)? | parameters
receiver      := "&" "mut" "self"
parameters    := parameter ("," parameter)*
parameter     := IDENT ":" type
return_type   := "->" type
contract_clause
              := ("requires" | "ensures") expression
              | "terminates" "by" expression ("->" IDENT)? ";"

type          := "i32" | "u8" | IDENT
              | "[" type ";" NAT "]"
              | "&" "[" type "]" domain_note?
domain_note   := "in" IDENT

block         := "{" statement* terminal? state_decl* "}"
state_decl    := "state" IDENT "(" parameters? ")" block

statement     := "let" IDENT ":" type domain_note? "=" expression ";"
              | place "=" expression ";"
              | call ";"
              | "assert" expression ";"

terminal      := transition
              | "return" expression ";"
              | expression

transition    := "transition" expression? "{" arm+ "}"
arm           := pattern "->" continuation
pattern       := INT | "true" | "false" | "_"
              | IDENT "::" IDENT binder?
binder        := "{" (IDENT ("," IDENT)*)? "}"
continuation  := IDENT "(" arguments? ")" | expression

place         := IDENT | self_path | index
self_path     := "self" ("." IDENT)+
index         := (IDENT | self_path) "[" expression "]" ("." IDENT)?
call          := IDENT "(" arguments? ")"
              | self_path "(" arguments? ")"
arguments     := expression ("," expression)*

expression    := precedence-ordered composition of:
                 INT | CHAR | STRING | "true" | "false"
               | IDENT | self_path | index
               | IDENT "::" IDENT ("(" arguments? ")")?
               | call | "(" expression ")"
               | expression (".len" | ".as_slice")
               | expression "[" expression? ".." expression? "]"
               | "-" expression
               | expression binary_operator expression

binary_operator
              := "*" | "/" | "%" | "+" | "-"
               | "<<" | ">>" | "&" | "^" | "|"
               | "<" | "<=" | ">" | ">=" | "==" | "!="
               | "&&" | "||"
```

`attribute_text`, `use` declarations, `in IDENT`, `requires`, `ensures`, and
`terminates by` are admitted only when a selected Delta v1 profile assigns
them a rule below. Unknown attributes or clauses reject; they are never
silently skipped. Delta v1 assigns no runtime effect to `use`, attributes, or
domain notes. `requires` and `ensures` are checked assertions at machine entry
and normal return respectively. A `terminates by m -> P;` clause requires `m`
to be a nonnegative `i32` at entry and to decrease strictly on every recursive
or cyclic transfer governed by the clause; failure traps as `Contract`.

The authoritative v1 compiler closure uses only one translation unit,
`boundary trait Console`, data records, fixed arrays, `i32`/`u8`, receiver
machines, states, and the operations described below. The wider grammatical
forms remain part of Delta only where this document gives them semantics; a
historical sample does not add a form.

## 4. Static semantics

A program is accepted only when all of the following hold:

1. Declaration names are unique within their namespace. Field, case, machine,
   parameter, local, and state names are unique in their declaring scope.
2. Exactly one `data Main` and one `machine Main::main(&mut self)` exist.
   `Main::main` has no value parameters. Its optional return type is `i32`.
3. Every qualified owner names a data declaration. Receiver machines use that
   owner's single instance reachable from `Main`; ambiguous or missing
   instances reject.
4. Every type is defined, finite, and nonrecursive by value. Boundary fields
   are zero-size capabilities and may be used only for the declared boundary
   calls. Arrays have a positive decimal length representable as `i32`.
5. A data declaration is either a record or a sum. Mixing ordinary fields and
   `case` members in one declaration rejects in v1. Sum cases have at most three
   payload fields. Case tags are zero-based declaration indexes.
6. Every name, field path, state, case, and call resolves uniquely. Argument
   counts and types agree exactly. A value-returning call appears only in an
   expression; a void call appears only as a statement or continuation.
7. `i32` and `u8` are distinct. Reading a `u8` yields its zero-extended `i32`
   value. Storing to `u8` requires a value in `0..255`; otherwise execution
   traps. No implicit narrowing or truth conversion exists.
8. Array indexes and slice endpoints have type `i32`. Record fields and sum
   payloads retain their declared types. A sum transition is exhaustive: it
   names every case exactly once or ends in `_`.
9. State parameters are immutable bindings initialized by the selected arm.
   Locals and receiver fields are mutable. Reading a local before its
   initializer rejects.
10. Each reachable block ends in a transition, return, process exit, value
    expression, or—only for a void receiver machine—falloff. A transition has
    at most one `_` arm and `_` is last. Non-sum literal patterns are unique.
11. `Main::main` may call the four `Console` operations. Other machines may use
    only the operations declared for their owner and threaded through `Main`.
    Unknown methods reject.

## 5. Values, storage, and evaluation

All storage is declared in source. A `Main` value and every nested record are
created before entry with every scalar, array element, record field, and sum
tag/payload byte initialized to zero. Arrays never move. Delta has no ambient
heap, pointer arithmetic, host pointer, filesystem, environment, clock,
network, thread, process-spawn, or foreign-memory operation.

An `i32` is a signed two's-complement integer in
`[-2147483648, 2147483647]`. Arithmetic is deterministic:

- ordinary `+`, `-`, and `*` are checked and trap as `Overflow` when the exact
  mathematical result is outside `i32`;
- a binding annotated `in Wrapping` performs those three operations modulo
  `2^32`, then reinterprets the result as signed `i32`;
- a binding annotated `in Saturating` clamps those three operations to the
  nearest `i32` endpoint;
- `/` truncates toward zero and `%` has the dividend's sign; zero divisors and
  `INT_MIN / -1` trap;
- shifts use the low five bits of a nonnegative right operand; a negative shift
  count traps. `>>` is arithmetic. `&`, `^`, and `|` operate on the 32-bit
  two's-complement representation;
- comparisons return exactly `0` or `1`.

Operands, call arguments, assignments, and statements evaluate left to right.
`&&` and `||` short-circuit and require operands equal to `0` or `1`; any other
operand traps as `NonBoolean`. `true` is `1` and `false` is `0`. `min(a,b)` and
`max(a,b)` evaluate `a` then `b` and return one operand.

An array access first evaluates its base and then its index. An index outside
`0..length-1` traps as `Bounds` before a read or write occurs. A slice
`a[lo..hi]` is a bounded view of the same array with `0 <= lo <= hi <= length`;
invalid endpoints trap. `.len` returns the view length. `.as_slice` creates the
full view. Views cannot outlive a call and do not allocate.

Record values are ordered products in declaration order. Sum values contain a
tag and exactly the selected case payload, also in declaration order. A sum
constructor evaluates payload expressions left to right. Pattern binders are
introduced simultaneously after the tag matches.

Calls are deterministic and have value semantics for parameters plus one
mutable receiver reference where declared. A receiver call observes all prior
writes and commits all its writes before returning. Recursion is permitted.

`assert e` evaluates `e`; `1` continues, `0` traps as `Assertion`, and every
other value traps as `NonBoolean`. `requires` and `ensures` use the same rule.

## 6. Machines and transitions

Calling a machine creates zero-initialized storage for its locals, binds its
parameters, checks its precondition, and enters the machine body. State
declarations are control labels owned by that invocation; they are not
separately callable machines.

A straight-line statement completes before the next begins. Assignment
evaluates the right side before changing its destination. A state transition:

1. evaluates the subject once (`transition { ... }` uses subject `0`);
2. examines arms in source order;
3. selects the first equal literal/case arm, or the final `_` arm;
4. evaluates only the selected continuation arguments, left to right; and
5. transfers to the selected state or tail-calls the selected machine.

No arm match is a `NonExhaustive` trap; static checking prevents it for sums,
and the dynamic result remains defined for non-sum input that lacks `_`.

`return e` evaluates `e`, checks the postcondition, and returns it. A void
receiver machine may fall off its body or a state and returns only its updated
receiver. A value expression in terminal position returns that value. Returning
from `Main::main` is equivalent to `exit_process(value)`; void falloff exits
zero.

## 7. Sealed byte I/O and observations

The only v1 boundary operations are:

```text
read_byte()                 -> i32
write_byte(value: i32)      -> void
write_line(bytes: &[u8])    -> void
exit_process(code: i32)     -> never
```

`read_byte` returns the next sealed input byte as `0..255`, or `-1` after the
last byte. EOF is stable. `write_byte` appends one byte and traps as `ByteRange`
if its argument is outside `0..255`. `write_line` appends the view bytes and
then byte `10`. `exit_process` terminates immediately after all earlier output;
it performs no truncation or host-status remapping.

The observation profile contains exact `stdin_bytes` and observes only the
terminal result plus exact stdout or stdout prefix. For a compiler invocation,
successful stdout is the exact artifact byte stream; Delta adds no container,
encoding, or newline. Diagnostics are a separate channel and are empty for
`Exit`, `Trap`, `Exhausted`, and divergence.
Filesystem state, paths, environment, elapsed time, process identifiers,
heartbeats, producer identity, replay count, and native debug information are
not Delta observations.

## 8. Resource classification

Delta separates language storage from the machinery used to decide its
meaning.

### Source-visible semantic bounds

- Every `[T; N]` has exactly the source-declared `N`; bounds checks and
  zero-initialization are language rules.
- Record and sum sizes, machine/state parameter counts, and source string
  lengths follow the finite declarations. V1 imposes no small semantic count
  such as 128 machines, 128 locals, 1,024 states, or four parameters.
- A fixed-backing allocator written in Delta has precisely the capacity and
  failure protocol its source implements. It does not acquire hidden host
  allocation.

### Explicit execution-profile parameters

The verifier selects and binds:

```text
stdin_bytes
alpha_tape_bytes_max
alpha_memory_bytes
alpha_return_stack_bytes
execution_step_bound
```

These parameters bound one checked compiler execution or finite observation.
They do not change Delta meaning. Exhausting a private execution capacity
returns `Incomplete(owner, parameter)`. Reaching a finite observation step
bound without a terminal result is `Incomplete`, not evidence of divergence.
Every selected numeric value is recorded with the exact source/tape claim.

The source/image transport ceiling of 524,288 bytes is a compiler
resource-profile parameter, not the largest legal Delta program or stdin in the
language. A different checked profile may select a larger finite carrier
without changing the judgment.

### Private implementation budgets

Every absolute address, arena, source buffer, parser table, symbol table,
temporary SSA table, certificate arena, and output buffer in a Delta compiler or
checker is private unless the selected resource profile names it explicitly.
The 128-machine, 128-local, 1,024-state, four-parameter, string-scratch,
`Chunks`/tree, template, raw-output, assembly, line, and native-artifact limits
in the removed Beta translator/native-publication route were private
budgets, not Delta language limits.

Crossing a private budget must return `Incomplete` before emitting a semantic
result or Alpha tape. Emitting malformed or partial tape, accepting a
truncated table, aliasing adjacent regions, silently dropping a declaration, or
converting private exhaustion to `Reject` is nonconforming.

The source-declared arrays `[i32; 21528]`, `[u8; 524288]`, and `[i32; 16]` in
the current candidate `D` implementation are different: their capacities are
visible Delta storage chosen by that program. Its `reserve_typed` and
`reserve_byte` methods
define explicit success/failure behavior inside Delta and therefore do not
produce the language-level `Exhausted` result.

## 9. Delta-to-Alpha compiler relation

The Gamma-written compiler accepts exact Delta source bytes, applies the static
rules above, and emits one Alpha tape. Its required correctness proposition is:

```text
DeltaV1(source, input, profiles)
  observationally refines
AlphaV1(tape, input, corresponding_profiles)
```

The artifact-aware owner reconstructs both systems independently. Checked
simulation relates Delta machine/state transfers, stores, calls, I/O, traps,
and terminal outcomes to Alpha steps. High-level blocks may lower to many Alpha
instructions; erased source structure may lower to none. Every unmatched step
is silent and decreases a well-founded rank.

The compiler may use internal CFG, layout, and tape-construction
representations, but they do not become source semantics or external compiler
stages. Checked intermediate lemmas may structure the certificate. The compiler
emits tape directly and may not invoke the Beta compiler, Gamma evaluator,
Alpha assembler, or a host lowerer to finish its semantic work.

Agreement with the historical Beta translator, another compiler, or another
execution is diagnostic and does not replace source-to-Alpha refinement.

## 10. Excluded and compatibility surfaces

Delta v1 has no proof terms, dependent or linear types, domains with runtime
authority, generics, packages, traits other than sealed boundaries, optimizer,
concurrency, atomics, volatile access, exceptions, implicit allocation,
garbage collection, arbitrary pointers, or ambient host services.

Forms outside this document are rejection tests, not a compatibility surface.
In particular, an Omega-looking file accepted by Delta has only this contract's
Delta meaning. No alternate translator or compatibility harness is retained.

## 11. Conformance obligations

Conformance requires all of the following:

1. complete positive and negative grammar coverage against this document;
2. independent reconstruction of static judgments and Delta small-step
   execution;
3. exact observation tests for exit, stdout, rejection, every trap, private
   `Incomplete`, and source-visible fixed-storage behavior;
4. mutation controls over source identity, rule identity, input/resource
   profile, Alpha tape, and terminal observation; and
5. direct checked source-to-artifact refinement under the lattice decisions.

The Gamma-written compiler tape may receive authority only after these join.
Byte-identical replay, native producer pedigree, and successful execution are
useful diagnostics, not substitutes.
