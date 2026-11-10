# Boundary signature shapes

Source: `wiki/spec/build/boundary_shapes.md` lines 1–66 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
BOUNDARY_SHAPES1 | "A [calling policy](calling_plans.md) may classify a semantic type directly only when its public normalized structure determines every required ABI fact." | Boundary signature shapes | Direct classification of a semantic type by a calling policy is permitted only if the public normalized structure fixes all required ABI facts | not stated | 9
BOUNDARY_SHAPES2 | "Otherwise the native leaf must declare the concrete foreign shape." | Boundary signature shapes | When direct classification is not allowed, the native leaf must declare the concrete foreign shape | not stated | 9
BOUNDARY_SHAPES3 | "Knowing a private compiler layout does not make it a public ABI." | Boundary signature shapes | Private compiler layout must not be treated as a public ABI | not stated | 9
BOUNDARY_SHAPES4 | "Fixed arrays and fixed records expose element/member types, counts, size, alignment, and public layout." | Public aggregates | Fixed arrays and fixed records must expose element or member types, counts, size, alignment, and public layout | not stated | 1
BOUNDARY_SHAPES5 | "Classification is recursive: equal size does not imply equal ABI class." | Public aggregates | Aggregate classification must be recursive; equal size must not imply equal ABI class | not stated | 9
BOUNDARY_SHAPES6 | "A policy may place or reject the aggregate under its target's rules." | Public aggregates | The policy must either place or reject the aggregate according to its target rules | not stated | 9
BOUNDARY_SHAPES7 | "An erased field retains semantic/proof identity but contributes no ABI field, offset, size, alignment, register/stack fragment, or transfer." | Public aggregates | Erased fields must keep semantic and proof identity but must contribute nothing to ABI layout or transfer | not stated | 1
BOUNDARY_SHAPES8 | "Omission is recursive." | Public aggregates | Erased-field omission must apply recursively through nested structure | not stated | 1
BOUNDARY_SHAPES9 | "Terminal Psi retains the exact erased type identity separately." | Public aggregates | Terminal Psi must retain exact erased type identity apart from ABI omission | not stated | 9
BOUNDARY_SHAPES10 | "A record containing only erased fields has no by-value ABI carrier and rejects." | Public aggregates | A record with only erased fields must have no by-value ABI carrier and compilation must reject | not stated | 1
BOUNDARY_SHAPES11 | "Case-bearing or unresolved generic aggregates cannot be classified without a specified public ABI shape; private compiler layout is not a substitute." | Public aggregates | Case-bearing or unresolved generic aggregates must not be classified without a specified public ABI shape; private compiler layout must not substitute | not stated | 7
BOUNDARY_SHAPES12 | "`BoundarySignature` uses a bounded flat graph: parameter/result roots index shape nodes, fixed-array nodes name an element root and count, and record nodes name a contiguous field range with child roots and byte offsets." | Public aggregates | `BoundarySignature` must use the stated bounded flat graph structure for roots, arrays, and records | not stated | 9
BOUNDARY_SHAPES13 | "The ordered native telescope projects semantic formals and interleaves declared native-only callback entries." | Public aggregates | The native telescope must project semantic formals in order and interleave declared native-only callback entries | not stated | 9
BOUNDARY_SHAPES14 | "A callback has target function-pointer shape without a semantic runtime node." | Public aggregates | Each callback entry must be function-pointer shape with no semantic runtime node | not stated | 9
BOUNDARY_SHAPES15 | "The policy returns ABI shape and placement per native entry; the compiler checks both against the graph and telescope." | Public aggregates | Policy-supplied ABI shape and placement per native entry must be checked against the `BoundarySignature` graph and the native telescope | not stated | 9
BOUNDARY_SHAPES16 | "Opaque by-value roots first require the exact [representation application](opaque_representations.md)." | Public aggregates | Opaque by-value roots must require the exact representation application before boundary use | not stated | 9
BOUNDARY_SHAPES17 | "Its declaration, source, target-semantics version, closed shape, movement, lifecycle, and evidence origin participate in boundary-application identity." | Public aggregates | Listed attributes must participate in boundary-application identity for opaque by-value roots | not stated | 9
BOUNDARY_SHAPES18 | "There is no implicit C array decay." | Foreign declarations | The compiler must not apply implicit C array decay at the boundary | not stated | 9
BOUNDARY_SHAPES19 | "`[T; N]` declares an aggregate by value;" | Foreign declarations | `[T; N]` at a foreign declaration must be treated as an aggregate passed by value | not stated | 1
BOUNDARY_SHAPES20 | "`&[T; N]` declares a reference." | Foreign declarations | `&[T; N]` at a foreign declaration must be treated as a reference, not by-value aggregate | not stated | 2
BOUNDARY_SHAPES21 | "A C parameter written as an array but passed as a pointer must use the corresponding pointer/reference contract in Omega." | Foreign declarations | C array parameters passed as pointers must use the matching Omega pointer or reference contract | not stated | 9
BOUNDARY_SHAPES22 | "Slices, text views, vectors, and bounded text do not by themselves specify a foreign length type, nullability, retention, ownership, terminator, or descriptor layout." | Foreign declarations | Those carriers must not be treated as specifying foreign length, nullability, retention, ownership, terminator, or descriptor layout without further declaration | not stated | 6
BOUNDARY_SHAPES23 | "Default native policies reject those bare carriers at a leaf." | Foreign declarations | Under default native policies, bare slice, text, vector, and bounded-text carriers must be rejected at a leaf | not stated | 9
BOUNDARY_SHAPES24 | "An API expecting separate pointer and length arguments declares them separately:" | Foreign declarations | APIs that expect separate pointer and length must declare pointer and length as separate parameters | not stated | 9
BOUNDARY_SHAPES25 | "An API expecting a terminator or descriptor declares that shape instead." | Foreign declarations | APIs expecting terminator or descriptor layout must declare that shape explicitly | not stated | 9
BOUNDARY_SHAPES26 | "A pointer/length record is not interchangeable with two native parameters." | Foreign declarations | A pointer-length record type must not be treated as interchangeable with two separate native parameters | not stated | 9
BOUNDARY_SHAPES27 | "A custom policy may explicitly define a slice/text ABI; it is not inferred from the compiler's current carrier." | Foreign declarations | Slice or text ABI may be defined only by an explicit custom policy, not inferred from the compiler's current carrier | not stated | 9
BOUNDARY_SHAPES28 | "A checked adapter exposes the safe Omega view." | Foreign declarations | A checked adapter must expose the safe Omega view at the boundary | not stated | 9
BOUNDARY_SHAPES29 | "A synchronous non-retaining leaf uses a call-scoped borrowed-out contract; its calling plan cannot extend the loan." | Foreign declarations | A synchronous non-retaining leaf must use call-scoped borrowed-out; the calling plan must not extend the loan beyond that scope | at a call | 2
BOUNDARY_SHAPES30 | "Retention requires a pinned loan, ownership transfer, or registration protocol." | Foreign declarations | Retention across the boundary must use pinned loan, ownership transfer, or registration protocol | not stated | 2
BOUNDARY_SHAPES31 | "Outbound text supplies bytes without its `Utf8` qualification;" | Foreign declarations | Outbound text crossing the boundary must supply bytes without retaining `Utf8` qualification | not stated | 6
BOUNDARY_SHAPES32 | "inbound text validates bytes before establishing that qualification." | Foreign declarations | Inbound text must validate bytes before `Utf8` qualification is established | not stated | 6
BOUNDARY_SHAPES33 | "Proof objects do not cross the foreign boundary." | Foreign declarations | Proof objects must not cross the foreign boundary | not stated | 8
BOUNDARY_SHAPES34 | "Process entry's selected requirement fixes its exit-status scalar, not the source name `main`." | Foreign declarations | Process entry exit-status scalar type must come from the selected requirement, not from the source name `main` | at entry | 9
BOUNDARY_SHAPES35 | "Firmware and other boundaries may permit aggregate results when their selected policy does." | Foreign declarations | Aggregate results at firmware or other boundaries are permitted only when the selected policy permits them | at normal return | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"A [calling policy](calling_plans.md) may classify a semantic type directly only when its public normalized structure determines every required ABI fact." | "May classify" means the policy is allowed to classify directly when the condition holds; it must not classify directly when the condition fails. | "May" leaves room that the policy could still refuse direct classification even when public normalized structure determines every ABI fact. |
"A policy may place or reject the aggregate under its target's rules." | For each aggregate and target, the policy must choose placement or rejection consistently with target rules. | The policy has discretionary choice to place or reject among aggregates that satisfy target rules. |
"Default native policies reject those bare carriers at a leaf." | Only default native policies reject; non-default policies might accept bare carriers without an explicit slice/text ABI. | "Default native policies" names a fixed set; whether other policies exist or must also reject unless they define ABI is not stated in this sentence. |
"The policy returns ABI shape and placement per native entry; the compiler checks both against the graph and telescope." | Any mismatch between policy output and graph or telescope must fail compilation. | The compiler checks consistency but the sentence does not state whether mismatch is always a hard reject versus a diagnostic-only check. |
"A synchronous non-retaining leaf uses a call-scoped borrowed-out contract; its calling plan cannot extend the loan." | "Non-retaining" is defined by use of call-scoped borrowed-out and no loan extension in the calling plan. | "Non-retaining" is a separate classification that must be detected independently; violating the contract is not stated as the sole criterion. |
"A custom policy may explicitly define a slice/text ABI; it is not inferred from the compiler's current carrier." | Without an explicit custom policy definition, slice/text carriers remain rejected even if the compiler has an internal carrier representation. | A custom policy might be implicit in policy selection elsewhere; this sentence only forbids inference from the compiler carrier, not from other policy metadata. |
"Firmware and other boundaries may permit aggregate results when their selected policy does." | Aggregate results are forbidden unless the selected policy explicitly permits them for that boundary kind. | "May permit" states permission only; whether the compiler must reject aggregate results when the policy is silent is not stated. |

## Terms used without definition in this chapter

- calling policy
- semantic type
- public normalized structure
- ABI fact
- native leaf
- concrete foreign shape
- private compiler layout
- public ABI
- fixed arrays / fixed records
- public layout
- ABI class
- erased field
- semantic/proof identity
- Terminal Psi
- by-value ABI carrier
- case-bearing aggregate
- unresolved generic aggregates
- public ABI shape
- `BoundarySignature`
- shape nodes / parameter/result roots
- fixed-array nodes
- record nodes
- native telescope
- semantic formals
- native-only callback entries
- semantic runtime node
- ABI shape and placement
- opaque by-value roots
- representation application
- target-semantics version
- closed shape
- movement
- lifecycle
- evidence origin
- boundary-application identity
- C array decay
- slices / text views / vectors / bounded text
- bare carriers
- default native policies
- leaf (boundary leaf)
- terminator
- descriptor layout
- pointer/length record
- custom policy
- compiler's current carrier
- checked adapter
- safe Omega view
- Foreign storage (referenced chapter)
- inert pointers
- checked marshaling
- retained custody
- synchronous non-retaining leaf
- call-scoped borrowed-out contract
- calling plan
- pinned loan
- registration protocol
- `Utf8` qualification
- proof objects
- foreign boundary
- process entry
- selected requirement
- exit-status scalar
- firmware (as boundary kind)
- selected policy
- `Ptr` / `u8` / `u64` / `i32` (example types only)

CHAPTER COMPLETE
