# Source to Psi

Start with [source_to_psi.rs](source_to_psi.rs). It takes the selected source files for one stage and returns a published Psi program.

| Steps | What happens |
| --- | --- |
| 00-01 | [Lex and parse](pipeline/01_tokens-to-syntax-trees/streaming.rs) each file while its tokens are available. |
| 02-03 | [Resolve names and type the program](source_continuation.rs). |
| 04 | [Prepare the typed trees](pipeline/04_typed-trees-to-prepared-typed-trees/prepare.rs) after the driver [settles the source inputs](source_preparation.rs). |
| 05-06 | [Lower the prepared trees](pipeline/05_prepared-typed-trees-to-lowered-program/src/compilation.rs) and [check the lowered program](source_judgment.rs). |
| 07 | [Publish the checked program as Psi](pipeline/07_checked-program-to-psi/src/source_publication.rs). |

The [Product driver](product.rs) gathers target declarations and settles providers before publication. The [Build invocation driver](build_invocation.rs) retains its source consent through judgment and publication. [Compile-time calls](compile_time/calls.rs) materialize their arguments and request judgment on the selected call. Each reaches the same [source judgment](source_judgment.rs).

During source preparation, a constant initializer or a compile-time call can require a smaller checked invocation. Source selects the exact declaration and arguments, passes the checked invocation to the Build interpreter and returns its value and accounting to the Product or Build stage waiting for it. [Constant initializers and arguments](compile_time/constants.rs) · [Call preparation](compile_time/calls.rs) · [Invocation and accounting](compile_time/calls/execution.rs)

For a native request, the resulting Psi program stays in memory. Stage 07 supplies the completed Build bindings and checked provider demand for that manifest and deployment. A portable publication exposes the same checked artifact to the `.psi` writer. [Source publication](source_to_psi.rs) · [Target provider handoff](../product-records/target-providers/publication.rs) · [Shared logical provider selection](../psi/providers/logical_reach.rs) · [Psi encoding](../psi/codec/)

The [numbered pipeline](pipeline/) owns the transformations. The [representations](representations/) describe the data passed between them.
