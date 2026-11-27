# Parsing

Start at [src/lib.rs](src/lib.rs). `parse_into` reads one token stream into a
syntax page and returns its declaration roots. It dispatches declarations
through [declarations/items.rs](src/declarations/items.rs). The
[streaming caller](../../driver/parse_source_batch.rs) calls this operation as it lexes each
file, recycling each token buffer after parsing.

The [source-files input](../../representations/source-files/source_files.rs) defines the origin and dependency scope retained on each parsed page. Run the parser unit, diagnostic and sample tests with `mbx test --locked -p source-parser`. Multi-file page scheduling and parallel dispatch live in [parse_source_batch](../../driver/parse_source_batch/batch.rs).

| Follow this path | It owns |
| --- | --- |
| [declarations/items.rs](src/declarations/items.rs) | Root dispatch; sibling files parse machines, data, imports and other declarations |
| [expressions/parse.rs](src/expressions/parse.rs) | The expression frame loop; siblings handle operators, postfix syntax, primary operands, membership and proof facts |
| [types/references.rs](src/types/references.rs) | Named, applied, reference and aggregate types; `properties.rs` handles type properties |
| [parameters/callable.rs](src/parameters/callable.rs), [generics.rs](src/parameters/generics.rs) | Callable and generic binders; requirements and relevance rules are shared here |
| [contracts/clauses.rs](src/contracts/clauses.rs) | Machine clauses; siblings parse facts, signatures, conformance and external bindings |
| [declarations/machine_bodies.rs](src/declarations/machine_bodies.rs) | Statements and nested machine declarations inside a machine body |
| [transitions/blocks.rs](src/transitions/blocks.rs) | One dispatch per transition block, with subjects and ordered arm patterns, guards and targets |
| [statements/sequence.rs](src/statements/sequence.rs), [parse.rs](src/statements/parse.rs) | Statement sequence and dispatch, then binding, destructuring and expression statements |
| [patterns/fields.rs](src/patterns/fields.rs) | Destructure field entry and result; `bindings.rs` rewrites captured binding references; [positional.rs](src/patterns/positional.rs) reads `(first, _, ..)`, recording each component's place and a final `..` as a row of its own |
| [assembly/parse.rs](src/assembly/parse.rs) | Authored mnemonics, operands, labels and contracts. [Instruction preparation](../02_syntax-trees-to-symbol-resolved-trees/src/declarations/assembly/mod.rs) resolves their target behavior |
| [token_input/cursor.rs](src/token_input/cursor.rs) | Source spans and lookahead across grammar tokens. The cursor retains trivia for newline-sensitive clauses. [delimiters.rs](src/token_input/delimiters.rs) handles balanced token ranges. |
| [literals/numbers.rs](src/literals/numbers.rs) | Numeric value validation after lexical recognition |
| [errors/parse_error.rs](src/errors/parse_error.rs) | The public parse error; `expected.rs` builds expected-token diagnostics |

For `(registry.store)(key, value)`, [postfix parsing](src/expressions/postfix.rs)
records a call on a member and retains both arguments. Follow the receiver into
[typing](../03_symbol-resolved-trees-to-typed-trees/src/expressions/atomic_calls.rs) to see
why an atomic cell's `store` checks its ordering argument. For a transition,
[dispatch preparation](../02_syntax-trees-to-symbol-resolved-trees/src/declarations/dispatches.rs)
takes the authored arms and arranges to evaluate the subjects once.

The output belongs to [syntax-trees](../../representations/syntax-trees/syntax_trees.rs).
Converting `ParseError` to a diagnostic retains the existing path, line, column and message format.
Generic spans come from [foundation/arena](../../../../../foundation/arena/src/arena.rs);
[tree copying](../../representations/syntax-trees/copying/statement_paths.rs) belongs
to the representation. The parser accesses its public tables directly.
This stage preserves authored syntax and source spans. Symbol resolution, typing
and proof checking follow it. It neither changes the token representation nor
publishes a second inter-stage product.

The expression loop uses heap-backed frames for nested expressions, groups,
arguments and aggregates. Type parsing and some grammar crossings still recurse;
this is not a claim that the entire parser is stackless. Precedence and operator
policy live in [expressions/operators.rs](src/expressions/operators.rs), beside the
loop that applies them.

[tests/unit/](tests/unit/) groups behavior tests by grammar feature. Small private
helper tests remain registered with their owner, but their bodies live there too.
[tests/diagnostics.rs](tests/diagnostics.rs) checks public errors;
[tests/sample_files.rs](tests/sample_files.rs) checks sample source parsing.
[tests/lexical_handoff.rs](tests/lexical_handoff.rs) checks that comments and
whitespace between tokens move spans and change nothing else in the tree.
[tests/positional_patterns.rs](tests/positional_patterns.rs) checks what a
positional pattern and a final `..` hand to the next stage: each place, the
two bytes of the `..`, and the reads of the bindings by place.
Small fixtures use [tests/support/frontend_operations.rs](../../../../../tests/support/frontend_operations.rs)
to construct complete stage inputs. Private grammar tests can still inspect
partial construction directly.
There are no inline test bodies in production files.

## Comparing parser designs

The timing harness uses one persistent invocation worker, preserving the large
parsing stack without timing worker creation for each parse.

The closest fit for discoverability is Clang's named entry and grammar domains.
The useful part of Rust's layout is also its grammar names, independent of its
module convention. These observations come from the upstream implementations
inspected on 2026-11-16, not a cross-language speed comparison:

| Compiler | What its source shows | What applies here |
| --- | --- | --- |
| [Clang](https://github.com/llvm/llvm-project/tree/main/clang/lib/Parse) | `ParseAST.cpp` drives translation-unit parsing; `ParseDecl`, `ParseExpr` and `ParseStmt` files own grammar domains | Make the entry do real coordination and let the reader follow grammar names |
| [Rust](https://github.com/rust-lang/rust/tree/master/compiler/rustc_parse/src/parser) | Separate expression, item, statement, type, pattern, path and generic parsing | Split by language concepts; avoid our former grammar re-export facade |
| [Zig](https://github.com/ziglang/zig/blob/master/lib/system/zig/Parse.zig) | A central parser with grammar comments, indexed node tables, extra data and reusable scratch lists | Keep grammar intent near the code; shared scratch and compact storage merit measurements, not automatic adoption |
| [Odin](https://github.com/odin-lang/Odin/blob/master/src/parser.cpp) | A large parser file covers expressions, statements, declarations and package work | A clear filename helps, but one large file does not supply navigable domains at our scale |
| [Go](https://go.dev/src/cmd/compile/internal/syntax/parser.go) | A central recursive-descent parser with file/declaration entry and precedence-based expression parsing | Keep control flow direct; there is no need to introduce a parser generator |

Omega already has indexed syntax tables and iterative expression parsing. This
change keeps them. Reserving eight expression frames was measured separately and rejected because
it did not show a reliable gain. The measurement report records both comparisons.

## Benchmark parsing

[benchmarks/parser_timing.rs](benchmarks/parser_timing.rs) measures parsing of
retained, already-tokenized inputs, including creating and dropping syntax trees.
File reads, lexing, token fingerprints and syntax snapshots happen before timing.
The fingerprint covers input text, token kinds/spans/decoded bytes, import ranges
and the existing root syntax snapshot; it is not a complete serialization of all
internal tables. The behavior tests also check diagnostics and less common forms.

```text
mbx bench -p source-compiler --features test-support --bench parsing_parser_timing -- 100 <absolute-source-path>...
mbx bench -p source-compiler --features test-support --bench parsing_parser_timing --no-run
```

Use the repository's host build budget. Cargo runs the harness from its package
directory, so use absolute input paths. Save the executable printed by `--no-run`
for each source revision and compare immutable inputs with
[omega-bench](../../../../../tools/benchmark/README.md) `reported_work` plans.
The [before/after report](../../../../../wiki/drafts/measurements/rust_parser_2026_11_16.md)
records source identities, host limits, paired results and rejected experiments.
