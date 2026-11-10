# Validation subjects

Source: `wiki/spec/build/validation_subjects.md` lines 1–102 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| VALIDATION_SUBJECTS1 | "A check fully validates the packages selected as its subjects and freshly checks the complete semantic dependency closure those declarations require." | Validation subjects | A check must fully validate subject packages and freshly validate the full semantic dependency closure their declarations require. | not stated | 9 |
| VALIDATION_SUBJECTS2 | "Everything else is outside the check's claim." | Validation subjects | No claim may extend beyond subjects and their required closure. | not stated | 9 |
| VALIDATION_SUBJECTS3 | "every declaration of the subject packages is valid, and every declaration they depend on is valid." | Validation subjects | A passed check must mean every subject-package declaration and every declaration those depend on are valid. | not stated | SHARED |
| VALIDATION_SUBJECTS4 | "The subject set is decided by the request, not by where source lives:" | Subjects | Subject packages must be determined only from the check or build request, not from source location. | not stated | 9 |
| VALIDATION_SUBJECTS5 | "Check or build an application or package | That package" (table row) | Check or build of an application or package must treat that package as the subject. | not stated | 9 |
| VALIDATION_SUBJECTS6 | "Check a workspace with no member named | Every declared member" (table row) | Workspace check with no named member must treat every declared member as a subject. | not stated | 9 |
| VALIDATION_SUBJECTS7 | "Build or check one workspace member | That member" (table row) | Build or check of one workspace member must treat that member as the subject. | not stated | 9 |
| VALIDATION_SUBJECTS8 | "Admit, update or audit a package revision | That revision" (table row) | Admit, update, or audit must treat that package revision as the subject. | not stated | 9 |
| VALIDATION_SUBJECTS9 | "Any of the above with `Build::validate` rows | The product plus each named dependency" (table row) | When `Build::validate` rows are present, subjects must include the product plus each named dependency from those rows (in addition to the row above). | not stated | 9 |
| VALIDATION_SUBJECTS10 | "Being bundled with the toolchain, vendored, located in the workspace or editable does not make a package a subject." | Subjects | Bundling, vendoring, workspace location, or editability must not by itself make a package a subject. | not stated | 9 |
| VALIDATION_SUBJECTS11 | "A dependency is a dependency." | Subjects | Dependencies must not be treated as subjects unless the request rules say otherwise. | not stated | 9 |
| VALIDATION_SUBJECTS12 | "The closure starts from every declaration of every subject, including unused ones, plus the Build and preparation entries the compile executes, the product entries, and the obligations of the Build's selections." | Closure | Semantic closure must start from all subject declarations (including unused), Build and preparation entries executed, product entries, and Build-selection obligations. | not stated | 9 |
| VALIDATION_SUBJECTS13 | "From there it follows calls, callable values, callbacks, initialization and cleanup, types and their invariants, contracts, proofs and lemmas, selected conformances with their required operations and laws, layouts, and target bindings." | Closure | Closure must follow all listed dependency kinds from those seeds. | not stated | SHARED |
| VALIDATION_SUBJECTS14 | "An unused subject declaration that depends on an invalid dependency declaration fails the check." | Closure | If an unused subject declaration depends on an invalid dependency declaration, the check must fail. | not stated | SHARED |
| VALIDATION_SUBJECTS15 | "Target obligations are exactly what the Build's selections commit to: each declared target row demands its selected closure, including helpers and callbacks, under [provider selection](provider_selection.md#build-owned-target-selection)." | Closure | Target obligations must match Build selections: each target row's selected closure including helpers and callbacks per provider selection rules. | not stated | 9 |
| VALIDATION_SUBJECTS16 | "Sharing a target directory is not a dependency." | Closure | Shared target directory must not be treated as a semantic dependency edge. | not stated | 9 |
| VALIDATION_SUBJECTS17 | "The subject is request information and survives every internal re-rooting." | Orchestration | Subject set must remain the request's subjects through internal re-rooting. | not stated | 9 |
| VALIDATION_SUBJECTS18 | "Executing a dependency's Build demands that Build's closure and nothing more;" | Orchestration | Running a dependency's Build must check only that Build's closure, not extra packages. | not stated | 9 |
| VALIDATION_SUBJECTS19 | "it never promotes the dependency to a subject." | Orchestration | Executing a dependency's Build must not promote that dependency to a subject of the consumer check. | not stated | 9 |
| VALIDATION_SUBJECTS20 | "Generated source belongs to the package that owns it and enters a check the way that package does." | Orchestration | Generated source must be attributed to its owning package and enter checks under that package's subject rules. | not stated | 9 |
| VALIDATION_SUBJECTS21 | "establish the declaration environment needed for correct name resolution, then check demanded declarations and add dependencies as checking discovers them." | Orchestration | Checking must establish declaration environment for name resolution before demanded declarations; dependencies enter as discovered. | not stated | SHARED |
| VALIDATION_SUBJECTS22 | "A deferred body stays in name resolution, so deferral cannot remove an overload candidate, hide an ambiguity or change what a name denotes." | Orchestration | Deferral must leave bodies in name resolution only; overload set, ambiguity, and name denotation must not change due to deferral. | not stated | 7 |
| VALIDATION_SUBJECTS23 | "A demanded declaration receives every ordinary check: a generic body under its requirements, a selected conformance with its laws, a lemma with valid evidence." | Orchestration | Each demanded declaration must get full ordinary checking (generics under requirements, conformances with laws, lemmas with valid evidence). | not stated | SHARED |
| VALIDATION_SUBJECTS24 | "Closure completeness is the correctness obligation of this rule: no fact used to accept a product may come from an unchecked declaration." | Orchestration | Acceptance must not use facts from any declaration outside the checked closure. | not stated | SHARED |
| VALIDATION_SUBJECTS25 | "`Build::validate` records a validation requirement: the named registered dependency is a subject of every check of this product." | Build::validate | Each `Build::validate` call must add the named registered dependency as a subject of every check of this product. | at a call | 9 |
| VALIDATION_SUBJECTS26 | "It only widens the subject set." | Build::validate | `Build::validate` must only expand subjects, not narrow or replace them. | at a call | 9 |
| VALIDATION_SUBJECTS27 | "The argument is a dependency this Build registered directly; a transitive dependency must be registered to be named." | Build::validate | `Build::validate` operand must be a dependency registered directly on this Build; transitive deps require their own registration to be named. | at a call | 9 |
| VALIDATION_SUBJECTS28 | "Ownership follows the root Build activation: a helper acting on the root's supplied `Build` may add subjects, a dependency's separate Build activation cannot enlarge its consumer's set." | Build::validate | Only helpers on the root activation's `Build` may widen subjects; a dependency's separate Build activation must not enlarge the consumer's subject set. | not stated | 9 |
| VALIDATION_SUBJECTS29 | "The requirement is satisfied by the ordinary checking pipeline before success is reported;" | Build::validate | `Build::validate` requirements must be satisfied by ordinary checking before reporting success. | not stated | 9 |
| VALIDATION_SUBJECTS30 | "it does not launch a compilation from inside Build execution." | Build::validate | Build execution must not start an inner compilation to satisfy validate requirements. | not stated | 9 |
| VALIDATION_SUBJECTS31 | "The check report lists its subjects." | Build::validate | Check reports must list the subject set. | not stated | 9 |
| VALIDATION_SUBJECTS32 | "A package revision is checked in full whenever it is a subject: during its own development, in a workspace check, and when a project admits, updates or audits it." | Admission and release | Whenever a revision is a subject, it must receive full-package checking in development, workspace check, and admit/update/audit. | not stated | 9 |
| VALIDATION_SUBJECTS33 | "Full-package validation covers the revision's complete declared inventory and the obligations its declarations state, including unused declarations;" | Admission and release | Full-package validation must cover the entire declared inventory and stated obligations, including unused declarations. | not stated | SHARED |
| VALIDATION_SUBJECTS34 | "an invalid declaration is a validation failure, not a discretionary acceptance choice." | Admission and release | Invalid declarations must fail validation; acceptance must not be discretionary. | not stated | SHARED |
| VALIDATION_SUBJECTS35 | "Admission validates the revision being admitted;" | Admission and release | Admission must validate the revision being admitted, not only the consumer's edits. | not stated | 9 |
| VALIDATION_SUBJECTS36 | "a consumer's edit does not promote its unchanged dependencies to subjects." | Admission and release | Consumer edits must not promote unchanged dependencies to subjects. | not stated | 9 |
| VALIDATION_SUBJECTS37 | "A corpus of programs is not a full-package check." | Admission and release | Corpus runs must not be treated as full-package validation. | not stated | 9 |
| VALIDATION_SUBJECTS38 | "The toolchain's own release validation checks each bundled library as a subject." | Admission and release | Toolchain release validation must check each bundled library as a subject. | not stated | 9 |
| VALIDATION_SUBJECTS39 | "No result is reused: a consumer checks every dependency declaration it needs afresh," | What this rule does not do | Consumers must recheck every needed dependency declaration; prior results must not be reused. | not stated | 9 |
| VALIDATION_SUBJECTS40 | "the full-package gate is a claim about the library, not evidence a consumer consumes in place of checking." | What this rule does not do | Full-package success must not substitute for the consumer's own checking of needed declarations. | not stated | 9 |
| VALIDATION_SUBJECTS41 | "`--verify` withholds trust acceptance and re-verifies under retained source pins; it does not widen the subject set." | What this rule does not do | `--verify` must withhold trust acceptance and re-verify under pins without widening subjects. | not stated | 9 |
| VALIDATION_SUBJECTS42 | "Optimization settings change how a valid program is realized, never which declarations a check claims." | What this rule does not do | Optimization must not change which declarations a check claims to validate. | not stated | 9 |
| VALIDATION_SUBJECTS43 | "There is no setting that narrows a subject below its full declaration set." | What this rule does not do | No setting may limit a subject to fewer than all its declarations. | not stated | 9 |
| VALIDATION_SUBJECTS44 | "Checked-tree pruning still runs only after the subjects and their closure are checked, within that declared scope." | What this rule does not do | Checked-tree pruning must run only after subjects and closure are checked, staying within declared scope. | not stated | 9 |

## Ambiguous sentences (both readings)

| quoted sentence | reading A | reading B |
| --- | --- | --- |
| "A check fully validates the packages selected as its subjects and freshly checks the complete semantic dependency closure those declarations require." | "Freshly checks" means no cached validation results from prior runs may be used for closure members. | "Freshly checks" means the closure is computed and checked in the same run as the subjects, without implying anything about cross-run caching policy. |
| "Everything else is outside the check's claim." | Packages not in the subject set or closure are not validated at all during the check. | Packages outside subjects/closure may still be touched for tooling, but success does not certify their declarations. |
| "A passed check therefore means exactly: every declaration of the subject packages is valid, and every declaration they depend on is valid." | "Depend on" is limited to declarations reached by the closure rules in this chapter. | "Depend on" includes every declaration in the semantic dependency graph, even if the closure section would not list some edges. |
| "Check a workspace with no member named | Every declared member" (table row) | "No member named" means the request does not name a single workspace member filter. | "No member named" means the workspace literally has no members with names (empty or unnamed-member edge case). |
| "Any of the above with `Build::validate` rows | The product plus each named dependency" (table row) | Subjects are the union of the prior row's subjects plus each `Build::validate` dependency. | Subjects replace the single-package row with product plus validate rows only (not both). |
| "The closure starts from every declaration of every subject, including unused ones, plus the Build and preparation entries the compile executes, the product entries, and the obligations of the Build's selections." | "Preparation entries the compile executes" are only those on the executed path for this check. | All preparation entries declared on the Build are seeds regardless of whether this compile executes them. |
| "From there it follows calls, callable values, callbacks, initialization and cleanup, types and their invariants, contracts, proofs and lemmas, selected conformances with their required operations and laws, layouts, and target bindings." | Closure is transitive closure along all listed edge kinds until fixpoint. | Closure includes only one hop along each kind unless further hops are implied elsewhere. |
| "An unused subject declaration that depends on an invalid dependency declaration fails the check." | Failure requires a direct dependency edge from the unused declaration to the invalid declaration. | Failure occurs if any unused subject declaration is part of a chain that eventually references an invalid declaration in the closure. |
| "Target obligations are exactly what the Build's selections commit to" | No target-related obligations outside explicit Build selection rows enter closure. | Target obligations may include implicit obligations if they are committed by selection semantics defined in provider selection. |
| "Sharing a target directory is not a dependency." | Two packages with the same output directory do not acquire mutual semantic closure obligations. | Artifacts colocated in a directory must not create package-graph edges; build scheduling may still share paths. |
| "The subject is request information and survives every internal re-rooting." | Internal compiler re-rooting of paths or roots must not change which packages are subjects. | Subject set is fixed at request parse time and cannot change mid-pipeline for any re-rooting reason. |
| "Executing a dependency's Build demands that Build's closure and nothing more;" | Only the dependency Build's own closure is checked when that Build runs. | Consumer check must not validate more than the dependency Build's closure when invoking it, but may check less if scheduling defers work. |
| "Generated source belongs to the package that owns it and enters a check the way that package does." | Generated files are subject to the same subject/dependency rules as hand-written files of the owning package. | Generated source is checked only when the owning package is a subject, never when only a dependency. |
| "add dependencies as checking discovers them." | Dependencies enter closure incrementally during checking, possibly after initial subject pass. | Dependencies are discovered for scheduling order but closure must still be complete before acceptance. |
| "A deferred body stays in name resolution, so deferral cannot remove an overload candidate, hide an ambiguity or change what a name denotes." | Deferred checking is a phase separate from body type-checking but shares one overload resolution environment. | Deferral only delays body checks; overload/ambiguity resolution at use sites must match as-if all bodies were present. |
| "no fact used to accept a product may come from an unchecked declaration." | "Product" means the built executable artifact only. | "Product" means any successful outcome of the check request (library, test, or executable). |
| "a dependency's separate Build activation cannot enlarge its consumer's set." | Validate calls inside dependency Build scripts never affect consumer subjects. | No Build activation except root helpers on the supplied Build can widen subjects, including nested consumer builds. |
| "The requirement is satisfied by the ordinary checking pipeline before success is reported;" | Build::validate is a no-op at runtime if checking already included those deps as subjects via closure. | Build::validate forces those packages to be subjects even if closure would not have required full-package treatment. |
| "it does not launch a compilation from inside Build execution." | Build script evaluation must not spawn a nested `omega check` subprocess. | Build script must not trigger a separate product compile job; incremental checking during the same session may still run. |
| "Full-package validation covers the revision's complete declared inventory and the obligations its declarations state, including unused declarations;" | Every syntactic declaration unit in the package root inventory must be checked. | Only declarations admitted into the package's manifest inventory count, not every file on disk. |
| "a consumer's edit does not promote its unchanged dependencies to subjects." | Editing consumer source does not turn dependencies into full-package subjects unless request rules say so. | Bumping a consumer lockfile without changing dependency sources does not subject those dependencies to admission-level full check. |
| "A corpus of programs is not a full-package check." | Running many fixtures does not satisfy admission full-package validation for a library revision. | Corpus may check programs without validating every unused declaration in each library they import. |
| "No result is reused: a consumer checks every dependency declaration it needs afresh," | Cross-package artifact trust or build cache cannot skip re-validation of needed declarations. | Within one consumer check, internal memoization of the same declaration is allowed if it is still "fresh" in the sense of this chapter. |
| "`--verify` withholds trust acceptance and re-verifies under retained source pins; it does not widen the subject set." | `--verify` re-runs verifiers on the same subjects with stricter trust rules. | `--verify` may change verification depth but cannot add packages to subjects beyond the base check. |
| "Optimization settings change how a valid program is realized, never which declarations a check claims." | Different `-O` levels must not skip checking declarations that a lower level would check. | Optimizations may remove dead code at lowering time but the check claim set is fixed before optimization. |
| "There is no setting that narrows a subject below its full declaration set." | Partial-module or single-file check modes cannot be used to certify a package while skipping declarations. | Settings may skip work on non-subjects but cannot declare a subject while checking only a subset of its declarations. |
| "Checked-tree pruning still runs only after the subjects and their closure are checked, within that declared scope." | Pruning removes unchecked files from outputs only post-validation. | Pruning may delete dead declarations from the checked tree after validation without re-opening subject narrowing. |

## Terms used without definition in this chapter

- check
- subject packages
- semantic dependency closure
- declaration
- request
- application
- package
- workspace
- declared member
- package revision
- admit
- update
- audit
- toolchain
- vendored
- editable
- dependency
- Build
- preparation entries
- product entries
- Build's selections
- calls
- callable values
- callbacks
- initialization and cleanup
- invariants
- contracts
- proofs
- lemmas
- selected conformances
- laws
- layouts
- target bindings
- target row
- helpers
- provider selection
- target directory
- internal re-rooting
- dependency's Build
- generated source
- declaration environment
- name resolution
- demanded declaration
- deferred body
- overload candidate
- ambiguity
- generic body
- requirements
- selected conformance
- valid evidence
- fact
- product
- unchecked declaration
- Build::validate
- validation requirement
- registered dependency
- transitive dependency
- root Build activation
- helper
- consumer
- ordinary checking pipeline
- compilation
- Build execution
- check report
- full-package validation
- complete declared inventory
- discretionary acceptance
- corpus of programs
- release validation
- bundled library
- result reuse
- full-package gate
- evidence
- `--verify`
- trust acceptance
- source pins
- optimization settings
- valid program
- full declaration set
- Checked-tree pruning
- declared scope

CHAPTER COMPLETE
