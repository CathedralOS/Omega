# Resolving names in parsed Omega source

Start at [resolver.rs](src/resolver.rs). A spelling such as `kit::pick` becomes a reference to a specific declaration. The source compiler can index declarations now and request a machine's implementation later.

| If you want to know… | Start here |
| --- | --- |
| Which targets admit a machine’s inline assembly? | [Assembly preparation](src/declarations/assembly/mod.rs) intersects the instruction catalog targets for each body. The resolved machine keeps that requirement when place instructions become ordinary assignments. |
| Where does a transition block acquire branches? | [Dispatch preparation](src/declarations/dispatches.rs) saves each runtime subject once and lowers the ordered arms. A complete Boolean pattern table can end with an unconditional branch. |
| Why does this resolved body contain an extra local? | [Statement lowering](src/lowering/statement.rs) saves evaluations that source order requires to run once. Typing derives the type of each compiler-created temporary from the expression it stores. |
| Who can supply this bodyless machine? | [Machine supply](src/lowering/machine_supply.rs) authenticates compiler-provided signatures. [Machine token bindings](src/selections/machine_token_bindings.rs) checks the resolved operand shapes for token-bearing declarations. |
| What runs, and in what order? | [Resolve declarations](src/resolver.rs), then [complete the requested machine bodies](src/deferred_bodies.rs). An appended source forest follows the same two operations. |
| What does a build pay for up front? | [`resolve_deferred`](src/resolver.rs) indexes declarations. [`complete_machine_bodies`](src/deferred_bodies.rs) completes the requested owners and the dependencies they need. |
| Which declarations come from the toolchain library? | The [bundled declaration catalog](../../representations/source-files/bundled-declarations/src/lib.rs) supplies exact library bytes to the resolver after source admission establishes ownership. |
| Which names are visible at this use? | [Lexical scopes](src/names/scopes.rs), [qualified paths](src/names/paths.rs), and [name resolution](src/names/lookup.rs) query the [source symbol table](../../representations/symbol-resolved-trees/symbol-table/source_symbols.rs) against the captured package and file. |
| How do an alias, generic, or trait default become ordinary declarations? | [Declaration preparation](src/declarations/mod.rs) rewrites the parsed forest before assigning symbols. |
| Where do mnemonics acquire targets, clobbers and flag effects? | [Assembly preparation](src/declarations/assembly/mod.rs) reads authored instructions and builds the block graph consumed by [statement lowering](src/lowering/statement/assembly.rs). |
| Where does a syntax node become a resolved node? | [Lowering](src/lowering/mod.rs) dispatches by item, expression, statement, or type reference. |
| Which call or operator did an expression select? | [Selections](src/selections/mod.rs), including [operator homes](src/selections/domain_operator_homes.rs) and [authored selections](src/selections/authored_selections.rs). |
| Where does a constant initializer go? | [Constants](src/constants/mod.rs) owns the initializer, dependencies and substitution across phases. |
| A build generated another source file. How does it see existing declarations? | [Generated source](src/generated_source.rs) extends the retained base and joins the new selections to its existing identities. |

## Porting the pass to Omega

The Omega-written stage starts at [resolver.omg](../../../../../source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/resolver.omg). Follow the operation on each side. For example, in Rust a qualified use leads from [names/paths.rs](src/names/paths.rs) into the [name lookup](src/names/lookup.rs) for a visible declaration. In Omega, [imports.omg](../../../../../source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/imports.omg) determines which source units are visible, and [names.omg](../../../../../source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/names.omg) binds uses. The corresponding starting points for a contract or measure are [names/contracts.rs](src/names/contracts.rs) and [names/measures.rs](src/names/measures.rs), alongside Omega's [contracts.omg](../../../../../source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/contracts.omg) and [measures.omg](../../../../../source/subsystems/compiler/source_to_psi/pipeline/syntax_trees_to_symbol_resolved_trees/measures.omg).

Keep the pass order explicit when porting. An operator can acquire a home before symbols are assigned. Substituting a constant needs the symbol table. Recording an authored selection needs the substituted value. Completing one machine body needs the declaration identities the build already holds. These dependencies carry over independently of Rust's module visibility or ownership types.

When a conformance overrides a trait default, [default selection](src/selections/conformance_blocks.rs) discards the synthesized machine and returns the new position of each survivor. Both deferred bodies and [service reaches](src/selections/service_reaches.rs) use those positions to stay with their declarations.

Rust's [name resolver](https://rustc-dev-guide.rust-lang.org/name-resolution.html) gives import resolution and the lexical scope walk distinct homes. That split makes it possible to follow a shadowed name directly to the scope rules. Zig's [AstGen](https://github.com/ziglang/zig/blob/0.15.2/lib/system/zig/AstGen.zig) lowers local syntax to untyped instructions, while [Sema](https://github.com/ziglang/zig/blob/0.15.2/src/Sema.zig) analyzes instructions on demand. For Omega the comparable split is the visible handoff from prepared declarations to requested machine bodies.

## Time one operation

```sh
mbx bench -p source-compiler --features test-support --bench resolver_timing -- body imports 100
```

Choose `full`, `deferred`, `body`, or `extension`, followed by an input and a positive repeat count. The inputs `tiny`, `imports`, `generics`, and `reach` exercise a single machine, cross-file overloads, a generic array extent, and an authored boundary-service reach. Use `machines-N` to vary the number of independent machines while keeping the requested body the same. For example, `body machines-4096 4` completes `main` four times in a package with 4,096 declarations.

The [harness](benchmarks/resolver_timing.rs) times one resolver operation per work unit. Source preparation, parsing, input copies and result inspection happen outside that timed operation. The extension measurement ends before the join with a typed consumer. The body measurement starts with an open package and requests `main`.

See the paired results for the [resolver reorganization](../../../../../../../../wiki/drafts/measurements/psi_resolver_layout.md) and [assigning service reaches to machines](../../../../../../../../wiki/drafts/measurements/psi_service_reach.md).
The [paired measurements](../../../../../wiki/drafts/measurements/psi_resolver_layout.md) record what the layout change cost on these inputs.
