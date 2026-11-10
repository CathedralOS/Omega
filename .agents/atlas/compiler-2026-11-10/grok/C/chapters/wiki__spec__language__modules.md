# Modules, imports, and visibility

Source: `wiki/spec/language/modules.md` lines 1–193 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
MODULES1 | "A source file belongs to its package by location; it does not repeat a package declaration." | Modules, imports, and visibility | Assign each source file to exactly one package by filesystem location; reject or ignore redundant package declarations in source | not stated | SHARED
MODULES2 | "Module names organize declarations within that boundary and participate in name/artifact identity." | Modules, imports, and visibility | Module paths must stay within the file's package boundary and feed name and artifact identity | not stated | SHARED
MODULES3 | "`::` selects a static name; `.` selects a field through a value." | Names and imports | Distinguish `::` static name selection from `.` value field projection in resolution | at the operation | SHARED
MODULES4 | "A field projection does not select a machine or insert a receiver argument." | Names and imports | Field projection must not resolve to a machine or synthesize a receiver argument | at the operation | 3
MODULES5 | "Imports select logical names, never filesystem paths, and do not execute code." | Names and imports | `use` must resolve logical names only; must not treat paths as imports or execute imported packages | at a call | SHARED
MODULES6 | "External selection requires a requester-local direct dependency and the target declaration's visibility." | Names and imports | Reject external name selection without a direct dependency edge in the requester and without visibility on the target declaration | at the operation | SHARED
MODULES7 | "Fully qualified spelling and explicit arguments obey the same rule." | Names and imports | Fully qualified paths and explicit selection arguments must satisfy the same dependency and visibility checks as other external selection | at the operation | SHARED
MODULES8 | "Tooling may discover an enclosing package; source cannot walk parent directories to gain undeclared reach." | Names and imports | Source resolution must not ascend parent directories to reach names; undeclared reach via filesystem walk is forbidden | not stated | SHARED
MODULES9 | "A logical `use dependency;` may remain deferred until explicit dependency registration and preparation make its exact declaration scope available to a subsequent checked continuation." | Names and imports | Allow deferring resolution of `use dependency;` only until registered prepared dependency exposes declaration scope to the next checked continuation | not stated | 9
MODULES10 | "Unused deferred imports normally warn without acquisition or execution;" | Names and imports | Emit warning for unused deferred imports without acquiring or executing the dependency | not stated | 9
MODULES11 | "references in types and contracts still require resolution before their checked use." | Names and imports | Resolve every name referenced in types and contracts before checked use even when the import was deferred | before checked use | SHARED
MODULES12 | "A later request cannot justify earlier code." | Names and imports | Reject earlier source that used unresolved or unprepared external names even if a later registration would have enabled them | not stated | SHARED
MODULES13 | "Alias/source/purpose/context registration is atomic and immutable, with no post-registration retarget or implicit compiler-filesystem fallback." | Names and imports | Dependency registration must be atomic and immutable; reject retargeting aliases or implicit filesystem fallback after registration | not stated | 9
MODULES14 | "`use name from Source::Path { ... };` is not an acquisition/import surface." | Names and imports | Treat `use name from Source::Path { ... }` as not performing acquisition or ordinary import | at a call | 9
MODULES15 | "Ordinary product source retains complete name resolution before type/proof checking its consumers;" | Names and imports | Product source must fully resolve names before type and proof checking downstream consumers | before checked use | SHARED
MODULES16 | "this Build staging rule is not lazy unchecked product execution or declaration identity invented from matching signatures." | Names and imports | Build staged imports must not defer to lazy unchecked product execution or fabricate declaration identity from signature match | not stated | 9
MODULES17 | "Build entry/local-helper imports use build dependencies; product and generated-product imports use ordinary product dependencies." | Names and imports | Classify imports by context: build-entry or local-helper use build dependency graph; product and generated-product use product dependency graph | at the operation | 9
MODULES18 | "The same alias can have distinct meanings in those scopes, with no fallback." | Names and imports | Resolve the same alias separately per scope without cross-scope fallback | at the operation | 9
MODULES19 | "A host library uses its own ordinary edges for implementation code." | Names and imports | Host library implementation imports must use that library's ordinary product dependency edges only | not stated | 9
MODULES20 | "source acquisition alone grants neither scope." | Names and imports | Acquiring source must not by itself grant build or product checked import scope | not stated | 9
MODULES21 | "Only designated toolchain product-reference operands resolve product declarations during build execution." | Names and imports | During build execution, resolve product declarations only through designated product-reference operands | at the operation | 9
MODULES22 | "Host imports, ordinary calls, and generic helpers do not acquire that exception." | Names and imports | Host imports, ordinary calls, and generic helpers must not use the product-reference exception during build execution | at the operation | 9
MODULES23 | "Carrying does not add a direct dependency or grant selection authority." | Names and imports | Type carry through a dependency API must not imply a new direct dependency or authored selection authority | not stated | SHARED
MODULES24 | "Imports affect the importing source file, not sibling files that declare the same logical module." | Import scope and exposure | Apply each file's imports only within that file, not to siblings sharing a logical module | not stated | SHARED
MODULES25 | "Import exposure is not transitive: another module's imports do not become this file's imports." | Import scope and exposure | Do not re-export or inherit another module's imports into the current file | not stated | SHARED
MODULES26 | "A package's source files that declare no module form one logical module: each sees its siblings' declarations without an import, while each file's imports remain its own." | Import scope and exposure | Unmoduled package files share one logical module for sibling declarations; per-file imports stay local | not stated | SHARED
MODULES27 | "A file's own declarations and its imports take precedence over a sibling's same-spelled declaration." | Import scope and exposure | On name clash, prefer the file's own declarations and its imports over a sibling's same-spelled declaration | at the operation | SHARED
MODULES28 | "Import a declaring module | Its directly declared public domains." | Import scope and exposure | Module import exposes only the module's directly declared public domains for carrier-qualified lookup | at the operation | SHARED
MODULES29 | "Import one domain declaration | That exact domain." | Import scope and exposure | Single-domain import exposes only that exact domain for carrier-qualified lookup | at the operation | SHARED
MODULES30 | "Import one ordinary machine | No sibling domains." | Import scope and exposure | Machine-only import must not expose sibling domains | at the operation | SHARED
MODULES31 | "Import a carrier type | No foreign domains merely because they attach to it." | Import scope and exposure | Carrier type import must not expose attached foreign domains without their own selection | at the operation | SHARED
MODULES32 | "Load a dependency or another source file | None merely because it was loaded." | Import scope and exposure | Loading a dependency or file must not by itself expose foreign domains for carrier-qualified lookup | not stated | SHARED
MODULES33 | "A module import does not recursively expose domains from descendant modules or modules it imports." | Import scope and exposure | Module import must not recursively expose domains from nested or imported modules | at the operation | SHARED
MODULES34 | "Repeated selection of one exact declaration adds no competing candidate." | Import scope and exposure | Duplicate import of the same declaration must not create an ambiguity from competing candidates | at the operation | SHARED
MODULES35 | "Dependencies resolve their bodies under their own source imports; consumer imports cannot reinterpret already selected declarations." | Import scope and exposure | Resolve dependency bodies with their own imports; consumer imports must not change meaning of already-selected foreign declarations | not stated | SHARED
MODULES36 | "An exact foreign-domain address begins with its declaring package/module, not the carrier owner's namespace:" | Foreign attached declaration paths | Foreign domain paths must start with the declaring package or module, not the carrier owner's namespace | at the operation | SHARED
MODULES37 | "The attachment is resolved in that declaration's context to the exact carrier, for example geometry's `Point`; a caller's same-spelled type cannot redirect it." | Foreign attached declaration paths | Resolve carrier attachment in the declaring context; reject caller same-spelled types redirecting the carrier | at the operation | SHARED
MODULES38 | "The domain and carrier retain independent owners." | Foreign attached declaration paths | Track separate owners for domain and carrier | not stated | SHARED
MODULES39 | "A module import such as `use ui_policy::screen;` exposes the module's directly declared public domains instead of just this one." | Foreign attached declaration paths | Module import exposes all directly declared public domains of the module, not only one attached path | at the operation | SHARED
MODULES40 | "Short carrier-qualified selection must match both the resolved carrier and an exposed domain." | Foreign attached declaration paths | Reject short carrier-qualified names unless both carrier resolution and domain exposure succeed | at the operation | SHARED
MODULES41 | "Direct qualified selection names only the exact declaration and does not expose siblings." | Foreign attached declaration paths | Fully qualified selection binds one declaration and must not expose sibling declarations | at the operation | SHARED
MODULES42 | "All selections retain ordinary dependency and visibility requirements; a declaration path is not a grant to select a private carrier." | Foreign attached declaration paths | Every selection path still requires dependency and visibility; path syntax must not bypass private carrier rules | at the operation | SHARED
MODULES43 | "Importing a domain permits name selection, not membership, minting authority, access to private representation, or an implicit change of operators." | Foreign attached declaration paths | Domain import grants lookup only; must not imply membership, minting, private representation access, or operator changes | at the operation | SHARED
MODULES44 | "A value may carry an exact foreign qualification and its evidence through an API without importing that domain for authored lookup." | Foreign attached declaration paths | Allow carrying qualified values and evidence without a domain import for authored selection | not stated | SHARED
MODULES45 | "Carrying neither erases that evidence nor exposes other declarations of the domain's package." | Foreign attached declaration paths | Carried qualification must preserve evidence and must not expose other package declarations | not stated | SHARED
MODULES46 | "1. Local bindings." | Resolution | Try local bindings first in name resolution | at the operation | SHARED
MODULES47 | "2. State parameters." | Resolution | Try state parameters second in name resolution | at the operation | 10
MODULES48 | "3. Machine parameters." | Resolution | Try machine parameters third in name resolution | at the operation | 10
MODULES49 | "4. Imported names." | Resolution | Try imported names fourth in name resolution | at the operation | SHARED
MODULES50 | "5. Fully qualified package/module paths within the occurrence's declared dependency scope." | Resolution | Try fully qualified paths within declared dependency scope last | at the operation | SHARED
MODULES51 | "Ambiguous imported declarations reject." | Resolution | Reject when multiple imported declarations match | at the operation | SHARED
MODULES52 | "Compiler traversal order cannot select between them." | Resolution | Must not break import ambiguity by traversal order | at the operation | SHARED
MODULES53 | "A field is selected through an explicit value such as `context.field`; neither a machine's qualified name nor a parameter's type introduces bare aliases for its fields." | Resolution | Require explicit value for field selection; forbid bare field aliases from machine qualification or parameter types | at the operation | 1
MODULES54 | "Case members are not an additional bare-name resolution step." | Resolution | Do not resolve bare identifiers to case members as a separate resolution pass | at the operation | 1
MODULES55 | "Value position requires the carrier-qualified `Type::Case` path even when an expected type or visible carrier would make one case appear likely." | Resolution | In value position require explicit `Type::Case`; reject inferring case from expected type alone | at the operation | 1
MODULES56 | "Lexical bindings keep their ordinary meaning; the compiler does not reinterpret an unresolved single-segment name as a case." | Resolution | Unresolved single-segment names must not be reinterpreted as cases when a binding failed | at the operation | 1
MODULES57 | "Distinct visible declarations competing for the same carrier-qualified domain or case, or for one module/trait/implementation machine path, reject." | Resolution | Reject when distinct visible declarations compete for the same qualified domain, case, or module/trait/impl path | at the operation | SHARED
MODULES58 | "Neither carrier ownership nor import order selects a winner." | Resolution | Must not use carrier ownership or import order to break resolution ties | at the operation | SHARED
MODULES59 | "Diagnostics retain both declaration owners, the exact carrier, and the imports exposing the conflict." | Resolution | Ambiguity diagnostics must name both owners, the carrier, and exposing imports | not stated | SHARED
MODULES60 | "Independently nameable data, domains, traits, machines, wire schemas, operators, constants, and complete named conformances are private unless marked `public`." | Visibility | Treat listed declaration kinds as private default; require `public` marker for external nameability | not stated | SHARED
MODULES61 | "Carrier qualification does not inherit visibility." | Visibility | Visibility of a carrier type must not flow to carrier-qualified names automatically | not stated | SHARED
MODULES62 | "Fields, cases, and trait requirements instead inherit their exact semantic owner's visibility." | Visibility | Fields, cases, and trait requirements use their semantic owner's visibility | not stated | SHARED
MODULES63 | "A data-attached domain head independently selects its carrier and therefore needs authority to name that carrier." | Visibility | Data-attached domain heads must have authority to name their selected carrier | at the operation | 8
MODULES64 | "A machine head cannot attach to a data carrier; its argument types and code-owning namespace retain their ordinary independent declaration selections." | Visibility | Reject machine head attached to data carrier; machine args and namespace keep independent selection rules | at the operation | 10
MODULES65 | "Nested machines are lexical helpers, not independently package-exportable declarations." | Visibility | Nested machines must not be independently package-exportable | not stated | 10
MODULES66 | "An enclosing machine's export does not export its descendants, and qualification cannot bypass their lexical scope." | Visibility | Enclosing machine `public` must not export nested machines; qualification must not escape nested lexical scope | not stated | 10
MODULES67 | "Source nesting affects visibility, not data ownership or implicit runtime arguments." | Visibility | Nesting changes visibility only; must not change ownership or inject implicit runtime arguments | not stated | 10
MODULES68 | "Ranking measures are private proof machinery; `public measure` rejects." | Visibility | Reject `public` on ranking measures; keep measures private proof machinery | not stated | 8
MODULES69 | "A public termination guarantee does not publish its implementation's ranking witness." | Visibility | Public termination must not expose private ranking witness from implementation | not stated | 8
MODULES70 | "Requirement-local `as Name` labels have no standalone package visibility." | Visibility | `as Name` labels in requirements must not be selectable as package-level names | not stated | SHARED
MODULES71 | "Public interfaces cannot select private declarations as consumer-nameable API." | Visibility | Reject public API surface that names private declarations for consumers | not stated | SHARED
MODULES72 | "An authorized public conformance may retain private implementation identities without granting consumers direct selection of them." | Visibility | Allow private impl identities behind public conformance but forbid consumer direct selection of those identities | not stated | SHARED
MODULES73 | "A public domain's issuer catalog may likewise retain private trait requirements or exact machines accessible to its author." | Visibility | Public domain issuer catalog may keep private trait requirements and machines for author only | not stated | 8
MODULES74 | "The same limited exception applies to exact reader/writer descriptors in its access policy." | Visibility | Access policy may keep reader/writer descriptors private with same limited exception | not stated | 8
MODULES75 | "That authorization metadata grants no consumer call, conformance, or private-type selection permission." | Visibility | Issuer or access metadata must not grant consumers call, conformance, or private-type selection rights | not stated | SHARED
MODULES76 | "Other public signatures and predicates retain their ordinary exposure rules; this is not general public access to private names." | Visibility | Do not generalize limited private-retention exceptions into broad public access to private names | not stated | SHARED
MODULES77 | "There is no `export` item relabeling dependency-owned declarations." | Visibility | Must not provide `export` item syntax to relabel foreign declarations | not stated | SHARED
MODULES78 | "`export` is not reserved." | Visibility | `export` identifier is not reserved in the language | not stated | SHARED
MODULES79 | "Ordinary public wrappers expose behavior under the wrapper owner's API." | Visibility | Public wrapper exports behavior under the defining owner, not the wrapped dependency owner | not stated | SHARED
MODULES80 | "Public lookalikes cannot create compiler intrinsics or acquire their availability." | Visibility | Reject public declarations that mimic intrinsics to acquire builtin availability | not stated | SHARED
MODULES81 | "Publishing proof vocabulary/contracts does not establish every application or accept the assumptions of a bodyless claim." | Visibility | Publishing proof vocabulary must not automatically establish applications or accept bodyless claim assumptions | not stated | 8
MODULES82 | "Publishing structural data publishes its field names and shape." | Public structural data | `public` on structural data exposes field names and shape to consumers | not stated | 1
MODULES83 | "Authorized consumers may read, construct, and update values subject to field types, invariants, borrowing, and qualification requirements, including domain-owned read/write policy." | Public structural data | Field access still subject to types, invariants, borrowing, qualification, and domain read/write policy | at the operation | 2
MODULES84 | "Knowing a field name grants no access that its qualified type withholds." | Public structural data | Field name visibility must not override access withheld by the field's qualified type | at the operation | 2
MODULES85 | "Those restrictions are opt-in domain clauses, not field visibility or privileges for every machine in a file, namespace, or package." | Public structural data | Additional access restrictions come only from explicit domain clauses, not implicit file or package privileges | not stated | 8
MODULES86 | "Omitting them adds no access restriction beyond ordinary validity, ownership, and other authority rules." | Public structural data | Absent domain clauses must not add access restrictions beyond ordinary validity, ownership, and authority | not stated | SHARED
MODULES87 | "Visibility is not confidentiality, unforgeable authority, or an ABI policy." | Public structural data | Do not treat `public` as confidentiality, unforgeable authority, or ABI stability guarantee | not stated | SHARED
MODULES88 | "There are no private fields, by design:" | Public structural data | Reject or do not provide private fields on structural data | not stated | 1
MODULES89 | "Denying selection itself is a package-boundary property: selecting a type's fields, cases or conformances requires a direct dependency on the owning package, while carrying its values does not" | Public structural data | Require direct dependency on owning package for field, case, or conformance selection; carrying values without that dependency remains allowed | at the operation | SHARED
MODULES90 | "Constructing geometry or handle bits does not construct its routed authority or provider provenance." | Public structural data | Value construction must not construct routed authority or provider provenance | at the operation | 8
MODULES91 | "Confidential state remains in custody outside the observer's access." | Public structural data | Confidential state must stay outside observer access despite public shape | not stated | 8
MODULES92 | "Changes to public shape change source/API identity;" | Public structural data | Public shape changes must change recorded source and API identity | not stated | SHARED
MODULES93 | "independent binary compatibility and replacement follow explicit component contracts." | Public structural data | Binary compatibility and replacement governed only by explicit component contracts, not visibility alone | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"This is the ordinary import rule; it does not restrict visibility of the logical module's own declarations." | File-local imports do not hide sibling-visible declarations inside the shared logical module. | Imports never change whether declarations inside the module are `public` or `private` to the package. |
"Unused deferred imports normally warn without acquisition or execution;" | Unused deferred imports always produce a warning and never trigger acquisition or execution. | Warning is the default policy but another diagnostic severity or silence path may exist elsewhere. |
"Ordinary product source retains complete name resolution before type/proof checking its consumers;" | Every consumer's checking waits until all names in the product closure resolve. | Each compilation unit resolves completely before its own type/proof pass; cross-unit ordering among consumers is not stated. |
"A host library uses its own ordinary edges for implementation code." | Implementation code inside a host/stdlib package resolves imports only against that library's product dependency graph. | Any code labeled host library in build context uses product edges, while build-only helpers in the same tree might still use build edges. |
"Import a declaring module | Its directly declared public domains." | Table row applies only when the import target is a module declaration node. | Table row applies when any import path resolves to a module namespace, including re-exports not stated here. |
"Load a dependency or another source file | None merely because it was loaded." | Parsing or loading for the build graph never adds carrier-qualified domain exposure without an import. | Loading for type-checking a dependency body does not expose its domains to the consumer's lookup table. |
"Short carrier-qualified selection must match both the resolved carrier and an exposed domain." | A two-segment `Carrier::Domain` form requires prior exposure of that domain through import rules. | Short form is permitted whenever carrier and domain resolve, and exposure is checked only at the domain segment. |
"Authorized consumers may read, construct, and update values subject to field types, invariants, borrowing, and qualification requirements" | `public` structural data grants these operations to any consumer with package visibility and satisfied authority. | Only consumers with separate authorization (domain policy, proofs) may perform these operations despite public shape. |
"There are no private fields, by design:" | The language rejects a `private` field modifier on structural data. | Fields have no visibility attribute; access control is entirely via types and plans, not a distinct private-field feature. |
"Denying selection itself is a package-boundary property: selecting a type's fields, cases or conformances requires a direct dependency on the owning package, while carrying its values does not" | Direct dependency is required only for authored selection syntax, not for uses that merely mention carried types. | Carrying values through generic APIs might still require dependency when the type appears in signatures; only value transport without selection is exempt. |
"Changes to public shape change source/API identity;" | Any public field add/remove/rename changes nominal API identity for consumers. | Only changes that alter the published public surface count; internal layout details with unchanged public field set might not change identity. |

## Terms used without definition in this chapter

- Build declarations / package directory / role / source lineage / dependency graph
- package declaration
- name/artifact identity
- name_path / module_decl / import_decl
- static name / field projection / receiver argument
- logical names / filesystem paths
- requester-local direct dependency
- tooling / enclosing package / undeclared reach
- checked staged imports / checked continuation
- `use dependency;` / deferred import / acquisition / execution
- alias/source/purpose/context registration / compiler-filesystem fallback
- `use name from Source::Path { ... }`
- type/proof checking / declaration identity / matching signatures
- build dependencies / product dependencies / generated-product
- host library / ordinary edges
- scoped build execution / two checked contexts
- product-reference operands / build execution
- package boundary / authored selection / carrying / nominal type / selection authority
- logical module / sibling files / import exposure
- carrier-qualified lookup / foreign domains
- declaring module / domain declaration / ordinary machine / carrier type
- descendant modules / competing candidate
- foreign-domain address / carrier owner / attachment / short carrier-qualified selection / direct qualified selection
- membership / minting authority / private representation / operators / evidence
- local bindings / state parameters / machine parameters / occurrence's declared dependency scope
- traversal order / bare aliases
- case members / carrier-qualified `Type::Case` path / lexical bindings
- module/trait/implementation machine path / declaration owners
- independently nameable data / wire schemas / complete named conformances
- carrier qualification / semantic owner
- data-attached domain head / machine head / data carrier / code-owning namespace
- nested machines / package-exportable / enclosing machine's export / lexical scope
- ranking measures / ranking witness / requirement-local `as Name` labels
- public conformance / implementation identities / issuer catalog / private issuer routes
- reader/writer descriptors / access policy / authorization metadata
- `export` item / public wrappers / public lookalikes / compiler intrinsics
- proof vocabulary / bodyless claim
- structural data / field names and shape / invariants / domain-owned read/write policy
- access plans / governed values
- declaration selection / fields, cases or conformances
- geometry or handle bits / routed authority / provider provenance / confidential state / custody
- source/API identity / component contracts / binary compatibility

CHAPTER COMPLETE
