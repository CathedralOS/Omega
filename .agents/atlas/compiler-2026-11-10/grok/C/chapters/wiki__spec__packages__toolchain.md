# Core, ordinary libraries, and semantic bindings

Source: `wiki/spec/packages/toolchain.md` lines 1–56 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
TOOLCHAIN1 | "`omega::language::core` is bundled as part of the language." | Core, ordinary libraries, and semantic bindings | Core must be present as bundled language component, not optional external-only | not stated | 9
TOOLCHAIN2 | "The checker depends on it," | Core, ordinary libraries, and semantic bindings | Checking must depend on bundled core, not proceed without it | not stated | 9
TOOLCHAIN3 | "its version is the language version," | Core, ordinary libraries, and semantic bindings | Core version must equal language version; reject or mismatch if they diverge | not stated | 9
TOOLCHAIN4 | "and two core versions cannot coexist in one graph." | Core, ordinary libraries, and semantic bindings | Reject resolved dependency graph containing two distinct core versions | not stated | 9
TOOLCHAIN5 | "This is explicit semantic coupling, not an implicit package default." | Core, ordinary libraries, and semantic bindings | Must not treat core as implicit package default; coupling must be explicit in semantics | not stated | 9
TOOLCHAIN6 | "Std has no equivalent privilege." | Core, ordinary libraries, and semantic bindings | Std must not receive core-like bundled or version-locked privilege | not stated | 9
TOOLCHAIN7 | "Its name, repository, lineage, path, filenames, and same-spelled declarations confer no compiler authority." | Core, ordinary libraries, and semantic bindings | Reject granting compiler authority from std name, repo, lineage, path, filenames, or same-spelled declarations alone | not stated | 9
TOOLCHAIN8 | "Freestanding programs need no std," | Core, ordinary libraries, and semantic bindings | Freestanding programs must compile or run without requiring std in the graph | not stated | 9
TOOLCHAIN9 | "and the compiler-owned Build protocol works without it." | Core, ordinary libraries, and semantic bindings | Build protocol must function without std present | not stated | 9
TOOLCHAIN10 | "Package-aware compilation admits only core's magic toolchain mount;" | Core, ordinary libraries, and semantic bindings | Reject package-aware compilation that admits magic toolchain mount for non-core packages | not stated | 9
TOOLCHAIN11 | "ordinary libraries use declared dependency edges and package-local imports." | Core, ordinary libraries, and semantic bindings | Ordinary libraries must resolve only via declared dependency edges and package-local imports | not stated | 9
TOOLCHAIN12 | "Ownership comes from that resolved graph, not from executing a library's build file." | Core, ordinary libraries, and semantic bindings | Package ownership must come from resolved graph; must not derive from executing library build file | not stated | 9
TOOLCHAIN13 | "Repeated selection of the same std instance preserves one ordinary package owner;" | Core, ordinary libraries, and semantic bindings | Repeated same std instance must map to one ordinary package owner | not stated | 9
TOOLCHAIN14 | "core preserves its exact toolchain owner." | Core, ordinary libraries, and semantic bindings | Core must keep its exact toolchain owner across selection | not stated | 9
TOOLCHAIN15 | "Neither may collapse onto an anonymous root's owner or acquire authority merely because a default build supplied it." | Core, ordinary libraries, and semantic bindings | Reject collapsing core or std onto anonymous root owner or granting authority solely because default build supplied it | not stated | 9
TOOLCHAIN16 | "preserve supplied provenance and use content identity only when none exists." | Core, ordinary libraries, and semantic bindings | Default-build owner identity preserves supplied provenance; content identity only when no provenance exists | not stated | 9
TOOLCHAIN17 | "Core is available in both checked build and product contexts." | Core, ordinary libraries, and semantic bindings | Core must be usable in checked build and product contexts | not stated | 9
TOOLCHAIN18 | "Std use in build code requires a build dependency;" | Core, ordinary libraries, and semantic bindings | Reject std use in build code without build dependency edge | not stated | 9
TOOLCHAIN19 | "product use requires an ordinary dependency;" | Core, ordinary libraries, and semantic bindings | Reject std use in product code without ordinary dependency edge | not stated | 9
TOOLCHAIN20 | "both uses require both edges." | Core, ordinary libraries, and semantic bindings | Reject std used in both build and product contexts unless both build and ordinary dependency edges present | not stated | 9
TOOLCHAIN21 | "Neither edge grants ambient host services or makes host and target type instances interchangeable." | Core, ordinary libraries, and semantic bindings | Dependency edges must not grant ambient host services or equate host and target type instances | not stated | 9
TOOLCHAIN22 | "bind its exact nominal identity and normalized schema in the accepted resolved closure." | Consumer-supplied semantic bindings | When integration or classification needs recognition, bind exact nominal identity and normalized schema in accepted resolved closure | not stated | 9
TOOLCHAIN23 | "A candidate designation only guides confined review; accepted binding comes from consumer policy." | Consumer-supplied semantic bindings | Candidate designation must not alone establish binding; accepted binding only from consumer policy after review | not stated | 9
TOOLCHAIN24 | "It grants no provider or capability and cannot be inferred from an alias or source location." | Consumer-supplied semantic bindings | Binding must not grant provider or capability; reject inferring binding from alias or source location alone | not stated | 9
TOOLCHAIN25 | "Rejoin package, exact declaration, complete schema, and any role-required selected plan." | Consumer-supplied semantic bindings | Consumer binding must rejoin package, exact declaration, complete schema, and role-required selected plan | not stated | 9
TOOLCHAIN26 | "Every supplied row settles exactly once;" | Consumer-supplied semantic bindings | Each supplied semantic-binding row must settle once only; reject duplicate settlement | not stated | 9
TOOLCHAIN27 | "foreign, stale, ambiguous, or unmatched rows reject." | Consumer-supplied semantic bindings | Reject foreign, stale, ambiguous, or unmatched supplied rows | not stated | 9
TOOLCHAIN28 | "Downstream consumers use the resolved declaration identity, not another name search." | Consumer-supplied semantic bindings | Downstream must use resolved declaration identity; reject alternate name search | not stated | 9
TOOLCHAIN29 | "Requirement-only roles cannot synthesize a provider." | Consumer-supplied semantic bindings | Reject synthesizing a provider from requirement-only roles | not stated | 9
TOOLCHAIN30 | "An interpreter routing token identifies the exact declaration in one checked program;" | Consumer-supplied semantic bindings | Routing token must identify exact declaration within one checked program | not stated | 9
TOOLCHAIN31 | "it grants no filesystem access or policy acceptance." | Consumer-supplied semantic bindings | Interpreter routing token must not grant filesystem access or policy acceptance | not stated | 9
TOOLCHAIN32 | "Package declaration visibility remains real inside core and ordinary libraries." | Consumer-supplied semantic bindings | Apply real package declaration visibility rules inside core and ordinary libraries | not stated | 9
TOOLCHAIN33 | "Source-facing float namespaces, formats, meanings, and operators are public because their declarations say so." | Consumer-supplied semantic bindings | Float source-facing items public only when declarations declare them public | not stated | 1
TOOLCHAIN34 | "The two primitive-to-proof projections remain private compiler-sealed operations;" | Consumer-supplied semantic bindings | Primitive-to-proof projections must stay private compiler-sealed; reject public exposure | not stated | 8
TOOLCHAIN35 | "public float contracts may cite only those exact checked toolchain declarations, not same-spelled authored replacements." | Consumer-supplied semantic bindings | Reject public float contracts citing same-spelled authored replacements instead of exact checked toolchain declarations | not stated | 8
TOOLCHAIN36 | "Primitive identities are compiler-owned;" | Consumer-supplied semantic bindings | Primitive identities must be compiler-owned, not user-redefinable | not stated | 5
TOOLCHAIN37 | "the toolchain designates exact core owners of their canonical operator families." | Consumer-supplied semantic bindings | Canonical operator families must have exact core owners designated by toolchain | not stated | 9
TOOLCHAIN38 | "This permits ordinary checked `machine +` bodies in authorized core code but does not make every core machine a compiler primitive." | Consumer-supplied semantic bindings | Allow checked `machine +` in authorized core only; must not classify every core machine as compiler primitive | not stated | 10
TOOLCHAIN39 | "Bodyless supply requires a separate exact closed-catalog entry." | Consumer-supplied semantic bindings | Reject bodyless operator supply without separate exact closed-catalog entry | not stated | 9
TOOLCHAIN40 | "Ordinary packages can own domain-specific arithmetic over primitive carriers, not inject operations into the primitive's unqualified closed family." | Consumer-supplied semantic bindings | Reject ordinary packages injecting into primitive unqualified closed family; allow only domain-specific arithmetic over carriers | not stated | 5
TOOLCHAIN41 | "Package-aware compilation does not inherit standalone library provenance." | Core, ordinary libraries, and semantic bindings | Package-aware compilation must not inherit standalone library provenance | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"It is an ordinary fetchable convenience package that may be replaced, split, omitted, or retired." | Compiler accepts graphs with std replaced, split, omitted, or retired without treating those as errors by itself. | Authors may change std externally; compiler still enforces exact dependency resolution and rejection when declared edges disagree. |
"When target entry/profile integration or risk classification needs to recognize an ordinary package declaration, bind its exact nominal identity and normalized schema in the accepted resolved closure." | Binding is required only when a named consumer subsystem needs recognition. | Any package-aware compile that could face integration or classification must pre-bind all ordinary declarations in the closure. |
"A candidate designation only guides confined review; accepted binding comes from consumer policy." | Candidate designation never affects checking until after policy accepts. | Candidate designation may affect review diagnostics while policy is pending; acceptance timing is unstated. |
"Rejoin package, exact declaration, complete schema, and any role-required selected plan." | Consumer must verify all four are present and consistent at binding time. | Rejoin is a data-merge step; whether absence of role-required plan rejects or defers is unstated. |
"Every supplied row settles exactly once;" | Duplicate settlement of the same row rejects. | A row may be updated through one settlement path; whether partial settlement counts as settled is unstated. |
"This permits ordinary checked `machine +` bodies in authorized core code but does not make every core machine a compiler primitive." | Only core machines explicitly authorized may use `machine +` as primitive supply. | Any core machine may use `machine +` syntactically, but only authorized ones count as primitives for semantics. |
"Bodyless supply requires a separate exact closed-catalog entry." | Without catalog entry, bodyless supply rejects at check time. | Bodyless supply is allowed only for operators listed in the closed catalog; catalog scope elsewhere defines membership. |
"preserve supplied provenance and use content identity only when none exists." | Cross-reference to default-build rules; applies when default build assigns owners for core/std. | Applies to all package graphs whenever provenance is missing, not only default-build activations. |

## Terms used without definition in this chapter

- `omega::language::core` / core
- checker / language version
- graph / semantic coupling
- std / convenience package
- repository / lineage / path / filenames
- compiler authority
- freestanding programs
- compiler-owned Build protocol
- magic toolchain mount
- declared dependency edges / package-local imports
- resolved graph
- library's build file
- ordinary package owner / exact toolchain owner
- anonymous root's owner
- default build / supplied provenance / content identity
- checked build context / product context
- build dependency / ordinary dependency
- ambient host services
- host and target type instances
- two checked contexts (linked section)
- target entry / profile integration / risk classification
- ordinary package declaration
- exact nominal identity / normalized schema
- accepted resolved closure
- candidate designation / confined review / consumer policy
- provider / capability
- supplied row
- resolved declaration identity
- requirement-only roles
- interpreter routing token
- checked program
- package declaration visibility
- source-facing float namespaces, formats, meanings, operators
- primitive-to-proof projections
- compiler-sealed operations
- public float contracts
- checked toolchain declarations
- same-spelled authored replacements
- primitive identities
- canonical operator families
- `machine +` bodies / authorized core code
- compiler primitive
- bodyless supply / exact closed-catalog entry
- domain-specific arithmetic / primitive carriers
- unqualified closed family
- package-aware compilation
- standalone library provenance
- operator supply and ownership (linked section)

CHAPTER COMPLETE
