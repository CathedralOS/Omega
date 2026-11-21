# Follow one program through the V2 compiler

Start with [`check_files` in the CLI](application/src/source.rs). For `omega source lower`, this is where you can see one file become tokens and a syntax tree, several parsed files become one resolved package, and that package go through preparation and into lowering. [`main`](application/src/main.rs) selects the command. [PLAN.md](PLAN.md) tracks implementation status.

| Layer | What comes in | Open this function | What goes out and who reads it |
| --- | --- | --- | --- |
| L0 · Lex | Bytes of one file | [`lex`](layers/00_bytes-to-tokens/src/lexing/lexer.rs) | [Tokens](representations/tokens/src/token.rs) with byte spans. L1 reads them alongside the original bytes. |
| L1 · Parse | File bytes and tokens | [`parse_file`](layers/01_tokens-to-syntax-trees/src/parsing/entry_points.rs) | A [`SyntaxTree`](representations/syntax/src/syntax_tree.rs). The CLI collects each tree with its text, tokens, path and package for L2. |
| L2 · Resolve | Parsed files for the package and its visible names | [`resolve_source_package`](toolchain/src/lib.rs) supplies the toolchain exports to [`resolve_stage_with_exports`](layers/02_syntax-trees-to-symbol-resolved-trees/src/stage.rs) | [`SymbolResolvedTrees`](representations/resolved/src/resolved_trees.rs), wrapped in a `SourcePackage` for typing. |
| L3 · Type | The resolved package and any static answers already known | [`type_source_package_for_preparation`](layers/03_symbol-resolved-trees-to-typed-trees/src/toolchain_declarations/source_package.rs) | [`TypingProgress`](layers/03_symbol-resolved-trees-to-typed-trees/src/typing/typing_progress.rs) gives L4 either completed typed trees or provisional trees with specific requests to evaluate. The standalone `source type` command uses `type_source_package` in the same file. |
| L4 · Prepare | The resolved package at the CLI entry point, then L3’s typed work in each round | [`prepare_source_package_diagnosed`](layers/04_typed-trees-to-prepared-trees/src/preparation/entry_points.rs) | [`PreparedTrees`](representations/prepared/src/prepared_trees.rs) holds the completed typed package and its frozen values. L5 reads this product. |
| L5 · Lower | A prepared package | [`lower_package`](layers/05_prepared-trees-to-terminal-psi/src/lowering/package.rs) | A [`LoweredPackage`](representations/psi/src/package.rs) contains portable Psi declarations and machine bodies. The CLI reports success or the lowering diagnostic. |

## Follow a signed constant through L3 and L4

Open [signed_wrap.omg](layers/04_typed-trees-to-prepared-trees/tests/fixtures/signed_arithmetic/signed_wrap.omg). Its last machine takes `Indexed<OFFSET>` and returns `Indexed<-128>`. To check that returning the argument is legal, L3 needs the value of `OFFSET`.

1. **Find the request.** L3 returns a pending typing pass. Its closed request points to the word `OFFSET` in the parameter type and asks for an `i8`. The [layer test](layers/04_typed-trees-to-prepared-trees/tests/signed_arithmetic.rs) inspects that request.
2. **Follow the answer.** In [`preparation/rounds.rs`](layers/04_typed-trees-to-prepared-trees/src/preparation/rounds.rs), start at `PreparationRound::begin` and follow `finish_without_external_evidence`. The round evaluates the request and `apply` supplies the answer to typing. In this program `adjust(127)` wraps its addition to `-128`, then saturating subtraction keeps it at `-128`.
3. **Inspect the product.** The same test reads `OFFSET` from the frozen constants and inspects both sides of the typed `carry` signature. Both refer to the same `Indexed` declaration with the static argument `-128i8`. L5 starts by reading `prepared.typed()` in [`lower_package`](layers/05_prepared-trees-to-terminal-psi/src/lowering/package.rs). On this fixture it reaches the generic data declaration and reports “generic data requires a closed application before lowering.”
4. **Change one operator.** [signed_overflow.omg](layers/04_typed-trees-to-prepared-trees/tests/fixtures/signed_arithmetic/signed_overflow.omg) uses exact addition (`+`) in place of wrapping addition (`+%`). Adding one to `127i8` now overflows. The layer test checks L4’s refusal and its span at `adjust(127)`. The [CLI test](application/tests/cli.rs) runs both files and checks the reported location and process status.

Run these tests from the V2 workspace:

```sh
mbx test --locked -j 1 -p omega-preparation --test signed_arithmetic
mbx test --locked -j 1 -p omega --test cli prepares_a_signed_index
```

## Find a test by behavior

| To inspect | Open the input | Open the assertion |
| --- | --- | --- |
| Which integer spellings lex | [L0 numbers](layers/00_bytes-to-tokens/tests/fixtures/numbers) | [Token and diagnostic comparisons](layers/00_bytes-to-tokens/tests/golden_fixtures.rs) |
| How the parser groups operators | [L1 operators](layers/01_tokens-to-syntax-trees/tests/fixtures/operators) | [Syntax tree and diagnostic comparisons](layers/01_tokens-to-syntax-trees/tests/golden_fixtures.rs) |
| How file location affects name lookup | [L2 sibling and child packages](layers/02_syntax-trees-to-symbol-resolved-trees/tests/fixtures/directory_lookup) | [directory_lookup.rs](layers/02_syntax-trees-to-symbol-resolved-trees/tests/directory_lookup.rs) |
| Which static index typing accepts | [L3 computed widths](layers/03_symbol-resolved-trees-to-typed-trees/tests/fixtures/computed_width) | [Typed facts and refusal comparisons](layers/03_symbol-resolved-trees-to-typed-trees/tests/expected.rs) |
| What preparation freezes or refuses | [L4 signed arithmetic](layers/04_typed-trees-to-prepared-trees/tests/fixtures/signed_arithmetic) | [signed_arithmetic.rs](layers/04_typed-trees-to-prepared-trees/tests/signed_arithmetic.rs) |
| How lowering handles a computed array extent | [L5 array extent](layers/05_prepared-trees-to-terminal-psi/tests/fixtures/array_extent) | [CLI fixture comparison](application/tests/cli.rs) · [Psi array type assertions](layers/05_prepared-trees-to-terminal-psi/tests/handoff.rs) |

## Where a test program lives

Most V2 test programs are short strings written beside the assertions that inspect them. A program becomes a file under its layer’s `tests/fixtures/<behavior>/` in three cases, and each case has its own layout:

| When | Layout | How a test finds it |
| --- | --- | --- |
| The test compares the layer’s whole printed output (lexer, parser, typer) | `<behavior>/accept/<case>.omg` or `<behavior>/reject/<case>.omg`, each with a sibling `<case>.expected` | The golden runner discovers every pair. Adding a pair adds a test. `OMEGA_BLESS=1` rewrites the `.expected` files for review. |
| The directory structure is what the test examines, as in name lookup or file imports | `<behavior>/<scenario>/<package tree>` | The test names each file with `include_str!`. Preserve paths inside the package because they determine which names are visible. |
| More than one test reads the program, usually a layer test and the [CLI test](application/tests/cli.rs) | `<behavior>/<case>.omg`, with a positive and negative twin side by side | Each test names the file. |

Keep a program inline when one test reads it and checks specific fields of the product, as [L5 arithmetic](layers/05_prepared-trees-to-terminal-psi/tests/arithmetic.rs) does.

The typer’s `prepared/accept` and `prepared/reject` cases need a value that preparation would normally compute. Each has a sibling `<case>.answer` file with one line per supplied value, written `<call text> | <carrier> | <argument> -> <value>`. In [computed_width.answer](layers/03_symbol-resolved-trees-to-typed-trees/tests/fixtures/computed_width/prepared/accept/computed_width.answer), `width(N) | u8 | 7 -> 8` tells typing that `width(N)` is 8 when `N` is 7. The runner refuses a `prepared/` case that has no answer.

The spec-corpus tests for [resolution](layers/02_syntax-trees-to-symbol-resolved-trees/tests/spec_corpus.rs), [typing](layers/03_symbol-resolved-trees-to-typed-trees/tests/spec_corpus.rs) and [lowering](layers/05_prepared-trees-to-terminal-psi/tests/spec_corpus.rs) each read their own `tests/spec-outcomes.txt`. These files record the expected result for examples drawn from the shared specification. The [fixed sample baseline](layers/05_prepared-trees-to-terminal-psi/tests/sample_baseline.rs) reads pinned samples from Git and uses [local import fixtures](layers/05_prepared-trees-to-terminal-psi/tests/fixtures/direct_file_imports) for its source import cases.
