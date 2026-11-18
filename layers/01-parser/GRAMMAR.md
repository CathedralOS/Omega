# L1 Parser grammar

Status: **all ten decisions made (2026-11-18); this is the parser's contract
in draft.** It becomes final when the fixtures written from it pass. No
parser code exists yet.

The spec has no grammar, so this document is one. Its sources, in order of
authority:

1. **Decisions by Jarod and Zac**, recorded under [Decisions](#decisions).
   Three of them depart from how the spec writes things.
2. **The spec's prose**, where it states a rule of form.
3. **The spec's 132 `omega` examples**, cited as `file:line` under
   `wiki/spec/`.
4. **Omega V1's parser**, only where the above are silent, and marked as such.

The parser's input is the lexer's tokens. No word is reserved: every word
below, `machine` included, arrives as a plain word and means something only
in the position this grammar gives it. A name such as `block` or `result` is
legal wherever a name is expected.

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
    generic_arg = type | expression | '(' selection ')'

In a declaration or a type, `<` after a name can only open generic arguments,
so it is written bare: `Outcome<u32, Never>`, `machine sort<T>(...)`.

In an expression, generic arguments are written with `::` first, and a bare
`<` after a name is always less-than (decision 7). The parser never looks
ahead to tell them apart.

| The spec writes | V2 writes |
| --- | --- |
| `apply<set_time>(&mut e, 5)` | `apply::<set_time>(&mut e, 5)` |
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
                | stable_number? 'case' word ( '(' ( case_field % ',' ) ')' )? ';'
                | data
    case_field  = stable_number? word ':' type
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
                  hypothesis* '{' machine* '}'
    hypothesis  = 'requires' 'machine' word '(' params ')' result? clause* ';'

`public SaturatingIncrement satisfies Incrementable<Counter> { ... }`
(language/conformances.md:39), and with hypotheses (proofs/quotients.md:196).
A hypothesis is the one clause that ends in a semicolon before a body.

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

- Qualified names `machine EchoSupplier::supply(...)`
  (build/assumed_binding_contracts.md:50).
- Operator tokens: `machine + add(...)` (language/expressions.md:151),
  `machine > Small::greater(...)` (language/domains.md:910),
  `machine [] read<Element [copy]>(...)` (language/expressions.md:405). The
  spec names `+`, `==`, `[]` and `[..]`; the rest of the list is the binary
  operators, pending a check against the spec's operator families.
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

    type        = '&' lifetime? 'mut'? type
                | '[' type ']'
                | '[' type ';' expression ']'
                | machine_type
                | type '->' type
                | path_type
    path_type   = word type_args? ( '::' word type_args? )*
    type_in     = type ( 'in' type_domains )?
    type_domains = path_type ( '&' path_type )*
    machine_type = 'machine' generic_params? '(' params ')' result? clause*
    lifetime    = ''' word

References and views `&[u8]`, `&mut [u8]`, `&'a U`, `&'a mut S`
(language/data_and_literals.md:109, language/some_packages.md:15), arrays
`[u8; 38]` (language/expressions.md:526), machine types as field and
parameter types (language/effects.md:201), function types `A -> B` in
mathematical declarations (proofs/mathematical_bindings.md:63), and
qualification `i32 in Km & Positive` (language/domains.md:572).

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
           | 'clobbers' ( 'none' | word % ',' )
    requires = 'requires' expression

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
- The last expression of a body, with no semicolon, is its result.

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

    pattern = '_' | literal | path
            | path '{' ( field_pattern % ',' )? '..'? '}'
            | '(' ( pattern % ',' ) ')'
    field_pattern = word ( 'as' ( word | '_' ) | ':' expression )?

"`field` binds the field, `field as name` renames it, `field as _` waives it,
and `field: value` is ordinary projected equality" (language/patterns.md,
"Pattern subjects and bindings").

## Expressions

    primary = literal | expr_path | '(' expression ')' | '(' expression % ',' ')'
            | expr_path '{' ( field_init % ',' )? '..'? '}'
            | '[' ( expression % ',' )? ']'
            | match
            | '(' selection ')'
    expr_path  = word expr_args? ( '::' word expr_args? )*
    field_init = word ( ':' expression )?
    selection  = path 'satisfies' type
    postfix = primary ( '(' args ')' | '.' word expr_args? | '[' index ']' )*
    index   = expression | expression? ( '..' | '..=' ) expression?
    prefix  = ( '&' 'mut'? | '!' | '~' | '-' | 'move' | 'suspend' | 'block' )* postfix
    cast    = prefix ( 'as' type? cast_entry* )*
    cast_entry = ( '+' | '-' | '=' ) path_type

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

- **Right shift** is two `>` tokens with nothing between them, and its policy
  suffix is a `%`, `|` or `!` directly after the second. The lexer has no `>>`
  token; this layer joins them by checking that their byte ranges touch
  (language/expressions.md: "Right shift keeps its two adjacent `>` tokens").
- **Casts:** "after `as`, an optional carrier type and then every following
  `=D`, `+D` or `-D` entry belong to the cast. A `+` or `-` directly after a
  cast therefore begins an entry, never an addition or subtraction"
  (language/expressions.md). Examples at language/domains.md:623 and :701.
- **Call markers** `suspend`, `block`, `suspend block` come before a call
  (language/effects.md:871). `block` and `suspend` are prefixes only when
  the next token is not `(`, so a machine named `block` can still be called.
- Record literals with shorthand and explicit omission `..`
  (language/data_and_literals.md:172).
- Membership `self in Value::Integer` (language/data_and_literals.md:272).
- Views `view[2..4]`, `bytes[..]` (language/expressions.md:499, :526).
- A call through a field `(operations.value)(&counter)`
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
  instructions of its parent.

## How the spec's examples change

Three decisions change how existing source is written. The spec's examples
were written before them, so V2's copy of the spec has to be edited to match
before the examples can serve as a test corpus.

| Decision | The spec's examples | V2 |
| --- | --- | --- |
| 2 | Arms on separate lines, no separator | A comma after each arm |
| 7 | `name<T>(x)` in expressions | `name::<T>(x)` |
| 1 | One `if` statement (language/conformances.md:39) | Rewritten as a `transition` |
| 4 | `requires ...;` and `requires ...,` before a body (language/generics.md:223, :242) | Nothing before the body |

Also not source, and tagged as such in the corpus: bodies written `{ ... }`
(build/task_runtime.md:133, language/data_and_literals.md:20,
language/generics.md:636), and the many examples that are fragments
(statements, clauses or expressions outside any declaration).

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
  an evidence lane in calls `f(a, b; evidence)`; `&write` and `&state`
  borrows; `zero_value<T>()`; `utf16 "..."`.
- An arm whose result is an empty block, `-> {}`.
- Clauses: `stack N bytes`, `reaches A + B`, `reaches <= Bound`, labelled
  predicates `requires name: pred`.
- Optional semicolons after data members.
