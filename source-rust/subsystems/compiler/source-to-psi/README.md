# Source to Psi

Start with [source_to_psi.rs](source_to_psi.rs). It takes the selected source files for one stage and returns a published Psi program.

| Steps | What happens |
| --- | --- |
| 00-01 | [Lex and parse](pipeline/01_tokens-to-syntax-trees/streaming.rs) each file while its tokens are available. |
| 02-03 | [Resolve names and type the program](pipeline/03_symbol-resolved-trees-to-typed-trees/resolve_and_type.rs). |
| 04-06 | [Prepare the typed trees, lower them, and check the resulting program](pipeline/prepare_and_check.rs). The checker discharges the obligations introduced by lowering. |
| 07 | [Publish the checked program as Psi](pipeline/07_checked-program-to-psi/src/source_publication.rs). |

The [Build invocation driver](pipeline/build_invocation.rs) prepares the selected Build stage and publishes its checked invocation with the same source consent.

The [numbered pipeline](pipeline/) owns the transformations. The [representations](pipeline/representations/) describe the data passed between them.
