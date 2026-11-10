# Generics — static machine binders through publication and assumptions

Source: `wiki/spec/language/generics.md` lines 578–810 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| GENERICS1 | "Mathematical proof applications add the separately checked logical-evidence supply for machine-shaped theorem hypotheses; it is not executable callback selection." | Static machine binder categories | Machine-shaped theorem hypotheses must use separately checked logical-evidence supply; that path must not be treated as executable callback selection | not stated | 8 |
| GENERICS2 | "Static calls under that complete callable contract." | Static machine binder categories | Structural callable binders may be used only for static calls under the complete callable contract given in the requirement | at a call | 7 |
| GENERICS3 | "Static calls or contextual realization under that exact requirement." | Static machine binder categories | Nominal callable binders may be used only for static calls or contextual realization under the exact named requirement | at a call | 7 |
| GENERICS4 | "Identity relationships only; no invocation." | Static machine binder categories | Declaration-identity binders may express identity only; invocation must be rejected | at a call | 7 |
| GENERICS5 | "A callable binder needs a complete originating or forwarded contract." | Static machine binder categories | Every callable machine binder must have a complete contract that is either authored at origin or obtained by forwarding | not stated | 7 |
| GENERICS6 | "The compiler cannot invent an originating contract from direct invocations, matching signatures, visible conformances or the current consumer set, even with one whole-program instantiation." | Static machine binder categories | Originating callable contracts must not be synthesized from invocations, signature match, visible conformances, consumers, or whole-program instantiation | not stated | 7 |
| GENERICS7 | "Missing or ambiguous contracts reject." | Static machine binder categories | Programs with missing or ambiguous callable contracts must be rejected | not stated | 7 |
| GENERICS8 | "it does not discover one by shape." | Static machine binder categories | Forwarding must not discover a callable contract by callee shape alone | not stated | 7 |
| GENERICS9 | "The nominal form inherits the exact requirement's complete shape, conditions, operational ceilings, and any boundary calling/entry plan; it does not repeat the signature." | Static machine binder categories | Nominal binders must inherit the full requirement metadata without repeating the signature in source | not stated | 7 |
| GENERICS10 | "Both nominal and structural callable invocations contribute reach dependencies under the requirement's open or finite bound." | Static machine binder categories | Reach dependencies for both binder kinds must be recorded under the requirement's open or finite reach bound | at a call | 3 |
| GENERICS11 | "The consumer authors neither a forwarding clause nor a second row argument." | Static machine binder categories | Consumers of nominal callable binders must not author forwarding clauses or a second row argument for that binder | not stated | 7 |
| GENERICS12 | "The generic body checks suspension/blocking against the requirement's upper bound." | Static machine binder categories | Generic machine bodies must check suspension and blocking against the requirement's upper operational bound | not stated | 10 |
| GENERICS13 | "Applications substitute published operational dependencies to determine their caller envelope." | Static machine binder categories | Call-site checking must substitute published operational dependencies to compute the caller envelope | at a call | 3 |
| GENERICS14 | "Declaration selection obeys ordinary visibility and direct-dependency rules, including nested contracts." | Static machine binder categories | Selected callback declarations must satisfy visibility and direct-dependency rules, including nested contracts | not stated | 7 |
| GENERICS15 | "A signature-free requirement path must identify one overload independently of its satisfiers." | Static machine binder categories | Signature-free requirement paths must fix one overload at the requirement, not from satisfiers | not stated | 7 |
| GENERICS16 | "A declaration-identity binder accepts one exact free-machine or signature-free trait-requirement declaration." | Static machine binder categories | Declaration-identity binders must accept exactly one such declaration form | not stated | 7 |
| GENERICS17 | "Types, conformances, runtime values, and overloaded requirement families reject." | Static machine binder categories | Using types, conformances, runtime values, or overloaded requirement families in place of declaration-identity binders must reject | not stated | 7 |
| GENERICS18 | "No default, law, or consumer may invoke that binder without a callable contract." | Static machine binder categories | Invocation of declaration-identity binders without a callable contract must reject, including via defaults, laws, or consumers | at a call | 7 |
| GENERICS19 | "it is not missing-contract inference." | Static machine binder categories | Declaration-identity must not be used to infer missing callable contracts | not stated | 7 |
| GENERICS20 | "Each invocation of a static-machine argument becomes a direct selected call after specialization." | Static machine binder categories | After specialization, each static-machine argument use must lower to a direct call to the selected declaration | at a call | 7 |
| GENERICS21 | "The binder is not a runtime function value or hidden callable argument." | Static machine binder categories | Static machine binders must not be represented as runtime function values or implicit call arguments | not stated | 7 |
| GENERICS22 | "Stateful behavior uses ordinary context data passed through explicit parameters with their normal ownership and access contracts." | Static machine binder categories | Stateful callback behavior must use explicit parameters with normal ownership and access rules | not stated | 2 |
| GENERICS23 | "Authors construct that data and explicitly select the named callback declaration." | Static machine binder categories | Callback data construction and declaration selection must be explicit in source | not stated | 7 |
| GENERICS24 | "No anonymous body/environment association or free-variable capture inference is provided." | Static machine binder categories | Compiler must not infer closure environments or free-variable capture for static machine binders | not stated | 7 |
| GENERICS25 | "The static binder remains a separate choice." | Static machine binder categories | Static machine binder selection must remain distinct from dynamic runtime machine selection mechanisms | not stated | 7 |
| GENERICS26 | "A static machine binder is a declaration argument, not itself a field type." | Static machine binder categories | Static machine binders must not be used as stored field types | not stated | 7 |
| GENERICS27 | "Once its executable application is closed, it may construct or return a machine value through ordinary callable-contract refinement." | Static machine binder categories | Closed applications may produce machine values only when ordinary callable-contract refinement permits | not stated | 7 |
| GENERICS28 | "That query creates an entry demand, not a callable value or an implicit callback selection." | Static machine binder categories | `address_of<Selected<Arguments>>()` must create only an entry demand, not a callable value or implicit selection | at the operation | 7 |
| GENERICS29 | "an exact selected requirement and destination authorize the private ABI entry relocation; an exposed numeric address cannot replace that relationship." | Static machine binder categories | Private callback entry relocation requires exact selected requirement and destination; numeric address exposure must not substitute | not stated | 9 |
| GENERICS30 | "A wrapper passing an explicitly kinded `machine F` binder as a static argument to an exact selected callee may omit a repeated callable requirement for `F`." | Forwarded callback requirements | When forwarding rules apply, wrappers may omit a duplicate `F` callable requirement on the wrapper signature | not stated | 7 |
| GENERICS31 | "Infer the callee's complete requirement after substituting the wrapper's arguments, retaining the exact nominal requirement or authored structural contract and its nested binders." | Forwarded callback requirements | Inferred requirement must be callee's complete contract after argument substitution, preserving nominal or structural form and nested binders | not stated | 7 |
| GENERICS32 | "This applies to public and private checked wrappers; it does not depend on the wrapper's current consumers." | Forwarded callback requirements | Forwarding inference must apply regardless of visibility and without consulting current consumers | not stated | 7 |
| GENERICS33 | "Retain omitted reach as a deferred dependency, explicit empty reach as an empty bound, and named ceilings as finite bounds throughout forwarding." | Forwarded callback requirements | Reach metadata must be preserved through forwarding with correct deferred, empty, or finite classification | not stated | 3 |
| GENERICS34 | "Neither example searches for an implementation; the consumer still selects `F` with its required exact nominal satisfaction edge." | Forwarded callback requirements | Forwarding must not search for implementations; consumer must still supply `F` with exact nominal satisfaction | not stated | 7 |
| GENERICS35 | "forwarding does not collapse it to empty." | Forwarded callback requirements | Forwarded reach dependencies must not be collapsed to empty when the requirement carries service reach | not stated | 3 |
| GENERICS36 | "The termination promise and the suspension/blocking restrictions are unchanged." | Forwarded callback requirements | Forwarded contracts must preserve termination promise and suspension/blocking restrictions from the origin | not stated | 10 |
| GENERICS37 | "Collect requirements from every forwarding use, independent of source order." | Forwarded callback requirements | All forwarding sites must contribute requirements; order in source must not affect the collected set | not stated | 7 |
| GENERICS38 | "The selected callback must satisfy their conjunction, including nested preconditions, guarantees, progress and operational permissions." | Forwarded callback requirements | Selected callback must satisfy the conjunction of all forwarded requirements and nested obligations | not stated | 7 |
| GENERICS39 | "Keep distinct nominal requirement identities even when signatures match." | Forwarded callback requirements | Nominal requirement identities must remain distinct across forwarding even if signatures coincide | not stated | 7 |
| GENERICS40 | "Incompatible binder kinds or callable shapes reject;" | Forwarded callback requirements | Incompatible binder kinds or callable shapes on forwarding paths must reject | not stated | 7 |
| GENERICS41 | "failure to establish any required contract rejects the application." | Forwarded callback requirements | If any forwarded required contract cannot be established, the application must reject | not stated | 7 |
| GENERICS42 | "a forwarding use cannot silently strengthen it to admit an otherwise invalid body." | Forwarded callback requirements | Forwarding must not strengthen an explicit originating body contract to admit an invalid body | not stated | 7 |
| GENERICS43 | "Recursive forwarding must have an exact originating contract; a cycle alone cannot invent one." | Forwarded callback requirements | Recursive forwarding chains must terminate at an exact originating contract; cycles cannot invent contracts | not stated | 7 |
| GENERICS44 | "A bare call `F(item)`, a matching machine signature or a visible conformance is not an origin." | Forwarded callback requirements | Bare calls, signature match, or visible conformance must not count as originating contracts | not stated | 7 |
| GENERICS45 | "Identity-only trait binders stay noncallable." | Forwarded callback requirements | Identity-only trait machine binders must remain noncallable through forwarding | at a call | 7 |
| GENERICS46 | "No inference of callback implementations, trait maps, boundary authority or arbitrary runtime entry predicates is introduced." | Forwarded callback requirements | Forwarding must not introduce inference of implementations, trait maps, boundary authority, or ad hoc runtime entry predicates | not stated | 7 |
| GENERICS47 | "Publish the normalized requirements with exact substitutions, origin provenance and ordinary visibility checks." | Forwarded callback requirements | Published forwarded requirements must be normalized with substitutions, provenance, and visibility checks | not stated | 7 |
| GENERICS48 | "Public consumers check the published contract without reading the wrapper's private body." | Forwarded callback requirements | Dependent checking on public APIs must use published contract only, not private wrapper bodies | not stated | 7 |
| GENERICS49 | "Changing a forwarding use can change that public contract and requires ordinary dependent revalidation." | Forwarded callback requirements | Public contract changes from forwarding edits must trigger ordinary dependent revalidation | not stated | 7 |
| GENERICS50 | "Inferred and authored copies of the same requirement normalize identically; binder renaming or wrapper extraction alone does not change it." | Forwarded callback requirements | Normalization must identify inferred and authored requirements; renaming or extraction alone must not change normalized form | not stated | 7 |
| GENERICS51 | "Diagnostics expose the inferred clause and the forwarding use that requires it." | Forwarded callback requirements | Diagnostics must show inferred requirement clauses and the forwarding site that demanded them | not stated | 7 |
| GENERICS52 | "A wrapper's machine contract also retains the callee's conditional suspension-carry obligations." | Forwarded callback requirements | Wrapper contracts must include callee conditional suspension-carry obligations | not stated | 10 |
| GENERICS53 | "Re-express conditions over the wrapper's exact callback binders and substitute the corresponding resource subjects and provenance." | Forwarded callback requirements | Suspension-carry conditions must be re-expressed on wrapper callback binders with correct resource substitution | not stated | 10 |
| GENERICS54 | "These rows constrain the enclosing application; they do not strengthen F's originating requirement." | Forwarded callback requirements | Forwarded suspension-carry rows constrain the wrapper application only; they must not strengthen `F`'s originating requirement | not stated | 10 |
| GENERICS55 | "Preserve scoped local acquisitions as such rather than exposing a caller-supplied permission for them." | Forwarded callback requirements | Scoped local acquisitions in forwarded suspension-carry must stay local, not become caller-supplied permissions | not stated | 2 |
| GENERICS56 | "Ordinary forwarding cannot discard an unresolved condition." | Forwarded callback requirements | Unresolved suspension-carry conditions must not be dropped by forwarding | not stated | 10 |
| GENERICS57 | "The selected body must refine the complete requirement for every admissible choice of those invocation binders." | Invocation-lifetime families | Selected callback bodies must refine the full requirement for all admissible invocation-lifetime choices | not stated | 2 |
| GENERICS58 | "Matching introduces fresh arbitrary lifetimes, substitutes both contracts by declaration identity, and verifies parameter/result correspondence, loans, and all other refinement axes without stronger premises." | Invocation-lifetime families | Higher-ranked matching must use fresh lifetimes, declaration-identity substitution, and full refinement axes without extra premises | not stated | 2 |
| GENERICS59 | "Rename-bound lifetimes match by their binding structure; matching a single existing environment lifetime is insufficient." | Invocation-lifetime families | Lifetime matching must use binding structure; matching only one environment lifetime must be insufficient for acceptance | not stated | 2 |
| GENERICS60 | "Each invocation instantiates the requirement afresh." | Invocation-lifetime families | Each static callback invocation must use a fresh requirement instantiation | at a call | 2 |
| GENERICS61 | "Sequential calls may end their context/item reborrows between invocations without ending the environment's longer-lived captured loans." | Invocation-lifetime families | Loan checking must allow ending invocation-scoped reborrows between calls while keeping environment captures | on every edge | 2 |
| GENERICS62 | "A callback cannot retain an item loan in a context whose lifetime/type contract does not permit it." | Invocation-lifetime families | Retained item loans across invocations must respect the context lifetime/type contract | not stated | 2 |
| GENERICS63 | "Returned views retain their declared input relationships; fresh instantiation cannot change which storage they denote." | Invocation-lifetime families | View results must keep declared storage relationships across fresh instantiations | at normal return | 2 |
| GENERICS64 | "This higher-ranked matching rule is limited to static callable requirement families and their exact selected bodies, including corresponding generic requirement members." | Invocation-lifetime families | Higher-ranked lifetime matching applies only to static callable requirement families and matching selected bodies | not stated | 7 |
| GENERICS65 | "It is not a blanket extension to whole-conformance lifetime applications, lifetime constants, variance, subtyping, or generic dynamic tables." | Invocation-lifetime families | Other lifetime/generic forms must keep their separate restrictions; this rule must not blanket-apply | not stated | 7 |
| GENERICS66 | "Ordinal substitution identifies binders; it does not by itself prove universal refinement." | Invocation-lifetime families | Binder identification by ordinal substitution must not alone discharge universal refinement proof | not stated | 8 |
| GENERICS67 | "Type-generic callable families similarly check every admitted type alternative." | Invocation-lifetime families | Type-generic callable families must be checked for every admitted type alternative | not stated | 7 |
| GENERICS68 | "Consumers prove each demanded application satisfies the declared family rather than inferring an interface from their current instantiations." | Invocation-lifetime families | Consumers must prove satisfaction of the declared family per application; interface inference from current instantiations must not occur | not stated | 8 |
| GENERICS69 | "Reflection adds member projection obligations, not a different generic matching judgment." | Invocation-lifetime families | Reflection must add projection obligations only, not alter generic matching judgment | not stated | 8 |
| GENERICS70 | "Each callable contract has its own terminator: `Inner` ends first, then the standalone semicolon closes `Schema`." | Nested contracts and proof families | Nested machine contracts must parse terminators so inner callable contracts close before outer ones | not stated | 7 |
| GENERICS71 | "`Selected` belongs to `forward`, not to `Schema`." | Nested contracts and proof families | Outer machine binders must not be attributed to inner contract scopes when delimiters say otherwise | not stated | 7 |
| GENERICS72 | "These delimiters do not separate ordinary predicate clauses." | Nested contracts and proof families | Callable-contract terminators must not be used to separate ordinary predicate clauses | not stated | 7 |
| GENERICS73 | "Operational clauses such as `suspends`, `blocks`, and `terminates` add no terminators of their own." | Nested contracts and proof families | Operational clauses must not introduce extra contract terminators | not stated | 10 |
| GENERICS74 | "Only the complete bodyless callable ends with `;`," | Nested contracts and proof families | Trailing `;` after bodyless callables must mark only complete bodyless callable contract end | not stated | 7 |
| GENERICS75 | "renaming `Inner` changes no contract, but changing its nested shape, conditions, reach, operational/crash ceilings, or termination guarantee can." | Nested contracts and proof families | Contract refinement must be binder-positional: renames invariant; nested semantic changes affect refinement | not stated | 7 |
| GENERICS76 | "Forwarding a distinct machine binder uses the same complete refinement judgment." | Nested contracts and proof families | Forwarded nested machine binders must use the same full refinement judgment as direct nesting | not stated | 7 |
| GENERICS77 | "Specialization substitutes the outer selections and continues through nested applications until executable calls are direct; it creates no runtime dictionary." | Nested contracts and proof families | Specialization must inline nested binders to direct calls without runtime dictionaries | not stated | 7 |
| GENERICS78 | "A static-machine application in a contract instantiates a logical schema." | Nested contracts and proof families | Contract uses of static-machine applications must instantiate logical schemas | not stated | 8 |
| GENERICS79 | "It does not by itself demand executable monomorphization of a selected generic machine." | Nested contracts and proof families | Logical schema instantiation must not by itself force executable monomorphization | not stated | 7 |
| GENERICS80 | "finite-layout data rejects those binders, and a binder is not a stored field type." | Nested contracts and proof families | Finite-layout data must reject machine binders as indices; binders must not be field types | not stated | 7 |
| GENERICS81 | "The selected argument must satisfy the full callable contract." | Nested contracts and proof families | Every selected static machine argument must satisfy the entire nested callable contract | not stated | 7 |
| GENERICS82 | "A proof carrier's telescope is its complete ordered static-parameter list." | Nested contracts and proof families | Proof carriers must expose telescopes as complete ordered static-parameter lists | not stated | 8 |
| GENERICS83 | "carrier parameters have no global relational role." | Nested contracts and proof families | Carrier parameters must not be assigned global relational roles beyond their telescope | not stated | 8 |
| GENERICS84 | "Executable machine-symbol binders do not replace general mathematical function/predicate term binders." | Nested contracts and proof families | Mathematical elaboration must keep term binders; machine-symbol binders must not replace them | not stated | 8 |
| GENERICS85 | "whose mathematical universes and conversion are not defined by machine-symbol lookup." | Nested contracts and proof families | Foundation universes and conversion must not be defined via machine-symbol lookup | not stated | 8 |
| GENERICS86 | "`u: core::Level` adds the checked universe-level binder category to generic telescopes; it is not a runtime integer parameter." | Nested contracts and proof families | Universe-level binders must be checked compile-time binders, not runtime integer parameters | not stated | 8 |
| GENERICS87 | "Logical applications of machine-shaped hypotheses may receive checked term evidence under the logical-use contract." | Nested contracts and proof families | Machine-shaped hypotheses in proofs may receive term evidence only under the logical-use contract rules | not stated | 8 |
| GENERICS88 | "This does not widen executable callback selection or make ordinary machine calls partial." | Nested contracts and proof families | Logical-evidence paths must not widen executable callback selection or partialize ordinary machine calls | at a call | 7 |
| GENERICS89 | "Checked interfaces retain logical-evidence versus executable declaration supply, full level telescopes and constraints." | Nested contracts and proof families | Checked interfaces must keep separate logical-evidence vs executable declaration supply and full level telescopes/constraints | not stated | 8 |
| GENERICS90 | "Public machine-binder identity retains its category and complete normalized requirements: recursively alpha-normalized structural contracts, exact nominal trait/requirements, or explicitly noncallable declaration identity." | Publication and assumptions | Public binder identity must retain category and fully normalized requirements in one of the listed forms | not stated | 7 |
| GENERICS91 | "Forwarded contracts carry the same identity obligations as authored ones; their conjunction is retained without signature-based merging." | Publication and assumptions | Forwarded public requirements must keep identity obligations; conjunction must not merge by signature alone | not stated | 7 |
| GENERICS92 | "Closed applications retain exact selections." | Publication and assumptions | Closed applications must preserve exact machine binder selections in published identity | not stated | 7 |
| GENERICS93 | "Renaming binders is stable; changing a nested contract or its authority is not." | Publication and assumptions | Binder renames must be identity-stable; nested contract or authority changes must change public identity | not stated | 7 |
| GENERICS94 | "Private nominal requirements and missing checked contract evidence reject public review under package boundaries." | Publication and assumptions | Public package review must reject private nominal requirements or missing checked contract evidence | not stated | 7 |
| GENERICS95 | "Generic axioms spend admission on the normalized template and required machine contract, not separately on each instance." | Publication and assumptions | Axiom admission cost must attach to normalized template and machine contract once, not per instance | not stated | 8 |
| GENERICS96 | "Instances retain selected contract identity and the template receipt." | Publication and assumptions | Specialized instances must keep selected contract identity and template admission receipt | not stated | 8 |
| GENERICS97 | "Instance-specific trust instead uses an explicit nongeneric admitted fact." | Publication and assumptions | Trust beyond the generic template must use explicit nongeneric admitted facts | not stated | 8 |
| GENERICS98 | "specialization never launders admitted premises into proofs." | Publication and assumptions | Specialization must not smuggle admitted axioms into proofs without receiving-policy rules | not stated | 8 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "Static calls or contextual realization under that exact requirement." | Contextual realization is limited to nominal callable binders only. | Structural callables might also realize contextually when the requirement is structural but named. |
| "The consumer authors neither a forwarding clause nor a second row argument." | Applies to all nominal callable consumers. | Applies only when the requirement is nominal and forwarding is otherwise available. |
| "If only one implementation is intended, source can call that machine directly without introducing a generic abstraction." | Pure author guidance with no compiler enforcement. | Compiler may warn or reject redundant generic wrappers when a direct call exists. |
| "Once its executable application is closed, it may construct or return a machine value through ordinary callable-contract refinement." | Permission whenever refinement obligations are met after closing. | Permission only at specific pipeline stages named in linked refinement rules. |
| "Infer the callee's complete requirement after substituting the wrapper's arguments" | Inference is mandatory whenever omission is allowed. | Inference runs only when the wrapper omits the clause; otherwise authored text wins. |
| "Retain omitted reach as a deferred dependency, explicit empty reach as an empty bound, and named ceilings as finite bounds throughout forwarding." | Three disjoint reach classes always preserved separately. | Deferred vs empty vs finite might collapse when normalizing if another chapter equates them. |
| "Collect requirements from every forwarding use, independent of source order." | Set semantics; duplicates collapse before conjunction. | Multiset semantics; duplicate forwarding sites might duplicate obligations. |
| "Keep distinct nominal requirement identities even when signatures match." | Identity is nominal path only; structural duplicates still distinct if names differ. | Distinct identities might still normalize to one check if all metadata matches. |
| "An explicit originating callable contract remains the body's bound: a forwarding use cannot silently strengthen it" | Body checking uses origin only; forwarded conjuncts check selection elsewhere. | Body bound is intersection of origin and forwarded conjuncts but origin cannot be weakened for body validity. |
| "Recursive forwarding must have an exact originating contract; a cycle alone cannot invent one." | Any cycle without authored origin rejects. | Cycles may be allowed if an origin appears anywhere in the chain outside the cycle. |
| "Public consumers check the published contract without reading the wrapper's private body." | Private bodies invisible to dependents even for effect rows re-exported via forwarding. | Suspension-carry rows inferred from private callees are in the published contract, not read from body text. |
| "Inferred and authored copies of the same requirement normalize identically; binder renaming or wrapper extraction alone does not change it." | Normal form ignores binder names and wrapper nesting depth. | Normal form ignores renames/extraction but might still depend on visibility or package path. |
| "These rows constrain the enclosing application; they do not strengthen F's originating requirement." | F's declaration contract unchanged for downstream users of F. | F's originating requirement unchanged only for body checking, not for F's own public export. |
| "Preserve scoped local acquisitions as such rather than exposing a caller-supplied permission for them." | Local acquisitions stay callee-local in suspension-carry rows after forwarding. | Caller never gains proof of local acquisitions; whether runtime permissions move is not stated. |
| "Sequential calls may end their context/item reborrows between invocations without ending the environment's longer-lived captured loans." | Loan checker must model invocation-scoped reborrow end between calls. | Authors may choose to end reborrows; compiler does not require ending them between calls. |
| "Ordinary rejection does not require general authored outlives syntax." | Failures use specific lifetime-family rules without mandating outlives annotations in source. | Compiler may still suggest outlives syntax optionally while rejecting without it. |
| "Ordinal substitution identifies binders; it does not by itself prove universal refinement." | Universal proof needs a separate rule beyond ordinal matching. | Ordinal substitution is necessary but insufficient only for some binder kinds. |
| "Reflection adds member projection obligations, not a different generic matching judgment." | Reflection only adds proof goals; matching algorithm unchanged. | Reflection might add goals that reuse matching with a fixed projection prefix. |
| "Here `Stream` denotes an appropriate proof-only family." | Compiler must reject executable uses of this family in the example pattern. | Stream might be executable in other contexts; only this example is proof-only. |
| "Recursive proof-only data may use machine binders as family indices; finite-layout data rejects those binders" | Layout classification is decidable from type definition form. | Finite vs recursive proof-only might need whole-type analysis not spelled here. |
| "Relations choose whether left/right representatives have independent packs or one shared pack;" | Relation declaration fixes pack sharing; compiler enforces at relation use. | Choice might be inferred from relation kind elsewhere in proofs chapter. |
| "Mathematical function values can appear as ordinary parameters and appropriate generic value indices under their declared staging rules." | Staging rules in another doc gate which appearances are legal. | This sentence alone permits any staging the compiler already allows for values. |
| "Generic axioms spend admission on the normalized template and required machine contract, not separately on each instance." | One admission covers all instances of the template. | Admission covers template; instances might still need separate machine-contract evidence without extra admission cost. |
| "Instance-specific trust instead uses an explicit nongeneric admitted fact." | Per-instance trust cannot reuse generic axiom admission. | Nongeneric facts might still reference generic parameters if not counted as instance-specific trust. |

## Terms used without definition in this chapter

- declaration-supply roles
- logical-evidence supply
- machine-shaped theorem hypotheses
- executable callback selection
- Structural callable / Nominal callable / Declaration identity
- complete callable contract
- originating contract
- forwarded contract
- whole-program instantiation
- exact callee application
- generic abstraction
- complete shape
- operational ceilings
- boundary calling/entry plan
- reach dependencies
- open or finite bound
- forwarding clause
- second row argument
- upper bound (suspension/blocking)
- caller envelope
- operational dependencies
- direct-dependency rules
- signature-free requirement path
- overload
- satisfiers
- free-machine declaration
- signature-free trait-requirement declaration
- overloaded requirement families
- default, law (consumer)
- direct selected call
- specialization
- runtime function value
- hidden callable argument
- context data
- named callback declaration
- anonymous body/environment association
- free-variable capture inference
- runtime machine values
- operations records
- some packages
- declaration argument
- executable application (closed)
- callable-contract refinement
- machine value
- `address_of<Selected<Arguments>>()`
- entry demand
- address exposure
- private ABI entry relocation
- exposed numeric address
- explicitly kinded `machine F` binder
- exact selected callee
- public and private checked wrappers
- current consumers
- deferred dependency
- empty bound / finite bounds
- service reach
- exact nominal satisfaction edge
- conjunction (requirements)
- nested preconditions, guarantees, progress
- operational permissions
- nominal requirement identities
- originating callable contract
- recursive forwarding
- bare call
- visible conformance
- trait maps
- boundary authority
- runtime entry predicates
- normalized requirements
- origin provenance
- dependent revalidation
- conditional suspension-carry obligations
- resource subjects
- scoped local acquisitions
- caller-supplied permission
- invocation lifetimes
- environment's captured lifetimes
- admissible choice (invocation binders)
- fresh arbitrary lifetimes
- declaration identity (substitution)
- refinement axes
- rename-bound lifetimes
- binding structure
- context/item reborrows
- general authored outlives syntax
- returned views
- higher-ranked matching rule
- static callable requirement families
- whole-conformance lifetime applications
- lifetime constants
- generic dynamic tables
- ordinal substitution
- universal refinement
- type-generic callable families
- admitted type alternative
- declared family
- member projection obligations
- generic matching judgment
- structural machine contract
- machine parameters (nested)
- terminator (callable contract)
- ordinary predicate clauses
- machine header delimiters
- proof-only family
- Stream (proof-only)
- binder-positional refinement
- nested shape
- operational/crash ceilings
- runtime dictionary
- logical schema
- executable monomorphization
- recursive proof-only data
- finite-layout data
- family indices
- proof carrier
- telescope
- left/right representatives
- independent packs / shared pack
- carrier parameters
- global relational role
- family matching, laws, lifting (Quotients)
- executable machine-symbol binders
- term binders
- selected foundation
- mathematical universes
- machine-symbol lookup
- `core::Level`
- universe-level binder category
- Mathematical function values
- staging rules
- logical-use contract
- logical-evidence versus executable declaration supply
- level telescopes
- public machine-binder identity
- alpha-normalized structural contracts
- signature-based merging
- closed applications
- exact selections
- nested contract / authority
- public review
- package boundaries
- generic axioms
- admission (axioms)
- normalized template
- template receipt
- instance-specific trust
- nongeneric admitted fact
- Proof receiving policy
- launders admitted premises

CHAPTER COMPLETE
