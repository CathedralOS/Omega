# Source to Psi

Start at [source_to_psi.rs](source_to_psi.rs) for the supported requests. A check stops at the lowered-form verdict. Compiling, inspecting a machine and selecting a deployment continue to publication. [driver.rs](driver.rs) runs those steps and assembles the result for a deployment.

| Steps | Driver responsibility | Numbered transformation |
| --- | --- | --- |
| 00–01 | [Parse a source batch](driver/parse_source_batch.rs) in file order, reusing an identical retained parse when available. | [Lex](pipeline/00_source-files-to-tokens) and [parse](pipeline/01_tokens-to-syntax-trees) one file. |
| 02–03 | [Resolve and type](driver/resolve_and_type.rs) the selected declarations and bodies. A Build can query declarations and generated files can extend the same source identities. | [Name resolution](pipeline/02_syntax-trees-to-symbol-resolved-trees) and [typing](pipeline/03_symbol-resolved-trees-to-typed-trees). |
| 04 | [Settle demanded source inputs](driver/prepare_typed_source.rs), including constant values and target layouts. | [Prepare typed trees](pipeline/04_typed-trees-to-prepared-typed-trees/src/lib.rs) issues the snapshot accepted by lowering. |
| 05–06 | [Judge the lowered source](driver/judge_lowered_source.rs) with its exact declarations and supplies. | [Lower](pipeline/05_prepared-typed-trees-to-lowered-program/src/compilation.rs), then [check that lowering](pipeline/06_lowered-program-to-checked-program/src/publication.rs). |
| 07 | Publish the accepted Product or Build invocation. | [Publication](pipeline/07_checked-program-to-psi/src/source/publication.rs) uses the checked program. |

[Source syntax](source-syntax/) lets source-editing callers lex and parse one string without running later stages. Stage 01 itself parses tokens, including the statements and nested declarations of a machine body.

Three operations can require more source work during one request.

- A [Build](driver/compile_build.rs) can pause at an authored [dependency wait](driver/compile_build/waits/mod.rs). The next interval receives the live values at that source occurrence and the declarations the Build has since acquired.
- A [Product](driver/compile_product.rs) can discover a provider that needs another body judgment or a layout on a particular target. Its worklist closes when every chosen provider is present in the checked program.
- A [compile-time invocation](driver/compile_time_invocations.rs) can supply a value required for preparation. Materialized calls, constant arguments and array extents use one [typed caller](driver/compile_time_invocations/typed_invocation.rs). The interpreter receives a checked invocation and returns its value and [evaluation record](representations/evaluation-receipts/accounting.rs).

The [representations](representations/) own source data shared across stages, including [substitution into generic data members](representations/typed-trees/type_records/data_applications.rs). Published Psi goes to the [artifact writer](../psi/artifact/codec/) or stays in memory for native compilation with the [provider choices for its deployment](representations/published-target-providers/publication.rs).
