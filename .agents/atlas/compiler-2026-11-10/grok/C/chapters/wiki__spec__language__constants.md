# Constants

Source: `wiki/spec/language/constants.md` lines 1–88 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
CONSTANTS1 | "`const` names an immutable compile-time value, not runtime storage." | Constants | A `const` declaration must denote a compile-time immutable value and must not be treated as runtime storage | not stated | 1
CONSTANTS2 | "It may be package/module-scoped or genuinely type-scoped, outside a `data` body:" | Constants | `const` placement must be package-, module-, or type-scoped and must not appear inside a `data` body | not stated | 1
CONSTANTS3 | "Symbolic mathematical definitions instead use [parameterized top-level `let`](../proofs/mathematical_bindings.md#named-mathematical-definitions)." | Constants | Symbolic mathematical definitions must use parameterized top-level `let`, not `const` | not stated | 8
CONSTANTS4 | "This distinction does not change local `let` or make `const` a noncomputability escape." | Constants | `const` must not be accepted as a way to escape noncomputability obligations that `let` would still enforce | not stated | 1
CONSTANTS5 | "Constants are not fields and do not contribute to `size_of<T>()`." | Constants | `size_of<T>()` for a type must exclude constants; constants must not be counted as fields | not stated | 1
CONSTANTS6 | "Their complete types recursively permit copying and have no cleanup obligation, shared ownership, or interior mutability." | Constants | A `const` value's complete type must recursively allow copying and must lack cleanup, shared ownership, and interior mutability | not stated | 1
CONSTANTS7 | "Fixed arrays, records, and copy-eligible sums are eligible; selecting an unrestricted active case does not make a structurally linear sum eligible." | Constants | Only fixed arrays, records, and copy-eligible sums may be constant types; an unrestricted active case selection must not make a structurally linear sum eligible | not stated | 1
CONSTANTS8 | "A constant has no stable address and grants no authority." | Constants | Constants must not be given stable addresses or authority grants | not stated | 1
CONSTANTS9 | "Borrowing a materialized copy follows ordinary temporary/storage rules; its address is not the identity of the `const` declaration." | Constants | Borrows of materialized constant copies must follow ordinary borrow rules; the borrow address must not identify the `const` declaration | not stated | 2
CONSTANTS10 | "A constant position requests semantic evaluation." | Evaluation | Uses in constant positions must trigger semantic evaluation | not stated | 1
CONSTANTS11 | "An initializer may call an ordinary machine only when that concrete invocation is admitted for evaluation; there is no separate `const machine` species." | Evaluation | Constant initializers may invoke machines only if that concrete call is admitted for evaluation; there is no separate const-only machine kind | not stated | 10
CONSTANTS12 | "Effectful [build execution](../build/execution.md) is a separate operation." | Evaluation | Effectful build execution must not be conflated with constant semantic evaluation | not stated | 9
CONSTANTS13 | "[Address exposure](counts_and_addresses.md#address-exposure) does not make a runtime storage address, load-resolved entry address, or interpreter identity a canonical constant input." | Evaluation | Runtime storage addresses, load-resolved entry addresses, and interpreter identities exposed by address exposure must not be accepted as canonical constant inputs | not stated | 1
CONSTANTS14 | "An executable entry relocation is a realization dependency, not a license to bake evaluator addresses into `const` values." | Evaluation | Executable entry relocations must not be used to embed evaluator addresses into `const` values | not stated | 9
CONSTANTS15 | "Data `size_of<T>()` and `align_of<T>()` may be ordinary constants once the selected target and validated layout are known." | Evaluation | `size_of<T>()` and `align_of<T>()` as data constants are permitted only after target selection and validated layout are known | not stated | 9
CONSTANTS16 | "Native `code_layout<Machine>()` is instead late artifact metadata; its sizes cannot feed the same product's semantic constants or layout decisions." | Evaluation | `code_layout<Machine>()` sizes must not feed the same product's semantic constants or layout decisions | not stated | 9
CONSTANTS17 | "Evaluation does not make an already-typed value anonymous." | Evaluation | Semantic evaluation must preserve type identity for already-typed values | not stated | 1
CONSTANTS18 | "Anonymous numeric initializers retain exact arithmetic, including rational division, until their landing boundary." | Evaluation | Anonymous numeric constant initializers must keep exact arithmetic through rational division until the landing boundary | not stated | 5
CONSTANTS19 | "Integer landing requires an integral in-range value; floating landing rounds to the selected format." | Evaluation | At integer landing the value must be an integral value in range; at floating landing the value must round to the selected format | not stated | 5
CONSTANTS20 | "The destination does not select integer division inside an anonymous initializer." | Evaluation | The destination type must not force integer division semantics inside an anonymous numeric initializer | not stated | 5
CONSTANTS21 | "[Target-semantic observations](evaluation.md#target-dependent-applications) are canonical constant inputs, including in array lengths and const-generic arguments." | Evaluation | Target-semantic observations must be accepted as canonical constant inputs in array lengths and const-generic arguments | not stated | 9
CONSTANTS22 | "Their symbolic dependency, exact target closure, and provenance survive folding." | Evaluation | Folding must preserve symbolic dependency, exact target closure, and provenance of target-semantic observations | not stated | 9
CONSTANTS23 | "Constants cannot add fields, cases, multiplicity, or declarations." | Evaluation | Constant evaluation must not introduce fields, cases, multiplicity, or declarations | not stated | 1
CONSTANTS24 | "Evaluation and runtime representation are separate judgments." | Materialization | The compiler must treat semantic evaluation and runtime representation as distinct judgments | not stated | 1
CONSTANTS25 | "A value used only by proofs, layouts, or further evaluation need not occupy runtime bytes." | Materialization | Values used only in proofs, layouts, or further evaluation may omit runtime byte occupation | not stated | 8
CONSTANTS26 | "Materialization requires a selected layout determining every observable bit of the realized active value, recursively through its active fields and case." | Materialization | Runtime materialization requires a selected layout that fixes every observable bit of the active value recursively through fields and cases | not stated | 1
CONSTANTS27 | "Failure identifies the offending component and producer origin." | Materialization | Materialization failure must report the offending component and its producer origin | not stated | 1
CONSTANTS28 | "Semantically unobservable padding is emitted as zero." | Materialization | Padding bits that are semantically unobservable must be emitted as zero in materialized output | not stated | 1
CONSTANTS29 | "Layout is necessary, not sufficient: the value must come from admitted computation or a checked realization of its mathematical meaning." | Materialization | Having layout is not enough; materialization demands a value from admitted computation or checked realization of mathematical meaning | not stated | 8
CONSTANTS30 | "A chosen member of a nonempty subset of `u32` may exist in a proof without providing materializable bits." | Materialization | A proof-chosen `u32` member need not supply materializable bits; materialization must not assume it does | not stated | 8
CONSTANTS31 | "A condition computationally depending on an unrealized mathematical value also creates demand even if both branch results are ordinary constants." | Materialization | If a condition computationally depends on an unrealized mathematical value, materialization demand exists even when both branches are ordinary constants | not stated | 8
CONSTANTS32 | "Admitted premises used only to justify executable computation are not themselves missing algorithms." | Materialization | Premises admitted only to justify executable computation must not be treated as missing algorithms for materialization | not stated | 8
CONSTANTS33 | "No domain cast or carrier substitution can discharge that demand." | Materialization | Domain casts and carrier substitutions must not satisfy unrealized-mathematical-value materialization demand | not stated | 8
CONSTANTS34 | "A NaN whose payload is not determined remains usable through `Float::meaning` in proof and compile-time positions." | Materialization | Undetermined-payload NaN must remain usable via `Float::meaning` in proof and compile-time positions | not stated | 5
CONSTANTS35 | "Runtime materialization requires canonicalization, explicit bits, or a selected realization publishing the exact representation." | Materialization | Runtime materialization of such NaN requires canonicalization, explicit bits, or a selected realization that publishes the exact representation | not stated | 5
CONSTANTS36 | "An arbitrary evaluator choice cannot become stable image data." | Materialization | Arbitrary evaluator choices must not become stable image data | not stated | 9
CONSTANTS37 | "A quotient retains the exact operational representative selected at construction, as runtime execution does." | Materialization | Quotient materialization must keep the operational representative chosen at construction, matching runtime execution | not stated | 8
CONSTANTS38 | "Ordinary opaque materialization may emit that carried representative without canonicalization; equivalent quotient values may have different bytes." | Materialization | Opaque materialization may output the carried quotient representative without canonicalization; equivalent quotients may differ in bytes | not stated | 8
CONSTANTS39 | "Representative-independent consumers instead require proved canonical form: stable serialization, public ABI representation, canonical const-index identity, structural hashing/interning, or reproducible raw bytes." | Materialization | Uses listed as representative-independent require proved canonical quotient form before materialization | not stated | 8
CONSTANTS40 | "The distinction does not expose the opaque representative to source code." | Materialization | Opaque quotient representatives must not be exposed in source code | not stated | 8
CONSTANTS41 | "Addressable immutable image storage would require a separate storage contract; it is not implied by `const`." | Materialization | `const` must not imply addressable immutable image storage without a separate storage contract | not stated | 1
CONSTANTS42 | "Likewise, there is no ambient mutable `static`." | Materialization | The language must not provide ambient mutable `static` storage implied by this chapter | not stated | 1
CONSTANTS43 | "Long-lived entry state is [one explicitly borrowed, provisioned occurrence](../build/entry_roots.md#entry-shape-and-arrival-bridge), not a globally nameable object." | Materialization | Long-lived entry state must be modeled only as an explicitly borrowed provisioned occurrence, not a global nameable object | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"It may be package/module-scoped or genuinely type-scoped, outside a `data` body:" | Any `const` must be one of those scopes and must sit outside any `data` body. | The list is illustrative of permitted placements; other scopes or positions outside `data` might still be allowed but are not stated here. |
"This distinction does not change local `let` or make `const` a noncomputability escape." | The compiler applies the same noncomputability constraints to `const` as to `let`; `const` cannot weaken them. | The distinction between `const` and mathematical `let` is documentary only; whether `const` can still escape other compile-time restrictions not tied to noncomputability is not stated. |
"Fixed arrays, records, and copy-eligible sums are eligible; selecting an unrestricted active case does not make a structurally linear sum eligible." | Only those aggregate kinds can be constant types; picking an unrestricted active case never overrides linearity of the sum. | "Copy-eligible sums" might admit sums that are not fixed arrays or records; the sentence does not say whether other copyable types (e.g. scalars) are eligible without naming them. |
"An initializer may call an ordinary machine only when that concrete invocation is admitted for evaluation; there is no separate `const machine` species." | Each machine call in a constant initializer is checked for evaluation admission under the ordinary machine rules. | "That concrete invocation" might mean syntactic identity of the call site only, or might include callee specialization; what counts as one invocation for admission is not stated in this sentence. |
"Data `size_of<T>()` and `align_of<T>()` may be ordinary constants once the selected target and validated layout are known." | They become valid constant expressions only after both target selection and layout validation complete. | "May be" permits but does not require them to be constants even when target and layout are known; rejection when known is still allowed. |
"Layout is necessary, not sufficient: the value must come from admitted computation or a checked realization of its mathematical meaning." | Materialization fails without both layout and an admitted computational or checked mathematical source for the value. | Layout plus either admitted computation or checked realization is required; the sentence does not say whether partial layout without full recursive bit determination still blocks materialization. |
"A condition computationally depending on an unrealized mathematical value also creates demand even if both branch results are ordinary constants." | Materialization is required for the condition's dependence on the unrealized value regardless of branch constant-ness. | "Creates demand" might mean only diagnostic demand or proof obligation, not necessarily runtime byte materialization of the mathematical value. |
"Ordinary opaque materialization may emit that carried representative without canonicalization; equivalent quotient values may have different bytes." | The compiler may always emit the construction-time representative for ordinary opaque materialization. | "May emit" allows non-canonical bytes but does not require them; the compiler might still canonicalize unless a consumer forces otherwise. |
"Representative-independent consumers instead require proved canonical form: stable serialization, public ABI representation, canonical const-index identity, structural hashing/interning, or reproducible raw bytes." | Each listed consumer kind must have proved canonical form before use. | The list might be exemplary rather than exhaustive; whether other consumers are representative-independent is not stated. |

## Terms used without definition in this chapter

- compile-time value
- runtime storage
- package/module-scoped
- genuinely type-scoped
- `data` body
- parameterized top-level `let`
- checked terms
- local `let`
- noncomputability escape
- fields (vs constants)
- `size_of<T>()`
- complete types
- recursively permit copying
- cleanup obligation
- shared ownership
- interior mutability
- fixed arrays
- copy-eligible sums
- unrestricted active case
- structurally linear sum
- stable address
- authority
- materialized copy
- temporary/storage rules
- semantic evaluation
- ordinary machine
- invocation admitted for evaluation
- `const machine` species
- effectful build execution
- address exposure
- runtime storage address
- load-resolved entry address
- interpreter identity
- canonical constant input
- executable entry relocation
- realization dependency
- evaluator addresses
- selected target
- validated layout
- `align_of<T>()`
- native `code_layout<Machine>()`
- late artifact metadata
- semantic constants
- layout decisions
- already-typed value
- anonymous numeric initializers
- exact arithmetic
- rational division
- landing boundary
- integer landing
- integral in-range
- floating landing
- selected format
- integer division
- target-semantic observations
- array lengths
- const-generic arguments
- symbolic dependency
- exact target closure
- provenance
- folding
- multiplicity
- runtime representation
- separate judgments
- proofs
- layouts
- runtime bytes
- materialization
- selected layout
- observable bit
- realized active value
- active fields and case
- offending component
- producer origin
- semantically unobservable padding
- admitted computation
- checked realization
- mathematical meaning
- nonempty subset of `u32`
- materializable bits
- unrealized mathematical value
- admitted premises
- missing algorithms
- domain cast
- carrier substitution
- NaN
- payload
- `Float::meaning`
- compile-time positions
- canonicalization
- explicit bits
- selected realization
- stable image data
- quotient
- operational representative
- opaque materialization
- representative-independent consumers
- proved canonical form
- stable serialization
- public ABI representation
- canonical const-index identity
- structural hashing/interning
- reproducible raw bytes
- addressable immutable image storage
- storage contract
- ambient mutable `static`
- long-lived entry state
- explicitly borrowed, provisioned occurrence
- globally nameable object

CHAPTER COMPLETE
