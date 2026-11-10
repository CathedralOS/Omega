# Command-line tools

Source: `wiki/spec/build/command_line.md` lines 1–150 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
COMMAND_LINE1 | "Argument parsing and presentation belong to the dispatcher; semantic analysis, validation and edits belong to their owning subsystem." | Command-line tools | The dispatcher must not perform semantic analysis, validation, or edits; those operations must run only in the owning subsystem | not stated | SHARED
COMMAND_LINE2 | "Subcommands do not assemble private compiler stages." | Command-line tools | A subcommand invocation must not assemble private compiler stages outside the routed subsystem | not stated | SHARED
COMMAND_LINE3 | "`omega --check` with no root inside a workspace checks every declared member as a [validation subject](validation_subjects.md)." | Command vocabulary | With no root path in a workspace, `--check` must treat every declared workspace member as a validation subject | not stated | SHARED
COMMAND_LINE4 | "`--verify` withholds trust acceptance and re-verifies; it does not widen the subject set." | Command vocabulary | When `--verify` is used, package trust acceptance must be withheld, re-verification must run, and the set of validation subjects must not grow beyond what `--check` alone would use | not stated | SHARED
COMMAND_LINE5 | "A locator requires that selector;" | Command vocabulary | Locator-style package audit must include the `--kind` selector | not stated | SHARED
COMMAND_LINE6 | "`--rev` belongs to that form." | Command vocabulary | `--rev` must be accepted only on the locator audit form that includes `--kind` and a locator | not stated | SHARED
COMMAND_LINE7 | "Reject combinations with project-audit-only arguments." | Command vocabulary | Locator audit arguments combined with arguments reserved for project audit must be rejected | not stated | SHARED
COMMAND_LINE8 | "Without a locator form, audit inspects the named project or its ordinary current-directory default." | Command vocabulary | Project audit without a locator must audit the named project or the ordinary current-directory default | not stated | SHARED
COMMAND_LINE9 | "Inspection does not accept package changes." | Command vocabulary | Psi inspection must not accept or apply package changes | not stated | SHARED
COMMAND_LINE10 | "There is one public spelling for each operation." | Command vocabulary | Each operation must be exposed under exactly one public command spelling | not stated | SHARED
COMMAND_LINE11 | "Former flat install/update, action-first audit and hyphenated inspection commands are not compatibility aliases." | Command vocabulary | Legacy command spellings must be rejected, not accepted as aliases | not stated | SHARED
COMMAND_LINE12 | "Use it in command names, help, reports and user-facing documentation." | Public Psi naming | User-facing command names, help, reports, and documentation must use the public name **Psi** for the portable IR, artifact, and interpreted execution product | not stated | SHARED
COMMAND_LINE13 | "`psi inspect` does not expose a compiler stage name." | Public Psi naming | The `psi inspect` command surface must not expose an internal compiler stage name | not stated | SHARED
COMMAND_LINE14 | "It is not the public artifact name." | Public Psi naming | The label `Terminal Psi` must not be used as the public artifact name in user-facing surfaces covered by this chapter | not stated | SHARED
COMMAND_LINE15 | "The interpreted engine spelling is `--engine psi`." | Public Psi naming | The public selector for the interpreted execution engine must be `--engine psi` | not stated | SHARED
COMMAND_LINE16 | "The interpreted work limit is `--psi-work-budget <positive-u64>` wherever that budget is applicable." | Public Psi naming | Where a interpreted work budget applies, the public limit selector must be `--psi-work-budget` with a positive u64 | not stated | SHARED
COMMAND_LINE17 | "`omega source rewrite <ExactRule>... <root.omg>` requests only the exact named transformations, with at least one rule." | Source rewriting | A rewrite invocation must name at least one exact rule and must apply only those named transformations | not stated | SHARED
COMMAND_LINE18 | "Unknown names reject." | Source rewriting | A rewrite rule name not in the registry must be rejected | not stated | SHARED
COMMAND_LINE19 | "There are no implicit style tiers, automatic rule bundles or rewrites during ordinary compilation." | Source rewriting | Ordinary compilation must not run implicit rewrite tiers, bundles, or unnamed rewrite rules | not stated | SHARED
COMMAND_LINE20 | "The rule registry specifies applicability and the preservation argument for each transformation." | Source rewriting | Each registered rewrite rule must have specified applicability conditions and a preservation argument before the transformation may run | not stated | SHARED
COMMAND_LINE21 | "Rechecking alone proves validity, not behavioral equivalence." | Source rewriting | A rewrite accepted after recheck may be treated as valid source only; behavioral equivalence must not be claimed from recheck alone | not stated | SHARED
COMMAND_LINE22 | "Resolve calls under the [acknowledgement contract](../language/effects.md#call-site-acknowledgements)." | Source rewriting | The `redundant-call-markers` rewrite must resolve call sites under the acknowledgement contract | not stated | 3
COMMAND_LINE23 | "Remove a marker only when its axis is absent from the source occurrence's checked envelope." | Source rewriting | A suspend or block marker may be removed only if that marker's axis is absent from the occurrence's checked envelope | not stated | 3
COMMAND_LINE24 | "A generic body uses its callback upper bounds; a currently synchronous instantiation does not authorize editing away a marker the generic needs." | Source rewriting | Marker removal in a generic body must use callback upper bounds; a synchronous instantiation must not justify removing a marker still required by the generic | not stated | 3
COMMAND_LINE25 | "Unknown selections retain their conservative requirements." | Source rewriting | When applicability is unknown, rewrite analysis must keep conservative marker and call requirements | not stated | 3
COMMAND_LINE26 | "Preserve the other marker, call syntax, ordinary obligations and all source behavior." | Source rewriting | After removing one marker, the other marker, call syntax, ordinary proof and contract obligations, and source behavior must remain preserved | not stated | SHARED
COMMAND_LINE27 | "Analysis uses the source's declared checking context and required selections, not an optimizer's predicted target or one convenient execution." | Source rewriting | Rewrite analysis must use declared checking context and required selections, not an optimizer-predicted target or a convenient execution choice | not stated | SHARED
COMMAND_LINE28 | "Limit edits to the requested writable source set; dependency stores are not editing targets." | Source rewriting | Edits must stay within the requested writable source set and must not target dependency stores | not stated | SHARED
COMMAND_LINE29 | "Retain exact source versions and token spans." | Source rewriting | Rewrite output must retain exact source versions and token spans for edited regions | not stated | SHARED
COMMAND_LINE30 | "Preserve comments and unaffected text, reject ambiguous or conflicting edits" | Source rewriting | Comments and unaffected text must be preserved; ambiguous or conflicting edits must be rejected | not stated | SHARED
COMMAND_LINE31 | "detect intervening file changes before committing rather than overwriting another edit." | Source rewriting | Before commit, intervening on-disk changes must be detected and must not be silently overwritten | not stated | SHARED
COMMAND_LINE32 | "Construct the complete edited source set in staging and recheck it before any original is changed." | Source rewriting | All planned files must be staged, rechecked as a complete set, and only then may originals change | not stated | SHARED
COMMAND_LINE33 | "Failed analysis or rechecking writes no source files." | Source rewriting | On analysis or recheck failure, no source file writes may occur | not stated | SHARED
COMMAND_LINE34 | "Commit the validated edits as one logical transaction: success means every planned file was updated; a reported write failure restores the originals." | Source rewriting | Successful rewrite commit must update every planned file or restore originals after any reported write failure | not stated | SHARED
COMMAND_LINE35 | "Recovery must complete or restore the edit set before another rewrite can succeed." | Source rewriting | After interrupted writes, recovery must finish or restore the edit set before another rewrite is allowed to succeed | not stated | SHARED
COMMAND_LINE36 | "With `--check`, report the proposed edits without writing source files." | Source rewriting | In preview mode, proposed edits must be reported and source files must not be written | not stated | SHARED
COMMAND_LINE37 | "No required edit yields success; available edits yield a nonzero CI result." | Source rewriting | Preview when no edit is required must exit successfully; preview when edits are available must yield a nonzero CI exit status | not stated | SHARED
COMMAND_LINE38 | "Analysis/recheck failures remain distinguishable from an edit-needed result." | Source rewriting | Failures of analysis or recheck must not be reported the same way as a successful preview that merely needs edits | not stated | SHARED
COMMAND_LINE39 | "Without `--check`, report the applied edits after successful rechecking and commit." | Source rewriting | After a successful non-preview rewrite, applied edits must be reported only after recheck and commit | not stated | SHARED
COMMAND_LINE40 | "The rule produces no default-path compiler warning and does not make permitted redundant markers invalid." | Source rewriting | The redundant-call-markers rule must not add a default-path compiler warning and must not treat permitted redundant markers as invalid source | not stated | 3
COMMAND_LINE41 | "`--validator <ExactName>` requests a named independent stage-output validator inside this compilation." | Independent stage validators | A named `--validator` option must request exactly that independent stage-output validator for the compilation | not stated | SHARED
COMMAND_LINE42 | "Combining it with named requests does not run the same validator twice;" | Independent stage validators | When `--every-validator` and named validators overlap, the same validator must not run twice | not stated | SHARED
COMMAND_LINE43 | "each explicit request still requires applicability." | Independent stage validators | Each explicitly named validator must still satisfy applicability before it runs | not stated | SHARED
COMMAND_LINE44 | "Selection adds checking work, not new source semantics or compiler authority." | Independent stage validators | Validator selection must add only checking work and must not change source semantics or expand compiler authority | not stated | SHARED
COMMAND_LINE45 | "It does not select extra compilation stages, emit a native artifact for `--check`, relax admission, or replace required source/boundary checks." | Independent stage validators | Validator selection must not add compilation stages, emit native artifacts under `--check`, relax admission, or replace required source or boundary checks | not stated | SHARED
COMMAND_LINE46 | "Unknown names reject." | Independent stage validators | An unknown validator exact name must be rejected | not stated | SHARED
COMMAND_LINE47 | "A named validator whose stage is not produced or whose target/product requirements are unavailable is an invocation error, even when `--every-validator` was also requested." | Independent stage validators | A named validator with no produced stage or unavailable target/product requirements must be an invocation error | not stated | SHARED
COMMAND_LINE48 | "A skipped or inapplicable validator is never reported as passed." | Independent stage validators | Skipped or inapplicable validators must not appear as passed in reports | not stated | SHARED
COMMAND_LINE49 | "A validator failure makes the command fail;" | Independent stage validators | Any validator failure must fail the invoking command | not stated | SHARED
COMMAND_LINE50 | "continue to distinguish compiler invariant failures from invalid authored source and unavailable checks." | Independent stage validators | Outcomes must remain distinguishable among compiler invariant failure, invalid authored source, and unavailable checks | not stated | SHARED
COMMAND_LINE51 | "`--verify` keeps its separate meaning: with `--check`, withhold retained package trust acceptance and re-verify under retained source pins." | Independent stage validators | `--verify` with `--check` must withhold retained package trust acceptance and re-verify under retained source pins; it must not be treated as stage validation | not stated | SHARED
COMMAND_LINE52 | "It is not a spelling for stage validation." | Independent stage validators | `--verify` must not be accepted as an alias for `--validator` or `--every-validator` stage validation | not stated | SHARED
COMMAND_LINE53 | "Default compilation retains the existing [verification placement](../../../omega-rust/pipeline.md#verification-placement)." | Independent stage validators | Default compilation must keep verification at the existing placement unless explicitly changed elsewhere | not stated | SHARED
COMMAND_LINE54 | "Missing markers and invalid positions still reject; permitted extras create no effect or continuation." | Required acceptance | Authored call sites with missing required markers or invalid marker positions must be rejected; extra permitted markers must not add effects or continuations | not stated | 3
COMMAND_LINE55 | "A successful rewrite preserves execution and ordinary contract acceptance." | Required acceptance | A rewrite that completes successfully must preserve execution behavior and ordinary contract acceptance of the rechecked source | not stated | SHARED

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"`--verify` withholds trust acceptance and re-verifies; it does not widen the subject set." | `--verify` only changes trust handling and re-verification on the same subjects `--check` would already use. | `--verify` might change verification work on each subject while still keeping the member list identical; the sentence does not spell out every check difference versus plain `--check`. |
"`--rev` belongs to that form." | `--rev` is valid only on locator audit with `--kind` and a locator argument. | `--rev` might be syntactically accepted in other audit forms but ignored; the sentence says it “belongs” without stating whether other uses reject or are ignored. |
"Reject combinations with project-audit-only arguments." | Any locator audit invocation that includes arguments defined only for project audit must fail. | Only some listed project-audit-only arguments might be forbidden; the chapter does not enumerate which flags count as project-audit-only here. |
"Rechecking alone proves validity, not behavioral equivalence." | After recheck, the rewritten source is accepted as well-formed only; runtime behavior equivalence is out of scope. | “Validity” might mean only type or proof checking, or might include stronger compile-time guarantees; behavioral equivalence is explicitly excluded either way. |
"A generic body uses its callback upper bounds; a currently synchronous instantiation does not authorize editing away a marker the generic needs." | Synchronous instantiation never relaxes marker requirements inferred from generic callback bounds. | Instantiation-specific facts might still permit removal when generic bounds are satisfied; “does not authorize” might mean only that sync facts alone are insufficient, not that removal is always forbidden. |
"No required edit yields success; available edits yield a nonzero CI result." | Preview exit status is success when the rewrite would make no change and failure when any change would be applied. | “Available edits” might mean optional rule hits versus required transformations; the sentence does not define “required edit” in preview terms. |
"each explicit request still requires applicability." | A named validator that is inapplicable must be skipped or errored under the same rules as without `--every-validator`. | An explicit name might force an attempt even when the stage is absent; the pairing with the later invocation-error sentence leaves the failure mode implicit. |
"Selection adds checking work, not new source semantics or compiler authority." | Validators cannot change language meaning or admission rules for source. | Validators might still cause compilation to fail on artifacts that would otherwise pass default checks, which can feel like stricter “authority” without changing semantics. |
"A skipped or inapplicable validator is never reported as passed." | Reports must show skip/inapplicable distinctly from pass. | Skipped validators might be omitted from some report views; “never reported as passed” does not require they appear in every view. |
"Missing markers and invalid positions still reject; permitted extras create no effect or continuation." | Required markers must be present at legal positions; optional redundant markers are semantically inert. | “Permitted extras” might mean only markers allowed by the redundant-call-markers rule, or any marker not required to reject; scope of “no effect or continuation” is not narrowed here. |
"A successful rewrite preserves execution and ordinary contract acceptance." | Success implies native/psi execution and compile-time contract checking match pre-rewrite behavior for accepted programs. | “Preserves execution” might mean only for the selected checking context used in rewrite analysis, not all targets or engines. |

## Terms used without definition in this chapter

- `omega` application
- compiler / execution / package / Psi inspection / source-editing subsystems
- dispatcher
- subsystem
- action
- validation subject
- workspace
- declared member
- trust acceptance
- re-verify
- subject set
- locator audit
- locator
- `--kind`
- `--rev`
- project-audit-only arguments
- project audit
- current-directory default
- public spelling
- compatibility aliases
- flat product commands
- portable IR
- interpreted execution product
- compiler stage name
- `Terminal Psi`
- checked or lowered trees
- internal contracts
- crate paths
- representation identifiers
- source-language terminal transfers/outcomes
- interpreted engine
- interpreted work limit
- positive-u64
- work semantics
- `--both` comparison
- wire identities
- `ExactRule`
- rule registry
- applicability
- preservation argument
- behavioral equivalence
- `redundant-call-markers`
- `suspend` / `block` markers
- acknowledgement contract
- checked envelope
- axis
- generic body
- callback upper bounds
- synchronous instantiation
- conservative requirements
- call syntax
- ordinary obligations
- declared checking context
- required selections
- optimizer's predicted target
- writable source set
- dependency stores
- source versions
- token spans
- staging
- logical transaction
- durable recovery
- edit set
- CI result
- independent stage-output validator
- deterministic registered set
- assurance profile
- compilation stages
- admission
- invocation error
- target/product requirements
- enumerated set
- compile report
- `--report-file`
- compiler identity
- roster
- compiler invariant failures
- invalid authored source
- unavailable checks
- retained package trust acceptance
- retained source pins
- verification placement
- source-to-executable correspondence
- redundant and required markers
- narrowing refinements
- legal positions
- invariant windows
- context floors
- permitted extras
- idempotence
- marker admission
- source cleanup
- validator tooling

CHAPTER COMPLETE
