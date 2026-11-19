# L1 Parser grammar

Status: **gate passed and signed off 2026-11-18.** All twelve decisions are
made. The parser in `src/` follows
this grammar, every rule below has fixtures (see [Rules and their
fixtures](#rules-and-their-fixtures)), and all 132 spec examples parse.
Building it changed the grammar in seven places and raised two questions;
both are listed under [What building it found](#what-building-it-found).

The spec has no grammar, so this document is one. Its sources, in order of
authority:

1. **Decisions by Jarod and Zac**, recorded under [Decisions](#decisions).
   The syntax changes from the imported spec are listed under
   [How the spec's examples change](#how-the-specs-examples-change).
2. **The spec's prose**, where it states a rule of form.
3. **The spec's 132 `omega` examples**, cited as `file:line` under
   `wiki/spec/`.
4. **Omega V1's parser**, only where the above are silent, and marked as such.

The parser's input is the lexer's tokens. No word is reserved: every word
below, `machine` included, arrives as a plain word and means something only
in the position this grammar gives it. A name such as `block` or `result` is
legal wherever a name is expected. The exact tests are under [When a word is
a keyword](#when-a-word-is-a-keyword).

The parser never tries one reading and falls back to another. Every choice
is made by looking ahead a bounded distance and committing. Where that costs
the language a corner, the corner is stated at the rule.

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
    item        = use | const | data | domain | trait | conformance | machine
                | asm_machine | math_let
    visibility  = 'public'

A file's namespace comes from its directory; there is no `module` header
(`language/modules.md`, "Directory namespaces").

## Names, and the two spellings of generic arguments

    path       = word ( '::' word )*
    type_args  = '<' ( generic_arg % ',' ) '>'            in types and declarations
    expr_args  = '::' '<' ( generic_arg % ',' ) '>'       in expressions
    generic_arg = lifetime | type | expression

In a declaration or a type, `<` after a name can only open generic arguments,
so it is written bare: `Outcome<u32, Never>`, `machine sort<T>(...)`. That
holds after `as` as well: `x as u32 < y` is refused and is written
`(x as u32) < y`.

**Type or value?** A generic argument is a type (`Outcome<u32, Never>`) or a
value (`ByteBuffer<64>`, `Buffer<N * 2>`), and the two can start alike. The
parser decides by looking ahead:

1. Skip any `[` the argument opens with.
2. `@` or `machine(` there begins a type.
3. A name there, with any `::name` parts, begins a type if the token after
   it is `<` or `->`, or ends the argument: `,` or `>` at the top, `]` or `;`
   inside brackets. So `[u8]` and `[u8; 4]` are types, and `[a, b]` and
   `[f(x)]` are array literals.
4. Anything else is an expression. In it, `>` ends the argument, so a
   greater-than comparison there is written in parentheses.

`Buffer<N>` therefore holds a type-shaped `N` that may turn out to name a
value; the resolver tells which. Two shapes land on the wrong side and are
written with parentheses: a value that begins with a generic type,
`Buffer<(Other::<T>::SIZE + 1)>`, and an array literal of one bare name,
`Routes<[(open_file)]>`.

In an expression, generic arguments are written with `::` first, and a bare
`<` after a name is always less-than (decision 7). The parser never looks
ahead to tell them apart.

| The spec writes | V2 writes |
| --- | --- |
| `apply<set_time>(&mut e, 5)` | `apply::<set_time>(@mut e, 5)` |
| `integer::max<i32>()` | `integer::max::<i32>()` |
| `Sensor<29u64>::read()` | `Sensor::<29u64>::read()` |
| `builder.tests.group<Tests::memory>()` | `builder.tests.group::<Tests::memory>()` |
| `Envelope<u32>::Body::Value { value: 7 }` | `Envelope::<u32>::Body::Value { value: 7 }` |
| `i < n` | `i < n`, unchanged |

## Items

### use

    use = 'use' path ';'

`use topology;` (build/scoped_execution.md:134),
`use ui_policy::screen::Point::OnScreen;` (language/modules.md:102).

### const

    const = visibility? 'const' path ( ':' type )? '=' expression ';'

Typed, and with a qualified name `EfiStatus::SUCCESS` (language/constants.md:6);
untyped `const clear_a = (clear satisfies A::clear);`
(language/conformances.md:526).

### data

    data        = visibility? 'data' word generic_params? attributes? dropped_by?
                  ( '=' type ';' | requires* '{' data_member* '}' )
    data_member = stable_number? word attributes? ':' ( type_in | data ) ';'
                | stable_number? 'case' word ( '(' ( case_field % ',' )? ')' )? ';'
                | data
    case_field  = stable_number? word ':' type_in
    stable_number = '#' integer
    attributes  = '[' ( attribute % ',' ) ']'
    attribute   = word ( '(' ( word ':' word ) % ',' ')' )?
    dropped_by  = 'dropped' 'by' path type_args?

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

    domain = visibility? 'domain' generic_params? path type_args?
             ( '=' type_domains ';'
             | requires? ( 'access' type )? dropped_by? ';' )

`domain i32::Km;`, `domain i32::Positive requires self > 0;`,
`public domain Reservation::Issued access ReservationAccess;`
(language/domains.md:131), a nested domain `i32::Positive::Small` (:666),
generic forms `domain<T> T::Metres;` (:701) and
`domain<T, const U: Unit> T::Quantity<U>;` (:1020), a composition
`public domain Socket::Usable = Socket::Connected & Socket::Authenticated;`
(:995), `requires` with `access` (resources/access.md:20), and an access
policy with arguments (resources/access.md:83).

`self` in a `requires` here or on a `data` declaration names the value being
judged. It has no meaning anywhere else and is an ordinary word there.

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
                  hypothesis* '{' ( machine | selection_row )* '}'
    selection_row = word '::' word '=' path ';'

A selection row names a trait requirement and an existing machine to supply
it. Both paths retain their own source spans. [Named conformances](../../wiki/spec/language/conformances.md#declaration-and-selection)

`public SaturatingIncrement satisfies Incrementable<Counter> { ... }`
(language/conformances.md:39), and with hypotheses (proofs/quotients.md:196).
A hypothesis is defined under [Header clauses](#header-clauses); it is the
only clause a conformance takes. `public satisfies T { }` declares a
conformance for a type named `public`.

### machine

    machine = visibility? 'boundary'? 'machine' operator_token? path generic_params?
              '(' params ')' result? clause* ( body | ';' )
    operator_token = '+' | '-' | '*' | '/' | '%' | '<' | '>' | '<=' | '>=' | '==' | '!='
                   | '[' ']' | '[' '..' ']'
    params  = ( param % ',' )?
    param   = word ':' type_in
    result  = '->' type_in
    body    = '{' statement* tail? machine* '}'
    tail    = expression | transition

- Qualified names such as `boundary machine NativeConsole::write(...)`
  (build/boundary_shapes.md:52).
- Operator tokens: `machine + add(...)` (language/expressions.md:151),
  `machine > Small::greater(...)` (language/domains.md:910),
  `machine [] read<Element [copy]>(...)` (language/expressions.md:405). The
  spec names `+`, `==`, `[]` and `[..]` and forbids binding suffixed
  integer policy operators. V1's `OperatorSpelling::ALL` supplies the full
  13-spelling list above. [Declaration-head tests](tests/operator_grouping.rs)
  check both machine and trait requirement heads against that list.
- A bodyless `boundary machine ...;` (build/boundary_shapes.md:52).
- Nested machines come after the executable part of a body: "Once
  declarations begin, executable statements cannot resume"
  (language/machines.md, "Nested machines and identity").
- "Executable bodies always use braces, never expression-body syntax"
  (language/machines.md, "Supply").

### mathematical let

    math_let = 'boundary'? 'let' word generic_params? '(' params ')' ':' type ( '=' expression )? ';'

`let greater_than(limit: i32, value: i32): core::Strict<0> = value > limit;`
(proofs/mathematical_bindings.md:63) and a bodyless
`boundary let choose<...>(...): A;` (:263). A `let` at the top of a file is
this form; inside a body it is a statement.

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

    type        = type_operand ( '->' type )?
                | '(' word ':' type ')' '->' type
    type_operand = '@' lifetime? ( 'mut' | 'write' )? type_operand
                | '[' type ']'
                | '[' type ';' expression ']'
                | machine_type
                | path_type ( '(' arguments? ')' )?
    path_type   = word type_args? ( '::' word type_args? )*
    type_in     = type ( 'in' type_domains )?
    type_domains = path_type ( '&' path_type )*
    machine_type = 'machine' generic_params? '(' params ')' result? clause*
    lifetime    = ''' word

References and views `@[u8]`, `@mut [u8]`, `@'a U`, `@'a mut S`
(language/data_and_literals.md:109, language/some_packages.md:15), arrays
`[u8; 38]` (language/expressions.md:526), machine types as field and
parameter types (language/effects.md:201), function types `A -> B` in
mathematical declarations and in machine parameters
(proofs/mathematical_bindings.md:63 and :226), and qualification
`i32 in Km & Positive` (language/domains.md:572).

- In a dependent function type such as `(x: A) -> F(x)`, the result can refer to the input by name. A type family takes ordinary expressions as arguments. Both forms appear in mathematical interfaces. [Mathematical bindings](../../wiki/spec/proofs/mathematical_bindings.md)
- **The function arrow** groups to the right, and `@A -> B` is a function
  from `@A`. It is a type everywhere except directly after `as`: in an arm
  written `x as T -> ...` the arrow belongs to the arm.
- **`machine` alone is a name.** It is a machine type when `(` or `<`
  follows it, which is how `const F: machine` (an inferred machine) parses.
- **A machine type inside `<...>` ends at the closing `>`**, even in the
  middle of one of its clauses: in
  `<const F: machine(v: u64) -> u64 ensures result >= v>` the last `>` closes
  the list (language/effects.md:571). A greater-than comparison in such a
  clause is written in parentheses.
- **Each `@` adds one reference layer.** `@@T` and `@ @T` have the same tree.
  In expressions, `@@x` borrows a borrow. Decision 12 reserves prefix `@`
  for references and borrows. `&` and `&&` keep their operator meanings.

## Header clauses

    clause = requires | hypothesis | 'ensures' expression
           | 'reaches' type
           | 'crashes' type expression?
           | 'blocks' ( 'true' | 'false' )?
           | 'suspends' ( 'true' | 'false' )?
           | 'terminates' ( 'by' expression ( '->' expression )? ( 'in' bound )? )?
           | 'invokes' type
           | 'satisfies' type
           | 'registers' word '=' expression
           | 'clobbers' word ( ',' word )*
    requires   = 'requires' expression
    hypothesis = 'requires' 'machine' word '(' params ')' result? clause* ';'
    bound      = expression ( ( '..' | '..=' ) expression )?

- A crash cause can be followed by a guard, as in `crashes Trap value == 0`. L3 checks that the guard is a total Boolean fact about the contract inputs. [Guarded crashes](../../wiki/spec/language/effects.md#guarded-crashes)
- `none` after `reaches`, `crashes` and `clobbers` is an ordinary name to the
  parser; what it means is a later layer's question.
- **A hypothesis is the one clause that ends in a semicolon.** It assumes a
  machine with that signature exists, and it carries clauses of its own;
  without the `;` there would be no telling where they stop and the enclosing
  header's clauses resume. It appears on machines
  (proofs/mathematical_bindings.md:226) and on conformances
  (proofs/quotients.md:196).
- The bound of `terminates by ... in` may be a range, `in 0..=limit`
  (language/termination.md:35). A range is not an expression anywhere else
  except inside index brackets.

- **Nothing separates a clause from what follows** (decision 4). A clause
  ends at the next clause word or at the `{`. A `;` or `,` between a clause
  and a body is refused. "Machine header clauses have no trailing semicolons
  ... The next clause keyword or the implementation body delimits a clause; a
  newline does not change its owner" (language/state_contracts.md, "Machine
  header delimiters").
- **This costs the parser one token of lookahead, and a comma would not save
  it.** A clause's predicate is an expression, and an expression stops by
  itself when the next token cannot continue it. A clause word after a
  complete expression cannot continue it, so the parser is then looking at a
  single token and asks whether it is a clause word, `{` or `;`. With a comma
  after each clause the parser would still have to read the next word to know
  which clause follows.
- **Words are whole tokens.** `requires ensures_stupid()` is a predicate that
  calls `ensures_stupid`; the lexer hands over the word `ensures_stupid`, and
  it is not the word `ensures`.
- **A clause word in operand position is a name.** In `requires a < reaches`
  the parser needs an operand after `<`, so `reaches` is a name there. It is
  a clause word only where an operand cannot go.
- A bodyless declaration ends with one `;` after its last clause.
- Clauses may come in any order after the result, and `requires` and
  `ensures` may repeat (V1's rule; the spec's examples vary the order and
  repeat `requires`, proofs/contracts.md:826).
- Forms: language/effects.md:743, language/termination.md:35,
  language/assembly.md:61 and :142, language/conformances.md:615.

## Statements

    statement = 'let' 'mut'? pattern ( ':' type_in )? ( '=' expression )? ';'
              | place '=' expression ';'
              | '_' '=' expression ';'
              | expression ';'
              | asm_block

- **There is no `if` statement** (decision 1), and no `else`, `while`,
  `loop`, `return`, `for`, `break` or `continue`. Branching is `match` and
  `transition`; repetition is nested machines and transitions.
- `let` with and without a type, with `mut`, and with no initializer
  `let mut bytes: [u8; 38];` (language/expressions.md:526).
- Destructuring `let Lease { pool, slot } = move value;`
  (language/ownership.md:309).
- Discard `_ = builder.tests.group::<Tests::memory>();`
  (build/scoped_execution.md:302, in V2's spelling).
- The last expression of a body, with no semicolon, is its result. Nothing
  executable follows it, and nothing executable follows a `transition`.
- A `match` used as a statement ends with `;` like any other expression.
- **`if` gets its own error.** `if` is not reserved, so `if(ready);` is a
  call to a machine named `if` and parses. A statement that begins with the
  word `if` and then fails to parse is reported as "Omega has no `if`
  statement", at the `if`.

## match and transition

    match      = 'match' subject? '{' arms '}'
    transition = 'transition' subject? '{' arms '}'
    subject    = expression                      with no record literal at its top level
    arms       = arm % ','
    arm        = pattern '->' expression

- **`match` and `transition` have one grammar** (decision 3). They differ in
  position only: `transition` is the form at the end of a body, where the
  chosen arm's result is the machine's result and nothing follows it. Both
  take a tuple subject and tuple patterns (language/patterns.md:82).
- **Arms are separated by commas**, with an optional comma after the last
  (decision 2). Newlines never matter. An arm has no marker of its own to say
  where the next one starts, which is what a clause gets from the next clause
  word; the comma is that marker.
- With no subject, each pattern is a condition or `_`
  (language/state_contracts.md:440).
- A subject cannot be a record literal unless it is in parentheses, since
  `match x {` would otherwise read `x { ... }` as one. This is V1's rule; the
  spec is silent.

| The spec writes | V2 writes |
| --- | --- |
| `Outcome::Ok { value } -> value` on one line, the next arm on the next | the same, with `,` after each arm |

## Patterns

    pattern = '_'
            | '(' ( pattern % ',' )? ')'
            | path '{' ( field_pattern % ',' )? '..'? '}'
            | expression
    field_pattern = word ( 'as' ( word | '_' ) | ':' expression )?

"`field` binds the field, `field as name` renames it, `field as _` waives it,
and `field: value` is ordinary projected equality" (language/patterns.md,
"Pattern subjects and bindings").

- **A pattern may be any expression.** With no subject an arm's pattern is a
  condition, and with one it may be a literal, a constant or a computed
  value. An expression pattern cannot hold a record literal at its top level,
  since `Name {` there begins a record pattern.
- **A pattern that is one bare word** is recorded as a binding. Whether the
  word binds a new name or names a constant or a case is a question about
  what it refers to, which the resolver answers.
- **Tuple pattern or parenthesized condition?** Both start with `(`. The
  parser finds the matching `)` and looks at the one token after it: `->`,
  `,`, `)`, `=`, `:` or `;` ends a pattern, so the parentheses were a pattern;
  anything else continues an expression, so they were part of a condition.
  `(a, b) -> ...` is a tuple pattern and `(a + b) > c -> ...` is a condition.
- One pattern in parentheses with no comma is that pattern, grouped. `()`,
  `(a,)` and `(a, b)` are tuples.
- `..` stands for every field not written, so it comes last, in a record
  pattern and in a record literal.

## Expressions

    primary = literal | expr_path | '(' ')' | '(' expression ')' | '(' expression % ',' ')'
            | expr_path '{' ( field_init % ',' )? '..'? '}'
            | '[' ( expression % ',' )? ']'
            | match
            | '(' selection ')'
    expr_path  = word expr_args? ( '::' word expr_args? )*
    field_init = word ( ':' expression )?
    selection  = path 'satisfies' type
    postfix = primary ( '(' args ')' | '.' word expr_args? | '[' index ']' )*
    index   = expression | expression? ( '..' | '..=' ) expression?
    prefix  = ( '@' ( 'mut' | 'write' )? | '!' | '~' | '-' | 'move' | 'suspend' | 'block' )* postfix
    cast    = prefix ( 'as' type? cast_entry* )*
    cast_entry = ( '+' | '-' | '=' ) path_type

A cast binds tighter than every binary operator and looser than a prefix, so
`-a as i64` casts `-a` and `a as u64 * 2` multiplies the cast.

Binary operators, tightest first, all left-associative (decision 6; this is
V1's table, checked in `expressions/operators.rs`, and it agrees with the two
tiers language/expressions.md gives):

| Tier | Operators |
| --- | --- |
| 1 | `*` `/` `%`, and `*%` `*\|` `*!` `/%` `/\|` `/!` `%%` `%\|` `%!` |
| 2 | `+` `-`, and `+%` `+\|` `+!` `-%` `-\|` `-!` |
| 3 | `<<` and `<<%` `<<\|` `<<!`; `>>` and its policy forms |
| 4 | `&` |
| 5 | `^` |
| 6 | `\|` |
| 7 | `in` |
| 8 | `<` `<=` `>` `>=` |
| 9 | `==` `!=` |
| 10 | `&&` |
| 11 | `\|\|` |

Bitwise operators bind tighter than comparisons, unlike C.

- **Comparisons do not chain** (decision 11). The six comparison operators,
  `==` `!=` `<` `<=` `>` `>=`, do not take a comparison as an operand unless
  it is in parentheses. `a < b < c`, `a == b == c` and `a < b == c` are
  refused; `(a < b) == c` and `a < b && b < c` are fine. A range is written
  as two comparisons joined by `&&`. Every other operator still groups to
  the left: `a - b - c`, `a || b || c`.

- **Right shift** is two `>` tokens with nothing between them, and its policy
  suffix is a `%`, `|` or `!` directly after the second. The lexer has no `>>`
  token; this layer joins them by checking that their byte ranges touch
  (language/expressions.md: "Right shift keeps its two adjacent `>` tokens").
- **Casts:** "after `as`, an optional carrier type and then every following
  `=D`, `+D` or `-D` entry belong to the cast. A `+` or `-` directly after a
  cast therefore begins an entry, never an addition or subtraction"
  (language/expressions.md). Examples at language/domains.md:623 and :701.
- **Call markers** `suspend`, `block`, `suspend block` come before a call
  (language/effects.md:871). `move`, `suspend` and `block` are prefixes only
  directly before another word, so each is still a name everywhere else:
  `block(x)` calls a machine named `block`.
- **`mut` and `write` after `@`** mark access only if an operand follows. `@mut;`
  borrows a name spelled `mut`.
- **A record literal needs parentheses** where a `{` after a name means
  something else: in the subject of a `match` or `transition`, in a clause's
  predicate, and at the top of a pattern.
- A field is reached by name. There is no `pair.0`.
- Record literals with shorthand and explicit omission `..`
  (language/data_and_literals.md:172).
- Membership `self in Value::Integer` (language/data_and_literals.md:272).
- Views `view[2..4]`, `bytes[..]` (language/expressions.md:499, :526).
- A call through a field `(operations.value)(@counter)`
  (language/machine_values.md:17).
- A selection `(clear satisfies A::clear)` is an expression and a generic
  argument (decision 9; language/conformances.md:197, :526).

## Assembly

Parsed in this layer, as the spec defines it (decision 8).

    asm_machine = visibility? 'asm' 'machine' path generic_params? ( '(' params ')' )?
                  result? clause* '{' asm_line* asm_machine* '}'
    asm_block   = 'asm' clause* '{' asm_line* '}'
    asm_line    = instruction ';' | 'transition' '->' word ';'
    instruction = word ( '.' word )? ( expression % ',' )?

- "`asm machine` declares an assembly-bodied machine. `asm { ... }` embeds a
  checked assembly region in ordinary Omega code" (language/assembly.md).
- "Every assembly-bodied declaration spells `asm machine`, including a
  declaration nested inside another assembly machine."
- "Instructions use `;` separators." The last one before `}` may omit it
  (language/assembly.md:142, :255).
- `transition -> next;` names the adjacent block (language/assembly.md:61).
- Operands are parsed as ordinary expressions. Whether a mnemonic exists and
  takes those operands is not this layer's question.
- `asm machine` at the top of a file is an item; nested, it follows the
  instructions of its parent. A plain `machine` inside an assembly machine is
  refused with a message that says to write `asm machine`.
- `asm` begins a block when `{` or a clause word follows it; otherwise it is
  a name.

## When a word is a keyword

No word is reserved (decision 5), so each word that the grammar gives a
meaning has a test for when it has that meaning. Everywhere the test fails,
the word is an ordinary name.

| Word | Is a keyword when | So this is still a name |
| --- | --- | --- |
| `use`, `const`, `data`, `trait` | a declaration can start here and a name follows | `data = 3;` |
| `domain` | a declaration can start here and a name or `<` follows | `domain.size` |
| `machine` | a declaration can start here and a name, or an operator and a name, follows | `machine.run();` |
| `machine` (in a type) | `(` or `<` follows | `const F: machine` |
| `asm` | `machine` and a name follow; or, in a body, `{` or a clause word follows | `asm = public;` |
| `let` (top of file) | a name and then `(` or `<` follow | |
| `let` (in a body) | a name or `(` follows | `let = 3;` |
| `public` | a declaration follows | `public satisfies T { }` |
| `boundary` | `trait`, `machine` or `let` follows, as a declaration | |
| `match`, `transition` | `{` follows, or a token that can start a subject | `match = match.field;` |
| `mut` | after `@` or `let`, and an operand or pattern follows | `let mut = move;` |
| `write` | after `@` when a type or operand follows | `@write;` borrows the name `write` |
| `move`, `suspend`, `block` | directly before another word | `block(use);` |
| `case` (in data) | a name follows | `case: u8;` |
| `const`, `machine`, `some` (generic parameter) | a name follows | `<const, machine, some>` |
| `as`, `in`, `by`, `satisfies`, `access`, `dropped by` | only in the position its rule names | |
| clause words | where a clause can start, which is after a complete header part | `requires a < reaches` |
| `self`, `none`, `true`, `false` | never; to the parser they are names | |
| `_` | at the start of a pattern, and before `=` at the start of a statement | |

Two words in a row are never a legal expression, which is why "a name
follows" is a safe test for a declaration.

The cost is in one place. A machine named `match` or `transition` cannot be
called as `match(x)`, because `(x)` could be a subject.

## Limits

- **Nesting is followed 128 levels deep.** Brackets, types, patterns, bodies
  and nested declarations each count. Past that the file is refused. The spec
  states no limit; this one exists so that a hostile file cannot run the
  parser out of stack, and it is far beyond anything a person writes. A long
  chain such as `a + a + ... + a` or `f()()()` is not nesting and has no
  limit.

## Fragments

The compiler parses files. The parser has a second entry point,
`parse_fragment`, that exists for one reason: most of the spec's examples
are not files. It accepts three shapes:

- **Statements**: declarations and statements in any order. An expression
  with no `;` is accepted wherever it ends, because the spec lists bare
  expressions one per line to show forms.
- **Clauses**: header clauses with no header, with an optional final `;`.
- **GenericParam**: one generic parameter with no list around it.

Four examples are parsed as one of the narrower shapes; they are named in
`tests/spec_corpus.rs`. Every fixture is a whole file and goes through
`parse_file`.

## How the spec's examples change

V2's spec corpus incorporates four syntax decisions from the parser review.

| Decision | The spec's examples | V2 |
| --- | --- | --- |
| 2 | Arms on separate lines, no separator | A comma after each arm |
| 7 | `name<T>(x)` in expressions | `name::<T>(x)` |
| 1 | One `if` statement (language/conformances.md:39) | Rewritten as a `transition` |
| 12 | Prefix `&` for references and borrows | Prefix `@` in types and expressions. Each adjacent `@` adds one layer. |
| 4 | `requires ...;` and `requires ...,` before a body (language/generics.md:223, :242) | Nothing before the body |

Also not source, and tagged as such in the corpus: bodies written `{ ... }`
(build/task_runtime.md:133, language/data_and_literals.md:20,
language/generics.md:636), and the many examples that are fragments
(statements, clauses or expressions outside any declaration).

## Rules and their fixtures

Every fixture is a whole file under `fixtures/`, parsed with `parse_file`.
An accept fixture's `.expected` is its tree; a reject fixture's is the one
error it must produce. 23 accept, 102 reject.

| Rule | Accept fixture | Reject fixtures |
| --- | --- | --- |
| file, items | `empty`, `comments_only` | `top_level_expression`, `top_level_let_statement`, `stray_closing_brace`, `boundary_data` |
| use, const | `use_and_const` | `use_with_glob`, `use_without_semicolon`, `public_use`, `const_without_value` |
| data | `data` | `data_without_body`, `data_member_without_semicolon`, `data_case_without_name`, `stable_number_is_not_an_integer`, `attribute_list_unclosed`, `attribute_setting_without_value`, `dropped_without_by`, `semicolon_before_data_body` |
| domain | `domains` | `domain_with_body`, `domain_composition_with_comma` |
| trait, conformance | `traits_and_conformances` | `trait_requirement_with_body`, `trait_with_field`, `restriction_without_star`, `conformance_with_statement`, `conformance_without_body`, `hypothesis_without_semicolon` |
| machine | `machines` | `missing_machine_params`, `missing_param_type`, `missing_result_type`, `machine_without_body_or_semicolon`, `operator_machine_with_unknown_operator`, `unclosed_params`, `unclosed_body` |
| mathematical let | `math_let` | `math_let_without_type`, `public_math_let` |
| generic parameters | `generic_params` | `generic_params_empty`, `generic_params_unclosed`, `const_param_without_type` |
| generic arguments | `generic_arguments` | `generic_args_empty`, `generic_args_unclosed`, `generic_arg_value_starting_with_type`, `generic_arg_greater_than`, `turbofish_in_type` |
| types | `types`, `reference_to_reference`, `job_queue` | `array_type_without_length`, `slice_type_unclosed`, `type_missing_after_reference`, `qualified_type_without_domain`, `logical_and_as_type`, `ampersand_reference_type` |
| header clauses | `clauses` | `semicolon_before_body`, `comma_before_body`, `clause_without_predicate`, `terminates_by_without_measure`, `reaches_without_type`, `registers_without_value`, `clobbers_without_name`, `record_literal_in_clause` |
| statements | `statements` | `if_statement`, `if_statement_parenthesized`, `return_statement`, `compound_assignment`, `crash_statement`, `empty_statement`, `let_with_literal_pattern`, `let_without_semicolon`, `statement_after_tail`, `statement_after_nested_machine`, `transition_not_last` |
| match, transition | `match_and_transition` | `missing_arm_comma`, `missing_arm_comma_in_transition`, `fat_arrow_arm`, `arm_without_result`, `arm_result_is_block`, `match_without_arms`, `record_literal_as_subject`, `transition_as_expression` |
| patterns | `patterns` | `record_pattern_rest_not_last`, `field_pattern_as_without_name` |
| binary operators | `expressions_binary` | `missing_operand`, `two_operators`, `chained_relational_comparison`, `chained_equality_comparison`, `comparison_of_a_comparison`, `comparison_as_right_operand`, `range_written_as_mathematics` |
| right shift | `right_shift` | `right_shift_with_space` |
| prefixes, postfixes, primaries | `expressions_unary_postfix`, `reference_to_reference` | `logical_and_as_borrow`, `ampersand_borrow`, `unary_plus`, `unclosed_call`, `unclosed_index`, `unclosed_parenthesis`, `field_access_by_number`, `path_ending_in_colons`, `record_literal_missing_comma`, `record_literal_omission_not_last`, `selection_without_trait` |
| casts | `casts` | `cast_then_arithmetic`, `cast_then_less_than` |
| assembly | `assembly` | `asm_instructions_without_separator`, `asm_transition_without_target`, `asm_machine_holding_a_machine`, `asm_block_unclosed` |
| no reserved words | `words_are_not_reserved` | covered by the rows above: each keyword test has a fixture on each side |
| nesting limit | 128 nested data declarations parse on a 1 MB stack | 129 nested data declarations, `nesting_too_deep`, and 17 constructs at 100,000 levels in `tests/hostile_inputs.rs` |

Beyond the fixtures, `tests/hostile_inputs.rs` checks that the parser refuses
without crashing or hanging: every recursive rule nested 100,000 deep on a
1 MB stack, five inputs that make a try-and-rewind parser take exponential
time, 30,000 random sources built from the grammar's own vocabulary, and
every accept fixture with each of its tokens removed in turn (3,515 inputs).

For accepted fixtures, spec fragments, random sources and token-deletion cases,
the invariant checks for one root, finished nodes, ordered tokens and brackets
that close inside the node that opened them. It then prints the leaves,
reducing each gap to one space. Lexing that text must recover every meaningful
token's spelling and kind, including the radix and suffix boundary of a number.

`tests/operator_grouping.rs` checks the tree grouping for all 1,600 ordered
pairs of the 40 binary operator spellings. For the 36 comparison pairs it
checks the diagnostic on the second operator and both explicit groupings.

`tests/reference_syntax.rs` checks the tree for every pair of shared,
mutable and write-only layers in types and expressions. Each pair is tried
with touching sigils, a space, a comment and a line break. Malformed and
retired prefixes have explicit diagnostic spans, including end of input.
The `reference_to_reference` fixture also checks adjacent AND and borrow
tokens and distinguishes a reference type argument from a borrowed value
argument written in parentheses.

`tests/psi_parser_cases.rs` adapts cases from V1's
`tests/psi/parser/test_parser.py` at `0b90bdbaaa6`. It checks 27 malformed
files against exact byte ranges and eight valid files against V2's rules.
Four of those eight exercise deliberate V2 verdicts, including a trailing
parameter comma and the name `self`. The tests run without a V1 checkout.
`tests/spec_corpus.rs` also checks the three fragment entry points, including
their diagnostics for trailing input.

## What building it found

The parser was written from this document and then run over the spec's 132
examples. 125 parsed on the first run. The other seven each showed something
the grammar had missed; all 132 parse now.

| # | Example | What the grammar had | What the example needs | Resolution |
| --- | --- | --- | --- | --- |
| F1 | language/effects.md:571 | A machine type's clauses were ordinary predicates. | `<const F: machine(v: u64) -> u64 ensures result >= value>`: the clause ends at the `>` that closes the list. | Inside `<...>`, a `>` ends a machine type's clause. |
| F2 | proofs/mathematical_bindings.md:226 | Function types only in mathematical `let`. | A machine parameter typed `A -> core::Strict<v>`. | `A -> B` is a type everywhere except directly after `as`. |
| F3 | proofs/mathematical_bindings.md:226 | A hypothesis only on a conformance. | `requires machine Step(...) ...;` on a machine. | A hypothesis is a clause; a conformance takes only that one. |
| F4 | language/termination.md:35 | `terminates by e in e`. | `in 0..=limit`. | The bound may be a range. |
| F5 | resources/access.md:83 | A generic argument starting `[` was a type. | `IssuedByAny<2, [reflect::callable(a), ...]>`, an array literal. | The lookahead rule under [generic arguments](#names-and-the-two-spellings-of-generic-arguments). |
| F6 | language/patterns.md:82, language/reflection.md:77 | Fragments were statements. | Bare expressions listed one per line. | The Statements fragment accepts an expression wherever it ends. Files are unaffected. |
| F7 | resources/content_custody.md:129 | A clause fragment ended at its last clause. | A final `;`, as on a bodyless declaration. | The Clauses fragment accepts one. |

Two things came up that were questions for the owners, not fixes. Both are
answered, and are owner questions 5 and 6 in PLAN.md.

- **Comparisons chained.** Every operator grouped to the left, so
  `0 <= index < length` parsed, as `(0 <= index) < length`: a true-or-false
  answer compared with a number. Decided 2026-11-18 (decision 11): a
  comparison is not the operand of another without parentheses. None of the
  132 examples was affected.
- **Reference syntax.** Owner question 6 originally retained prefix `&` and
  required spacing for two layers. Decision 12 supersedes that spelling.

## Decisions

Made by Jarod on 2026-11-18, with Zac on two of them.

| # | Question | Evidence | What was said | Decision |
| --- | --- | --- | --- | --- |
| 1 | Is there an `if` statement? | V1 retired it 2026-07-02 ("'if isn't a thing'"); the spec's one example is rot. | "obviously not" | **No.** |
| 2 | What ends an arm? | With no separator, the next pattern can read as a call, an index or a subtraction on the previous result; the spec's own example does. A clause is ended by the next clause word; an arm has nothing like it. | Jarod and Zac weighed newline against comma. "rewrites don't matter, it's v2" | **A comma.** Newlines never matter. |
| 3 | Tuples in both `match` and `transition`? | The spec: they "use the same pattern vocabulary". | "match and transition are identical, only differing in that transition is at the end" | **One grammar.** |
| 4 | After `requires`, before a body? | 775 of 794 such clauses in V1's source end with nothing. A comma would not save the parser any work. | Jarod: "whitespace is possibly fine". Zac, after seeing why a comma would not save the parser work: "earned on match, unearned on clauses" | **Nothing.** |
| 5 | Which words are reserved? | V1 reserves none in effect. | `self` matters only in data and domain predicates. | **None.** |
| 6 | Operator precedence? | V1's table, checked in its code. | "same as it was" | **V1's table.** |
| 7 | Generic arguments or less-than? | A parser cannot know what a name refers to, so in an expression the two start alike. | Jarod: "`::` would resolve it?" Zac: "yeah we can do `::`" | **`::<` in expressions.** |
| 8 | Is assembly parsed now? | The spec defines it. | "just parse it" | **Yes.** |
| 9 | `satisfies` expressions and `some` binders? | In the spec since 2026-11-10. | "obviously in" | **In.** |
| 10 | How is the spec corpus tagged? | 132 examples of mixed kinds. | "I dont care" | **Claude's choice.** |
| 11 | May a comparison be the operand of another? | Grouped to the left, `a == b == c` on three booleans type-checks as `(a == b) == c`, which is not "all three are equal", and nothing downstream objects. Rust, Zig and Lean refuse such chains, all six operators as one group; Rocq defines a chain as both comparisons holding; Odin groups to the left. No spec example chains comparisons. | Decided, then dropped ("just keep the chaining"), then on the boolean case: "this hole seems bad and it sounds like magic, which I hate" and "bring it back". | **No, not without parentheses.** All six comparison operators. This revises decision 6 for those six. |
| 12 | How are references and borrows spelled? | The reader can distinguish a borrow from bitwise AND at sight. | Jarod chose the job-queue example using `@`. | Each `@` adds one layer, including `@mut`, `@write` and nested `@@`. |

A rule that `iN` and `uN` cannot be declared as type names came up under
decision 5. It is a rule about names, so it belongs to the layer that
resolves them, not to this one.

## Where the spec and V1's parser disagree

Found by reading V1's parser. A subagent read its 82 files and reported each
rule with file, line and snippet; I checked the main claims against the code.
V1's compiler could not be run to confirm them: its 2026-11-13 build refuses
every file before parsing.

| # | The spec shows | V1's parser | Evidence | Verdict |
| --- | --- | --- | --- | --- |
| D1 | An `if` statement | Rejects it as retired | V1 retired it 2026-07-02; the spec line was last edited 2026-10-27 | **Rot** |
| D2 | `requires ...;` then a body | Reads the `;` as ending the machine | The example dates from 2026-09-08; the rule against it from 2026-10-30 | **Rot** |
| D3 | Tuple patterns in `match` | One subject, no tuples | Written 2026-11-10 | Spec ahead |
| D4 | An arm ending in a call, then an arm starting with `(` | Reads them as one call | Same example; V1 has no test for it | Hole in both |
| D5 | `asm machine` with `registers` | Only an `asm` statement | Specified 2026-10-29 | Spec ahead |
| D6 | `(clear satisfies A::clear)` | Only in declaration headers | Written 2026-11-10 | Spec ahead |
| D7 | The `some S` binder | Not recognized | Written 2026-11-10 | Spec ahead |

The two rotted examples are worth reporting to whoever maintains the spec:
language/conformances.md:39 and language/generics.md:223.

## What V1 accepts that the spec never shows

Not in this grammar. Each needs a check against the spec's prose before it is
added.

- Items: `proposition`, `capability`, `measure`, `operator`, `module` and
  `package` headers, `boundary data`, quotient data `data N = T % R`, and a
  target word before `machine`.
- Statements: `crash Trap;`, compound assignment `+=` and friends, and a
  proof-output binding `let (value; field: term) = call();`.
- Expressions: an `if` guard on an arm, `Name { fields } if condition -> ...`;
  an evidence lane in calls `f(a, b; evidence)`; `&state`
  borrows; `zero_value<T>()`; `utf16 "..."`.
- An arm whose result is an empty block, `-> {}`.
- Clauses: `stack N bytes`, `reaches A + B`, `reaches <= Bound`, labelled
  predicates `requires name: pred`.
- Optional semicolons after data members.
