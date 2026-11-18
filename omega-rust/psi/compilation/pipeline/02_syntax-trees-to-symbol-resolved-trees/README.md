# Resolving names in parsed Omega source

Start at [resolver.rs](resolver.rs). A spelling such as `kit::pick` becomes a reference to a specific declaration. The source compiler can index declarations now and request a machine's implementation later.

| If you want to know… | Start here |
| --- | --- |
| Where does a transition block acquire branches? | [Dispatch preparation](declarations/dispatches.rs) saves each runtime subject once and lowers the ordered arms. A complete Boolean pattern table can end with an unconditional branch. |
| Why does this resolved body contain an extra local? | [Statement lowering](lowering/statement.rs) saves evaluations that source order requires to run once. Typing infers an unannotated local from its initializer. |
| Who can supply this bodyless machine? | [Machine supply](lowering/machine_supply.rs) authenticates compiler-provided signatures. [Machine token bindings](selections/machine_token_bindings.rs) checks the resolved operand shapes for token-bearing declarations. |
| What runs, and in what order? | [`resolve` and `resolve_forest`](resolver.rs). Lower the items, select operator homes, assign symbol identities, then settle constants and calls. |
| What does a build pay for up front? | [`resolve_deferred`](resolver.rs) indexes declarations. [`complete_machine_bodies`](deferred_bodies.rs) completes the requested owners and the dependencies they need. |
| Which names are visible at this use? | [Lexical scopes](names/scopes.rs), [qualified paths](names/paths.rs), and [lookup in the symbol table](names/lookup.rs). |
| How do an alias, generic, or trait default become ordinary declarations? | [Declaration preparation](declarations/mod.rs) rewrites the parsed forest before assigning symbols. |
| Where do mnemonics acquire targets, clobbers and flag effects? | [Assembly preparation](declarations/assembly/mod.rs) reads authored instructions and builds the block graph consumed by [statement lowering](lowering/statement/assembly.rs). |
| Where does a syntax node become a resolved node? | [Lowering](lowering/mod.rs) dispatches by item, expression, statement, or type reference. |
| Which call or operator did an expression select? | [Selections](selections/mod.rs), including [operator homes](selections/domain_operator_homes.rs) and [authored selections](selections/authored_selections.rs). |
| Where does a constant initializer go? | [Constants](constants/mod.rs) owns the initializer, dependencies and substitution across phases. |
| What can build-time evaluation ask before full resolution? | [Pre-resolution queries](pre_resolution/mod.rs) return selected declarations and initializer inputs. |
| A build generated another source file. How does it see existing declarations? | [Generated source](generated_source.rs) extends the retained base and joins the new selections to its existing identities. |

## Porting the pass to Omega

The Omega-written stage starts at [resolver.omg](../../../../../source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/resolver.omg). Follow the operation on each side. For example, in Rust a qualified use leads from [names/paths.rs](names/paths.rs) into the [name lookup](names/lookup.rs) for a visible declaration. In Omega, [imports.omg](../../../../../source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/imports.omg) determines which source units are visible, and [names.omg](../../../../../source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/names.omg) binds uses. The corresponding starting points for a contract or measure are [names/contracts.rs](names/contracts.rs) and [names/measures.rs](names/measures.rs), alongside Omega's [contracts.omg](../../../../../source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/contracts.omg) and [measures.omg](../../../../../source/psi/pipeline/02_syntax-trees-to-symbol-resolved-trees/measures.omg).

Keep the pass order explicit when porting. An operator can acquire a home before symbols are assigned. Substituting a constant needs the symbol table. Recording an authored selection needs the substituted value. Completing one machine body needs the declaration identities the build already holds. These dependencies carry over independently of Rust's module visibility or ownership types.

Rust's [name resolver](https://rustc-dev-guide.rust-lang.org/name-resolution.html) gives import resolution and the lexical scope walk distinct homes. That split makes it possible to follow a shadowed name directly to the scope rules. Zig's [AstGen](https://github.com/ziglang/zig/blob/0.15.2/lib/std/zig/AstGen.zig) lowers local syntax to untyped instructions, while [Sema](https://github.com/ziglang/zig/blob/0.15.2/src/Sema.zig) analyzes instructions on demand. For Omega the comparable split is the visible handoff from prepared declarations to requested machine bodies.

## Time one operation

```sh
mbx bench -p syntax-trees-to-symbol-resolved-trees --bench resolver_timing -- body imports 100
```

Choose `full`, `deferred`, `body`, or `extension`, then `tiny`, `imports`, or `generics`, then a positive repeat count. The [harness](benchmarks/resolver_timing.rs) takes a clean owned input for each invocation. It measures one resolver operation per work unit. It prepares sources, parses, clones inputs, warms up, inspects the result, and drops the returned trees outside the clock. The extension measurement ends before the join with a typed consumer. The body measurement starts with an open package and requests `main`.

The [paired measurements](../../../../../wiki/drafts/measurements/psi_resolver_layout.md) record what the layout change cost on these inputs.
