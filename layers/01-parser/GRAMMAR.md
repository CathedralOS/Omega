# L1 Parser grammar

Status: **draft for Jarod's confirmation. Nothing here is decided, and no
parser exists yet.**

The spec has no grammar. This document derives one so the parser has a
contract to be tested against. Its sources, in order of authority:

1. **The spec's prose**, where it states a rule of form.
2. **The spec's 132 `omega` examples.** Each form below cites the examples
   that show it, as `file:line` under `wiki/spec/`.
3. **Omega V1's parser**, only where the spec is silent, and always marked as
   such. Those are gaps, to be decided like the lexer's were.

The parser's input is the lexer's tokens. The lexer has no keyword list, so
every word below, `machine` included, arrives as a plain word and it is this
grammar that gives it meaning.

## Notation

    name        another rule
    'text'      that exact token or word
    a?          optional
    a*          zero or more
    a+          one or more
    a | b       either
    ( a b )     grouping
    a % ','     one or more a separated by commas, trailing comma allowed

## A source file

    file        = item*
    item        = use | const | data | domain | trait | conformance | machine | math_let
    visibility  = 'public'

A file's namespace comes from its directory; there is no `module` header
(`language/modules.md`, "Directory namespaces").

## Paths and generic arguments

    path         = segment ( '::' segment )*
    segment      = word generic_args?
    generic_args = '<' ( generic_arg % ',' ) '>'
    generic_arg  = type | expression | '(' selection ')'

Seen as `Command::Operation::GetUser` (language/data_and_literals.md:20),
`Envelope<u32>::Body::Value` (:90), `Sensor<29u64>::read()` (language/effects.md:101),
`integer::max<i32>()` (language/conformances.md:39),
`run<(clear satisfies A::clear)>(&mut x)` (language/conformances.md:526).

## Items

### use

    use = 'use' path ';'

`use topology;` (build/scoped_execution.md:134), `use ui_policy::screen::Point::OnScreen;`
(language/modules.md:102).

### const

    const = visibility? 'const' path ( ':' type )? '=' expression ';'

Typed (language/constants.md:6), with a qualified name `EfiStatus::SUCCESS`
(same), and untyped `const clear_a = (clear satisfies A::clear);`
(language/conformances.md:526).

### data

    data        = visibility? 'data' word generic_params? attributes? dropped_by?
                  ( '=' type ';' | requires* '{' data_member* '}' )
    data_member = stable_number? word attributes? ':' ( type | data ) ';'
                | stable_number? 'case' word ( '(' ( case_field % ',' ) ')' )? ';'
                | data
    case_field  = stable_number? word ':' type
    stable_number = '#' integer
    attributes  = '[' ( attribute % ',' ) ']'
    attribute   = word ( '(' ( word ':' word ) % ',' ')' )?
    dropped_by  = 'dropped' 'by' path

- Records and sums, nested declarations (language/data_and_literals.md:20).
- A field whose type is declared in place: `op: data Operation { ... };` (:58).
- Stable numbers `#1` on fields, cases and case fields (:137).
- `requires` before the body (:272, :317).
- Alias form `data SmallBytes = ByteBuffer<64>;` (language/generics.md:138).
- Attributes `[linear]` (language/ownership.md:309) and
  `[linear, carry(suspension: allowed, ...)]` (resources/carry.md:6); a field
  attribute `proof [erased]: Valid<T>;` (proofs/contracts.md:406).
- `dropped by release_guard<T>` (language/domains.md:443).

### domain

    domain = visibility? 'domain' generic_params? path
             ( '=' type_domains ';'
             | requires? ( 'access' type )? dropped_by? ';' )

`domain i32::Km;`, `domain i32::Positive requires self > 0;`,
`public domain Reservation::Issued access ReservationAccess;`
(language/domains.md:131), a nested domain `i32::Positive::Small` (:666),
generic forms `domain<T> T::Metres;` (:701) and
`domain<T, const U: Unit> T::Quantity<U>;` (:1020), a composition
`public domain Socket::Usable = Socket::Connected & Socket::Authenticated;`
(:995), `requires` with `access` together (resources/access.md:20), and an
access policy with arguments (resources/access.md:83).

### trait

    trait       = visibility? 'boundary'? 'trait' word generic_params? ( ':' type )?
                  ( '{' requirement* '}' | '=' type '{' restriction* '}' )
    requirement = 'machine' operator_token? word generic_params? '(' params ')' result? clause* ';'
    restriction = 'machine' '*' clause* ';'

Ordinary and boundary traits (build/assumed_binding_contracts.md:50,
language/conformances.md:39), a supertrait
`boundary trait WallClock: ClockOps<NoState> {}` (language/conformances.md:338),
and a restricted alias
`public trait LocalLogger = Logger { machine * reaches none ...; }` (:615).

### conformance

    conformance = visibility? word generic_params? 'satisfies' type
                  hypothesis* '{' machine* '}'
    hypothesis  = 'requires' 'machine' word '(' params ')' result? clause* ';'

`public SaturatingIncrement satisfies Incrementable<Counter> { ... }`
(language/conformances.md:39), and with hypotheses
(proofs/quotients.md:196).

### machine

    machine = visibility? 'boundary'? 'asm'? 'machine' operator_token? path generic_params?
              ( '(' params ')' )? result? clause* ( body | ';' )
    operator_token = '+' | '>' | '==' | '[' ']' | '[' '..' ']' | ...
    params  = ( param % ',' )?
    param   = word ':' type
    result  = '->' type
    body    = '{' statement* expression? machine* '}'

- Qualified names `machine EchoSupplier::supply(...)`
  (build/assumed_binding_contracts.md:50).
- Operator tokens: `machine + add(...)` (language/expressions.md:151),
  `machine > Small::greater(...)` (language/domains.md:910),
  `machine [] read<Element [copy]>(...)` (language/expressions.md:405).
- A bodyless `boundary machine ...;` (build/boundary_shapes.md:52).
- Nested machines after the executable part of a body, and an `asm machine`
  with no parameter list (language/assembly.md:61). "Once declarations begin,
  executable statements cannot resume" (language/machines.md, "Nested machines
  and identity").
- "Executable bodies always use braces, never expression-body syntax"
  (language/machines.md, "Supply").

### mathematical let

    math_let = 'boundary'? 'let' word generic_params? '(' params ')' ':' type ( '=' expression )? ';'

`let greater_than(limit: i32, value: i32): core::Strict<0> = value > limit;`
(proofs/mathematical_bindings.md:63) and a bodyless `boundary let choose<...>(...): A;`
(:263).

## Generic parameters

    generic_params = '<' ( generic_param % ',' ) '>'
    generic_param  = word attributes?                 type parameter:    T, Element [copy]
                   | word ':' type                    kinded parameter:  Count: u32, u: core::Level
                   | 'const' word ':' type            static value:      const N: u64
                   | 'const' word ':' machine_type    static machine:    const F: machine(v: u32) -> u32
                   | 'const' word ':' 'machine'       inferred machine:  const F: machine
                   | 'machine' word                   machine binder:    machine Step
                   | 'some' word                      per-value binder:  some S
                   | lifetime                         'a

Shown at language/conformances.md:338 and :526, language/generics.md:223,
:636, :761 and :858, language/data_and_literals.md:109,
proofs/mathematical_bindings.md:63 and :226.

## Types

    type        = '&' lifetime? 'mut'? type
                | '[' type ']'
                | '[' type ';' expression ']'
                | machine_type
                | type '->' type
                | path
    type_in     = type ( 'in' type_domains )?
    type_domains = path ( '&' path )*
    machine_type = 'machine' generic_params? '(' params ')' result? clause*
    lifetime    = ''' word

References and views `&[u8]`, `&mut [u8]`, `&'a U`, `&'a mut S`
(language/data_and_literals.md:109, language/some_packages.md:15), arrays
`[u8; 38]` (language/expressions.md:526), machine types as field and
parameter types (language/effects.md:201), function types `A -> B` in
mathematical declarations (proofs/mathematical_bindings.md:63), and
qualification `i32 in Km & Positive` (language/domains.md:572). A parameter,
field, result and `let` may all carry `in`.

## Header clauses

    clause = requires | 'ensures' expression
           | 'reaches' ( 'none' | type )
           | 'crashes' ( 'none' | path )
           | 'blocks' 'false'?
           | 'suspends' 'false'?
           | 'terminates' ( 'by' expression ( '->' expression )? ( 'in' expression )? )?
           | 'invokes' path
           | 'satisfies' type
           | 'registers' word '=' expression
           | 'clobbers' 'none'
    requires = 'requires' expression

"Machine header clauses have no trailing semicolons ... The next clause
keyword or the implementation body delimits a clause; a newline does not
change its owner" (language/state_contracts.md, "Machine header delimiters").
Forms: language/effects.md:743, language/termination.md:35,
language/assembly.md:61 and :142, language/conformances.md:615.

## Statements

    statement = 'let' 'mut'? pattern ( ':' type_in )? ( '=' expression )? ';'
              | place '=' expression ';'
              | '_' '=' expression ';'
              | expression ';'
              | transition
              | 'if' expression body

- `let` with and without a type, with `mut`, and with no initializer
  `let mut bytes: [u8; 38];` (language/expressions.md:526).
- Destructuring `let Lease { pool, slot } = move value;`
  (language/ownership.md:309).
- Discard `_ = builder.tests.group<Tests::memory>();`
  (build/scoped_execution.md:302).
- The last expression of a body, with no semicolon, is its result.

## Transitions and match

    transition = 'transition' expression? '{' arm+ '}'
               | 'transition' '->' word ';'
    match      = 'match' expression '{' arm+ '}'
    arm        = pattern '->' expression

With a subject (language/patterns.md:22), without one
(language/state_contracts.md:440), and the assembly form `transition -> check;`
(language/assembly.md:61). Arms are written one per line with no separator
(language/patterns.md:82).

## Patterns

    pattern = '_' | literal | path
            | path '{' ( field_pattern % ',' )? '..'? '}'
            | '(' ( pattern % ',' ) ')'
    field_pattern = word ( 'as' ( word | '_' ) | ':' expression )?

"`field` binds the field, `field as name` renames it, `field as _` waives it,
and `field: value` is ordinary projected equality" (language/patterns.md,
"Pattern subjects and bindings"). Tuples at language/patterns.md:82.

## Expressions

    primary = literal | path | '(' expression ')' | '(' expression % ',' ')'
            | path '{' ( field_init % ',' )? '..'? '}'
            | '[' ( expression % ',' )? ']'
            | match
            | '(' selection ')'
    field_init = word ( ':' expression )?
    selection  = path 'satisfies' type
    postfix = primary ( '(' args ')' | '.' word | '[' index ']' | 'as' cast )*
    index   = expression | expression? '..' expression?
    cast    = type? cast_entry*
    cast_entry = ( '+' | '-' | '=' ) path
    prefix  = ( '&' 'mut'? | 'move' | 'suspend' | 'block' | '!' | '-' )* postfix

Binary operators, tightest first, from language/expressions.md ("Operator
declarations") where it gives them:

| Tier | Operators | Associativity |
| --- | --- | --- |
| Multiplicative | `*` `/` `%` and their `%` `\|` `!` policy forms | Left |
| Additive | `+` `-` and their policy forms | Left |

The spec gives no tier for shifts, comparisons, `in`, `&&`, `||` or ranges.
That is a gap; see below.

- Record literals with shorthand and explicit omission `..`
  (language/data_and_literals.md:172).
- Casts: "after `as`, an optional carrier type and then every following `=D`,
  `+D` or `-D` entry belong to the cast. A `+` or `-` directly after a cast
  therefore begins an entry, never an addition or subtraction"
  (language/expressions.md). Examples at language/domains.md:623 and :701.
- Membership `self in Value::Integer` (language/data_and_literals.md:272).
- Views `view[2..4]`, `bytes[..]` (language/expressions.md:499, :526).
- Call markers `suspend`, `block`, `suspend block` (language/effects.md:871).
- A call through a field `(operations.value)(&counter)`
  (language/machine_values.md:17).
- "Right shift keeps its two adjacent `>` tokens, and its suffix is the `%`,
  `|` or `!` immediately following the second `>`" (language/expressions.md).

## Assembly

    asm_block = 'asm' clause* '{' asm_line* '}'

Instructions are written `mnemonic operand, operand;`
(language/assembly.md:61, :142, :255). How much of an instruction this layer
understands is not settled here.

## Where the spec disagrees with itself

| # | What | Where |
| --- | --- | --- |
| S1 | A `requires` before a body is written three ways in one file: ending in `;`, ending in `,`, and with nothing. The prose says a semicolon before a body "does not terminate a clause and then resume the same declaration". | language/generics.md:223, :242, :363 against language/state_contracts.md, "Machine header delimiters" |
| S2 | `if` appears in one example and is defined nowhere. No `else`, `while`, `loop`, `return`, `for`, `break` or `continue` appears in the spec at all. | language/conformances.md:39 |
| S3 | Bodies written `{ ... }` in examples. The lexer reads `...` as `..` then `.`, so these are not source. | build/task_runtime.md:133, language/data_and_literals.md:20, language/generics.md:636 |
| S4 | Many examples are fragments: statements, clauses or expressions outside any declaration. They cannot be parsed as files. | for example language/generics.md:306, language/termination.md:35, language/reflection.md:77 |

Evidence for S2 from outside the spec: across the 301 `.omg` files of V1's own
`source` tree, with comments and strings removed, `if`, `else`, `while`,
`loop`, `return`, `for`, `break` and `continue` each occur zero times, while
`transition` occurs 5,993 times and `match` 24. V1's lexer reserves `if`,
`else`, `while`, `loop` and `return` all the same.

## What V1's parser does

The spec does not settle the questions below, so V1's parser was read for how
it handles each. This is evidence for a decision, not the decision. Paths are
under V1's `omega-rust/psi/compilation/pipeline/01_source-compilation/src/operations/parsing/`.

How this was established: a subagent read the parser (82 files) and reported
each rule with a file, line and snippet. I then checked the claims marked
*checked* against the code myself. The rest are as reported. An attempt to
confirm by running V1's compiler failed: its 2026-11-13 build refuses every
file, a one-line control included, with "no product route yet" before it
parses anything.

| # | Question | What V1 does |
| --- | --- | --- |
| Q1 | Generic arguments or less-than? | Tries generics first and falls back to comparison. On `<` it scans ahead for a `(` preceded by `>`, and accepts if the tokens between parse completely as an argument list. So `a < b > (c)` is the call `a<b>(c)`. (`expressions/postfix.rs:449`, `expressions/parse.rs:677`) |
| Q2 | What ends an arm? | Nothing but the end of the target expression. Newlines are ignored. A `transition` arm whose target starts with a name takes only a primary and its suffixes, no binary operators, so `-> value + 1` fails. `match` arms take a full expression and an optional comma. (`transitions/targets.rs:44`, `expressions/parse.rs:1472`) |
| Q3 | Subject followed by `{`? | `match` parses its subject in a mode where a path followed by `{` is not a record literal. `transition` cuts the subject at the first `{` outside brackets. Parentheses lift the restriction. (`expressions/parse.rs:970`, `transitions/blocks.rs:91`) |
| Q4 | Reserved words? | None, in effect. Every word is matched by spelling at the positions where it means something and is a legal name elsewhere, `self` excepted. Traps follow from that: a variable named `block` or `suspend` cannot be read unless called at once, since both are call prefixes. (`token_input/cursor.rs:374`, `:543`) |
| Q5 | Control flow? | `transition` and `match` only. The `if` statement is a hard error (*checked*, `statements/local_data.rs:37`: "the `if` statement is retired; dispatch is `transition <guard> { true -> ... _ -> ... }`"). `if` survives only as a guard on an arm. `else`, `while`, `loop`, `return` and `when` are lexer keywords the parser never looks at (*checked*: no use of those keyword kinds anywhere in the parser). |
| Q6 | Precedence? | Tightest first: postfix; unary `&` `&mut` `!` `~` `-` `move` `suspend` `block`; `as`; `* / %`; `+ -`; shifts; `&`; `^`; `\|`; `in`; `< <= > >=`; `== !=`; `&&`; `\|\|`. All left-associative. Bitwise operators bind tighter than comparisons, unlike C. (*checked*, `expressions/operators.rs:17` and `:38`) |
| Q7 | Clause order? | A fixed start (name, generics, parameters, result, `requires machine` hypotheses, `satisfies`), then the remaining clauses in any order until `{` or `;`. `requires`, `ensures`, `reaches`, `invokes` and `terminates` may repeat; `suspends` and `blocks` may not. (`declarations/machines.rs:22`, `contracts/clauses.rs:74`) |
| Q8 | Fragments? | Not applicable to V1. |
| Q9 | Assembly? | `asm` exists only as a statement inside a body. Instructions are `mnemonic operand, operand` with operands parsed as ordinary expressions, separated by `;`. (`assembly/parse.rs:52`, `:80`) |

Newlines matter in exactly one place in V1: inside the predicate list of a
`requires`, `ensures` or `crashes` clause, where a newline separates two
predicates (*checked*: `has_newline_before` is used only in
`contracts/facts.rs`).

## Where the spec and V1's parser disagree

Each of these is a spec example that V1's parser, as read, would refuse. None
could be confirmed by running V1. The spec is V2's reference, so the default
is to follow the spec; the last column says where that default looks wrong.

| # | Spec example | V1's parser | Note |
| --- | --- | --- | --- |
| D1 | An `if` statement (language/conformances.md:39) | Rejects it as retired. | The spec example looks stale: V1 retired `if` deliberately and V1's own source never uses it. |
| D2 | `requires ...;` then a body (language/generics.md:223) | Reads the `;` as ending a bodyless machine, then fails on `{`. A trailing `,` (:242) is accepted. | The spec's own prose sides with V1 against its example. |
| D3 | `match (a, b) { (0, _) -> ... }` (language/patterns.md:82) | `match` takes one subject and no tuple patterns. Tuples work only on `transition`. | The spec's prose says both forms "use the same pattern vocabulary". |
| D4 | An arm ending in a call, followed by an arm starting with `(` (language/patterns.md:82) | Glues them into one call, `reject()(...)`, and fails. | V1 has no test for this shape. |
| D5 | `asm machine name ... registers ecx = count { ... }` (language/assembly.md:61) | No such construct. `registers` is not a clause (*checked*: the word appears only in a message). | V1's `source` tree never uses `asm` either. |
| D6 | `(clear satisfies A::clear)` as an expression (language/conformances.md:197, :526) | `satisfies` is recognized only in declaration headers (*checked*). | |
| D7 | The `some S` binder (language/data_and_literals.md:109) | Not recognized. | |

### Rot, or the spec ahead of the code?

Both, and the dates in V1's git history say which. "Last changed" is when the
spec line showing the form was last edited.

| # | Spec line last changed | What V1's history says | Verdict |
| --- | --- | --- | --- |
| D1 `if` | 2026-10-27, by a change to the integer API it calls | V1 retired `if` on 2026-07-02. The commit reads "the `if` statement is retired ('if isn't a thing', settled 2026-07-02)". The spec never mentions it. | **Rot.** The example outlived the decision by four months and was even edited in place without anyone noticing the `if`. |
| D2 `requires ...;` | 2026-09-08 | The rule against it, `machine-clause-delimiters`, was confirmed 2026-10-30. | **Rot.** The example predates the rule and was not updated. |
| D2 trailing `,` | 2026-09-20 | Same rule, same date. | Probably rot, though V1 still accepts it. |
| D3 tuple `match` | 2026-11-10, under decision `record-or-sum-data` | V1's `match` has no tuple support. | **Spec ahead.** Written a week before the copy was taken; not implemented. |
| D4 arm then `(` | 2026-11-10, same example | V1 has no rule and no test for it. | **A hole in both.** Nobody has run this example. |
| D5 `asm machine` | 2026-10-29, under decision `checked-assembly-machine-bodies` | V1's parser has only the `asm` statement. | **Spec ahead.** Designed, not implemented. |
| D6 `satisfies` expression | 2026-11-10 | Not in V1's parser. | **Spec ahead.** |
| D7 `some S` | 2026-11-10 | Not in V1's parser. | **Spec ahead.** |

What this does to the decisions below:

- **Decisions 1 and 4 are already made**, by Zachary on 2026-07-02 and by the
  2026-10-30 rule. V2 follows them; the spec's examples are wrong, not V1.
- **Decisions 3 and 9 are the spec being newer than V1.** V2 follows the
  spec.
- **Decision 2 is new.** Neither the spec nor V1 has ever answered it.

The two rotted examples are worth reporting to whoever maintains the spec:
language/conformances.md:39 and language/generics.md:223.
## What V1 accepts that the spec never shows

These are in V1's parser and in none of the spec's 132 examples. They are not
in this grammar. Each needs a look at the spec's prose before it is added or
left out.

- Items: `proposition`, `capability`, `measure`, `operator`, `module` and
  `package` headers, `boundary data`, quotient data `data N = T % R`, and a
  target word before `machine`.
- Statements: `crash Trap;`, compound assignment `+=` and friends, and a
  proof-output binding `let (value; field: term) = call();`.
- Expressions: an evidence lane in calls `f(a, b; evidence)`, `&write` and
  `&state` borrows, `~`, `zero_value<T>()`, `utf16 "..."`.
- Clauses: `stack N bytes`, `reaches A + B`, `reaches <= Bound`, labelled
  predicates `requires name: pred`.
- Optional semicolons after data members.

## Decisions this grammar needs

Each row is a rule to settle before fixtures are written. "Leaning" is
Claude's recommendation and nothing more.

| # | Decision | Options | Leaning |
| --- | --- | --- | --- |
| 1 | Is there an `if` statement? | Follow the spec's one example, or follow V1 and the spec's silence everywhere else. | No `if` statement; `if` only as an arm guard. Treat language/conformances.md:39 as a stale example. |
| 2 | What ends an arm? | (a) A newline or comma ends an arm. (b) Newlines never matter and the parser looks ahead for `->`. (c) V1's rule, which cannot parse the spec's own example. | (a). Every example writes one arm per line, and it makes D4 parse. It does make arms the second place newlines matter. |
| 3 | Do `match` and `transition` share tuple subjects and patterns? | Spec says yes; V1 only does it for `transition`. | Yes, as the spec says. |
| 4 | Punctuation after a `requires` before a body? | Nothing, a comma, or a semicolon. | Nothing, per the spec's prose. Refuse `;`. Accept a trailing comma only if you want it. |
| 5 | Which words are reserved? | None, as V1; or a short fixed list. | None, as V1, with `self` special. It matches the spec letting `u8` be a parameter name. |
| 6 | Operator precedence? | V1's table, or something else. | V1's table. The spec gives two tiers and both agree with it. |
| 7 | Generic arguments versus less-than? | V1's try-then-fall-back rule, or a new explicit form. | V1's rule, stated as: `<` starts arguments when they parse completely and are followed by `(`, `::` or a closing delimiter. |
| 8 | Is assembly in the first parser? | Parse `asm` now, or leave it for its own step. | Leave it out of the first pass. The spec and V1 disagree on its whole shape (D5). |
| 9 | Are `(m satisfies T::r)` and `some S` in? | The spec shows both; V1 has neither. | In, as the spec shows. |
| 10 | How is the spec corpus used for testing? | Tag each of the 132 examples as a file, statements, clauses or an expression, and mark the ones that are placeholders or deliberately rejected. | Do that tagging as the parser's first fixture work. |