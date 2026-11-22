# Decisions behind the 31 L5 blockers

This is the starting map for the repair pass at `f572107080eda`. The readiness case ledger remains the source of the current verdict. Each row names the original cases, the producer and consumer involved, and a control that distinguishes this decision from nearby ones. Grouped cases share an observed first stop. Other failures may appear after that stop is repaired.

## Cases awaiting a language-owner ruling

Four measured cases await four open decisions. A test that reproduces the current refusal keeps its case open. Follow the named entries in the repository owner queue for the ruling and then test its effect on the indicated input.

| Case | Decision that would change its expected output |
| --- | --- |
| Simultaneous domain removal and addition | [Checked replacement](../../../OWNER_QUESTIONS.md#named-domain-casts-checked-replacement) determines which Psi evidence can represent one authored cast. |
| Non-exact computed ranking subject | [Descending rank entry expressions](../../../OWNER_QUESTIONS.md#descending-rank-entry-expressions) determines which arithmetic expressions are valid rank subjects. |
| Carry policy on a transparent data alias | [Transparent alias carry assertions](../../../OWNER_QUESTIONS.md#transparent-alias-carry-assertions) determines whether an alias can declare a floor that leaves its effective policy unchanged. |
| Hypothesis outside a theorem binder | [Machine hypotheses without a binder](../../../OWNER_QUESTIONS.md#machine-hypothesis-without-a-binder) determines whether the declaration is invalid or the citation owes an implicit evidence argument. |

## Storage, views and suspension

| Original readiness case | Decision boundary | Discriminating evidence |
| --- | --- | --- |
| `handoff/restoration through an unproved selector`<br>`handoff/restoration through a premised selector` | L5 store disposition → ownership checker | Move from p[i], then store to p[j]. Equality, inequality and absence of a premise change the evidence available to justify restoration or replacement. Keep both captured places, custody and the store action. |
| `handoff/caller-chosen boundary view result at suspension`<br>`handoff/recursive relay of a caller-chosen boundary view at suspension` | Boundary call selection → suspended-loan origin | A boundary result with an input-relative lifetime has a named lifetime anchor. Remove that input. Direct and recursively relayed calls both need the provider-backed result identified at suspension. The caller’s independent input cannot supply that backing. |
| `handoff/returning an indexed shared field` | L5 field descriptor → returned view | Forming a recursive record schema identifies the indexed referent. Returning its next field additionally needs the originating embedded loan and the return relation. |
| `handoff/shared float view of a mutable scalar local` | L5 local storage → borrow | The immutable scalar can supply a home. Making the scalar mutable requires its later assignments to update the storage that was lent. |
| `handoff/borrowed scalar home live across a suspension` | L5 suspension → retained storage | The borrow now has an address before the call. Suspending requires the parked call to retain that primitive home. |
| `handoff/arithmetic on float reads` | L5 arithmetic → Psi operation | Reading the bits of two f32 values succeeds. Adding them needs a floating-point operation with the selected arithmetic semantics. |
| `handoff/value-level cast from an integer to a float` | Typing and conversion rules → L5 | A reference recast, an owned bit conversion and a numeric conversion have different contracts. Settle the applicable conversion or rejection for the value-level cast before granting it a Psi result. |

For the unproved-selector case, the source moves `p[i]` into `old`, assigns `c` to `p[j]`, then restores `old` to `p[i]`. If `i == j`, the second assignment displaces `c`. If `i != j`, the first assignment displaces the original `p[j]`. The bounded `Lease` twins retain the same two bounds and call the selected `release` on different inputs at those respective sites. Bounds alone leave both schedules possible. With no selector premise, L5 compares the captured indices once and emits both schedules on the corresponding branches. [Selector and cleanup controls](tests/view_loans.rs)

The unchecked Psi body must already identify its executable operations and cleanup edges. L6 can prove that an emitted edge is justified. Choosing an edge during checking would also choose the program's disposal order and hook operand. Each arm now carries its own displacement, cleanup target and two stores. They rejoin after restoration, so L6 can check the emitted program without choosing its behavior. [Psi product and check order](../../../wiki/spec/terminal-psi/product.md#optimization-ordering) · [Borrowed-storage restoration](../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration)

## Generic machines and ranking

| Original readiness case | Decision boundary | Discriminating evidence |
| --- | --- | --- |
| `handoff/scalar field operand of a record with a non-scalar field` | L5 template operand → selected field | Keep the formal, field and call unchanged. Changing an unread sibling to an array used to prevent the selected scalar field from becoming a template operand. |
| `handoff/computed ranking subject` | L5 rank expression → rank checker | A formal and a constant have rank operands. K - n additionally requires exact ordered subtraction and substitution of the call actuals. |
| `handoff/cleanup in a ranked component` | L5 cleanup action → rank graph | The unranked package contains the cleanup call. Its ranked counterpart needs an edge naming that cleanup invocation and its actuals so a checker can compare the ranks. |
| `handoff/exported wrapper with a restricted evidence binder` | L5 exported interface → external caller | A closed private instance can carry its selected evidence. Export requires the open wrapper interface and its restricted binder to survive independently of that instance. |
| `handoff/open template whose only static binder is a value` | L5 open template → instance planner | A template with a callable or implementation binder can reach Psi. A template with only a scalar const binder follows a different admission path. |
| `handoff/open callback body beyond straight-line calls` | L5 open body → later instantiation | Two straight-line calls can be represented separately. Combining their results with an addition requires an intermediate value and an operation in the unspecialized body. |

## Access, aliases and domains

| Original readiness case | Decision boundary | Discriminating evidence |
| --- | --- | --- |
| `handoff/access plan evaluation at its domain`<br>`handoff/domain access policy` | Evaluated plan in L4 → governed domain in L5 | Both fixtures request the same plan and domain schema. L3 identifies the policy machine. L5 still needs the admitted computation’s result for that schema, including issuance. L5 consumes a validated plan when preparation holds one ([issuance tests](tests/domain_issuance.rs)) and still stops here without one. |
| `handoff/unselected access policy template constructor` | L5 package assembly → template callee | Remove the domain’s access selection. The policy template remains and still calls the imported library constructor, which must have a packaged declaration. |
| `handoff/carry policy on a transparent data alias` | Alias semantics → normalized type | An alias can inherit its target’s policy. L3 now refuses a list that adds a demand the expansion lacks, including the original `u64` case. The outstanding language question is whether an alias may author a list that adds no demand. |
| `handoff/add a tag and a predicate domain in one cast` | L5 qualification edge → domain checker | One arrival establishes two memberships with different reasons. Each added domain must retain its own origin on the same value. |
| `handoff/add an inherited child with its own predicate` | Terminal Psi membership rule → domain checker | On an input that retains the parent, the child’s completion carries its own predicate obligation. The child declaration identifies the parent required on that arrival. |
| `handoff/simultaneous domain removal and addition` | Terminal Psi cast form → lowering | Erasing alone and adding alone have operations. The canonical spec leaves the terminal form for a simultaneous removal and addition open. |

## Theorems and callback premises

| Original readiness case | Decision boundary | Discriminating evidence |
| --- | --- | --- |
| `handoff/proof steps in a theorem body` | L5 theorem → proof checker | A declaration can name hypotheses and a goal. A nonempty proof body additionally needs a proof term with its scoped steps. |
| `handoff/hypothesis outside a theorem binder` | Theorem binder rule → proof interface | A matched binder provides a slot for hypothesis evidence. The original case has no matching slot; the validity of that source form requires a ruling. |
| `handoff/theorem citation with a computed subject` | L5 theorem citation → proof checker | A named subject can refer to an existing value. A literal or computed subject requires an erased term that does not introduce executable evaluation. |
| `handoff/premise-bearing closed static callback` | L5 callback selection → contract checker | A closed callback without a premise can preserve its target and shape. A premise-bearing selection must retain the applied bound and the selected target’s refinement. |
| `sample/abstract-premise`<br>`sample/false-abstract-premise` | Historical source → L1 parser | Both pinned files stop on the same obsolete crash spelling. Their different truth values matter later. A lawful rewrite exposes a separate closed-callback contract case. |

## Sample setup and imports

| Original readiness case | Decision boundary | Discriminating evidence |
| --- | --- | --- |
| `sample/import-from-dependency` | Admitted dependency → L2 declaration lookup | With the dependency registered, importing a file by name still fails; importing its public declaration reaches an inter-package Psi call. Removing the alias isolates the dependency requirement. |
| `sample/cross-width-v2`<br>`sample/rational-landing-v2` | L2 visible names → package compilation | Both omit an import and stop at Wrapping on the hits field. Removing only that qualification in the baseline lets L5 inspect the three comparisons and the three identity calls respectively. The original sample sources remain pinned. |

## Decisions newly measured before the first autonomous wave

The next baseline had 33 blocking cases. Five of the original 31 had met their expectation. The seven cases below account for the rest of the change. All twelve programs behind these seven already stopped on the preceding compiler. Preserve those inputs when measuring the next repair.

| Case | Fact the consumer needs | Control |
| --- | --- | --- |
| `handoff/restoration requiring transitive selector equality` | The store disposition and both captured selectors, with the premises that justify comparing them. | `i == k` and `k == j` versus a missing second premise. |
| `handoff/non-scalar field operand of a formal record` | The carrier and transfer of the selected structural field at a template call. | Pass the array field itself versus its scalar sibling. |
| `handoff/nested scalar field operand of a formal record` | The complete path from the formal through its nested record to the scalar leaf. | Read `track.tag.id` versus a scalar directly on `track`. |
| `handoff/projected field operand beside a sibling with cleanup or linear use` | The selected scalar read and any ownership obligations that remain on the formal. | Change only the unselected sibling to a type with cleanup or a linear-use requirement. |
| `handoff/non-exact computed ranking subject` | The arithmetic meaning and admissibility of the subject used for the well-founded relation. | Change the subtraction mode or expression shape while keeping the same formal and constant. |
| `handoff/add a tag child and a predicate domain in one cast` | The parent and its tag child each have an origin on the same arrival. The unrelated predicate domain carries the only predicate debt. | Compare the explicit parent entry with the child shorthand and inspect the same exact memberships on their jumps. |
| `handoff/add an applied domain and a domain of the other origin in one cast` | Each added domain’s complete application and its establishment origin. | Keep the family fixed and vary its index, then remove only the other domain. |
