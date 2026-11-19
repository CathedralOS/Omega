# Source to Psi

Start with [source_to_psi.rs](source_to_psi.rs). It takes the selected source files for one stage and returns a published Psi program.

| Steps | What happens |
| --- | --- |
| 00-01 | [Lex and parse](pipeline/01_tokens-to-syntax-trees/streaming.rs) each file while its tokens are available. |
| 02-03 | [Resolve names and type the program](pipeline/03_symbol-resolved-trees-to-typed-trees/resolve_and_type.rs). |
| 04-06 | [Settle source inputs](source_preparation.rs), then [freeze, lower and check the program](source_judgment.rs). |
| 07 | [Publish the checked program as Psi](pipeline/07_checked-program-to-psi/src/source_publication.rs). |

The [Product driver](product.rs) gathers target declarations and settles providers before publication. The [Build invocation driver](build_invocation.rs) retains its source consent through judgment and publication. [Compile-time calls](compile_time/calls.rs) materialize their arguments and request judgment on the selected call. Each reaches the same [source judgment](source_judgment.rs).

The [numbered pipeline](pipeline/) owns the transformations. The [representations](representations/) describe the data passed between them.
