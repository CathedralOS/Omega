# Parsing

Start at [parser.rs](parser.rs). `parse` consumes one token stream, asks
[declarations/items.rs](declarations/items.rs) for each declaration and publishes
its syntax roots. The same entry file coordinates batches and independent syntax
pages. Grammar dispatch is visible here and in the domain entries below.

| Follow this path | It owns |
| --- | --- |
| [declarations/items.rs](declarations/items.rs) | Root dispatch; sibling files parse machines, data, imports and other declarations |
| [expressions/parse.rs](expressions/parse.rs) | The expression frame loop; siblings handle operators, postfix syntax, primary operands, membership and proof facts |
| [types/references.rs](types/references.rs) | Named, applied, reference and aggregate types; `properties.rs` handles type properties |
| [parameters/callable.rs](parameters/callable.rs), [generics.rs](parameters/generics.rs) | Callable and generic binders; requirements and relevance rules are shared here |
| [contracts/clauses.rs](contracts/clauses.rs) | Machine clauses; siblings parse facts, signatures, conformance and external bindings |
| [machine_bodies/states.rs](machine_bodies/states.rs) | Machine entries and states; `defaults.rs` handles trait defaults |
| [transitions/blocks.rs](transitions/blocks.rs) | Transition blocks, with guards and targets in adjacent files |
| [statements/sequence.rs](statements/sequence.rs), [parse.rs](statements/parse.rs) | Statement sequence and dispatch, then binding, destructuring and expression statements |
| [patterns/fields.rs](patterns/fields.rs) | Destructure field entry and result; `bindings.rs` rewrites captured binding references |
| [assembly/parse.rs](assembly/parse.rs) | Assembly blocks; `generated_statements.rs` constructs their register and flag statements |
| [token_input/cursor.rs](token_input/cursor.rs) | Token cursor, retained source, spans and lookahead; adjacent files handle delimiters and paths |
| [literals/numbers.rs](literals/numbers.rs) | Numeric value validation after lexical recognition |
| [syntax_storage/](syntax_storage/) | Shared handle spans, table access and copying paths into statement tables |
| [errors/parse_error.rs](errors/parse_error.rs) | The public parse error; `expected.rs` builds expected-token diagnostics |

The output belongs to [syntax-trees](../../representations/syntax-trees/).
This stage preserves authored syntax and source spans. Symbol resolution, typing
and proof checking follow it. It neither changes the token representation nor
publishes a second inter-stage product.

The expression loop uses heap-backed frames for nested expressions, groups,
arguments and aggregates. Type parsing and some grammar crossings still recurse;
this is not a claim that the entire parser is stackless. Precedence and operator
policy live in [expressions/operators.rs](expressions/operators.rs), beside the
loop that applies them.

[tests/unit/](tests/unit/) groups behavior tests by grammar feature. Small private
helper tests remain registered with their owner, but their bodies live there too.
[tests/diagnostics.rs](tests/diagnostics.rs) checks public errors;
[tests/sample_files.rs](tests/sample_files.rs) checks sample source parsing.
There are no inline test bodies in production files.

## Comparing parser designs

The closest fit for discoverability is Clang's named entry and grammar domains.
The useful part of Rust's layout is also its grammar names, independent of its
module convention. These observations come from the upstream implementations
inspected on 2026-11-16, not a cross-language speed comparison:

| Compiler | What its source shows | What applies here |
| --- | --- | --- |
| [Clang](https://github.com/llvm/llvm-project/tree/main/clang/lib/Parse) | `ParseAST.cpp` drives translation-unit parsing; `ParseDecl`, `ParseExpr` and `ParseStmt` files own grammar domains | Make the entry do real coordination and let the reader follow grammar names |
| [Rust](https://github.com/rust-lang/rust/tree/master/compiler/rustc_parse/src/parser) | Separate expression, item, statement, type, pattern, path and generic parsing | Split by language concepts; avoid our former grammar re-export facade |
| [Zig](https://github.com/ziglang/zig/blob/master/lib/std/zig/Parse.zig) | A central parser with grammar comments, indexed node tables, extra data and reusable scratch lists | Keep grammar intent near the code; shared scratch and compact storage merit measurements, not automatic adoption |
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
mbx bench -p tokens-to-syntax-trees --bench parser_timing -- 100 <absolute-source-path>...
mbx bench -p tokens-to-syntax-trees --bench parser_timing --no-run
```

Use the repository's host build budget. Cargo runs the harness from its package
directory, so use absolute input paths. Save the executable printed by `--no-run`
for each source revision and compare immutable inputs with
[omega-bench](../../../../../tools/benchmark/README.md) `reported_work` plans.
The [before/after report](../../../../../wiki/drafts/measurements/rust_parser_2026_11_16.md)
records source identities, host limits, paired results and rejected experiments.
