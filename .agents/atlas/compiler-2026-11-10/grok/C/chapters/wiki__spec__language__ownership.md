# Ownership and multiplicity

Source: `wiki/spec/language/ownership.md` lines 1–344 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
LANGUAGE_OWNERSHIP1 | "Copy and discard." | Ownership and multiplicity | Unrestricted (`[copy]`) values may be copied and discarded per permitted use | at the operation | 2
LANGUAGE_OWNERSHIP2 | "Move at most once; discard through eligible cleanup." | Ownership and multiplicity | Affine owned data must not be moved more than once; discard only via eligible cleanup | on every edge | 2
LANGUAGE_OWNERSHIP3 | "Transfer or consume exactly once." | Ownership and multiplicity | Linear (`[linear]`) values must be transferred or consumed exactly one time | on every edge | 2
LANGUAGE_OWNERSHIP4 | "`[copy]` and `[linear]` are mutually exclusive." | Ownership and multiplicity | A type must not declare both `[copy]` and `[linear]` | not stated | 2
LANGUAGE_OWNERSHIP5 | "Structural records, active sum payloads, and concrete generic substitutions retain every contained obligation." | Ownership and multiplicity | Contained ownership/cleanup obligations must remain in structural records, active sum payloads, and concrete generic substitutions | not stated | 1
LANGUAGE_OWNERSHIP6 | "A nominal container's declared multiplicity cannot erase linear debt in an active payload." | Ownership and multiplicity | Container multiplicity must not remove linear obligations from an active payload | not stated | 2
LANGUAGE_OWNERSHIP7 | "An inactive case creates no obligation for its absent payload." | Ownership and multiplicity | Inactive sum cases must not create obligations for payloads that are absent | not stated | 1
LANGUAGE_OWNERSHIP8 | "Copyability requires compatible owned contents and cannot duplicate unique cleanup responsibility." | Ownership and multiplicity | `[copy]` types must have copy-compatible contents and must not duplicate unique cleanup responsibility | not stated | 2
LANGUAGE_OWNERSHIP9 | "Shared reference and borrowed-view fields may occur in `[copy]` data when every field satisfies the ordinary copy rules." | Ownership and multiplicity | `[copy]` records may include shared-reference or borrowed-view fields only if every field satisfies ordinary copy rules | not stated | 2
LANGUAGE_OWNERSHIP10 | "Copying the record duplicates its shared loan carriers, not its referents; it preserves their exact backing, qualification, provenance, and lifetime obligations." | Ownership and multiplicity | Record copy must duplicate loan carriers only and preserve backing, qualification, provenance, and lifetime obligations on those carriers | at the operation | 2
LANGUAGE_OWNERSHIP11 | "It grants neither a longer lifetime nor additional carry or cross-activation sharing permission." | Ownership and multiplicity | Record copy must not extend lifetime or add carry or cross-activation sharing permission | at the operation | 2
LANGUAGE_OWNERSHIP12 | "Exclusive `&mut` and `&write` fields do not qualify." | Ownership and multiplicity | Records with exclusive `&mut` or `&write` fields must not be accepted as `[copy]` solely on other fields | not stated | 2
LANGUAGE_OWNERSHIP13 | "A record still requires an explicit `[copy]` declaration; copyable fields do not change the affine default." | Ownership and multiplicity | Records need explicit `[copy]` declaration; copyable field types must not make the record affine-by-default instead of default affine | not stated | 2
LANGUAGE_OWNERSHIP14 | "Machine-valued fields and `some` packages follow these same multiplicity rules." | Ownership and multiplicity | Machine-valued fields and `some` packages must obey the same multiplicity rules as ordinary data | not stated | 10
LANGUAGE_OWNERSHIP15 | "A mutable reference inside a package remains exclusive." | Ownership and multiplicity | Mutable references inside a package must remain exclusive | not stated | 2
LANGUAGE_OWNERSHIP16 | "Opening a package creates no duplicate loan or cleanup authority; moves and permitted copies retain the package witness association and exact borrowed places." | Ownership and multiplicity | Package open must not duplicate loan or cleanup authority; moves and permitted copies must retain package witness and exact borrowed places | at the operation | 2
LANGUAGE_OWNERSHIP17 | "Explicit `copy` and `linear` requests are checked at the declaration; failure rejects rather than weakening the request." | Property declarations | Explicit `copy`/`linear` on declarations must be checked; failure must reject, not weaken the request | not stated | 2
LANGUAGE_OWNERSHIP18 | "Fields and locals do not redeclare the multiplicity of their type: every use of `T` receives the property fixed by `T`'s declaration and concrete generic application." | Property declarations | Uses of `T` must use multiplicity fixed by `T`'s declaration and concrete application; fields/locals must not redeclare it | not stated | 2
LANGUAGE_OWNERSHIP19 | "A conformance, selected provider, or lowering cannot change it." | Property declarations | Conformance, provider selection, and lowering must not alter multiplicity | not stated | 2
LANGUAGE_OWNERSHIP20 | "Opaque boundary properties require accepted provider evidence." | Property declarations | Opaque boundary properties must have accepted provider evidence | not stated | 9
LANGUAGE_OWNERSHIP21 | "Ordinary packages cannot append structural properties to foreign types or grant themselves opaque claims." | Property declarations | Ordinary packages must not add structural properties to foreign types or self-grant opaque claims | not stated | 9
LANGUAGE_OWNERSHIP22 | "Apart from specified derived judgments, omitted properties are not inferred, and there is no negative property syntax." | Property declarations | Omitted multiplicity-related properties must not be inferred except listed derived judgments; negative property syntax must not be accepted | not stated | 2
LANGUAGE_OWNERSHIP23 | "Colon bounds such as `T: copy` and detached attribute-prefix forms do not declare these properties." | Property declarations | `T: copy` colon bounds and detached attribute-prefix forms must not be treated as declaring `[copy]`/`[linear]` properties | not stated | 2
LANGUAGE_OWNERSHIP24 | "Implicit zero-filling establishes no linear obligation." | Establishment and erasure | Implicit zero-fill must not create linear obligations | not stated | 2
LANGUAGE_OWNERSHIP25 | "Explicit construction of an all-zero linear value is legal and owed when that value meets its type's validity conditions." | Establishment and erasure | All-zero linear construction must establish owed linear obligation when validity conditions hold | at the operation | 2
LANGUAGE_OWNERSHIP26 | "An explicit [`..` literal](data_and_literals.md#explicit-field-omission) remains ordinary checked construction: it cannot fabricate a resource, and any established linear value carries its normal obligation." | Establishment and erasure | `..` literal construction must not fabricate resources; established linear values from it carry normal obligations | at the operation | 2
LANGUAGE_OWNERSHIP27 | "The live case owns the task obligation; empty storage is not a fabricated consumed task." | Establishment and erasure | Live sum case must own task obligation; empty case must not fabricate consumed task | not stated | 2
LANGUAGE_OWNERSHIP28 | "Establishing one value creates one obligation." | Establishment and erasure | One established value must create exactly one ownership obligation | at the operation | 2
LANGUAGE_OWNERSHIP29 | "Moves, calls, returns, receives, and storage operations transfer it rather than create copies." | Establishment and erasure | Moves, calls, returns, receives, and storage ops must transfer obligations rather than duplicate them for affine/linear carriers | at the operation | 2
LANGUAGE_OWNERSHIP30 | "Consumable permission is an affine or linear Type carrier, even if it has no runtime bytes." | Establishment and erasure | Permission Type carriers without runtime bytes must still use affine or linear multiplicity rules | not stated | 2
LANGUAGE_OWNERSHIP31 | "Zero size does not require an `[erased]` marker and does not remove ownership." | Establishment and erasure | Zero-sized types must not require `[erased]` to retain ownership; zero size must not erase ownership checking | not stated | 2
LANGUAGE_OWNERSHIP32 | "Explicitly erased Type witnesses likewise retain identity, multiplicity, validity, and conservation." | Establishment and erasure | Erased Type witnesses must retain identity, multiplicity, validity, and conservation obligations | not stated | 2
LANGUAGE_OWNERSHIP33 | "The permission context tracks exact established places, multiplicity, access, loans, lifetime, and provenance." | Paths and control flow | Permission context must track exact places, multiplicity, access, loans, lifetime, and provenance | on every edge | 2
LANGUAGE_OWNERSHIP34 | "Every normal path accounts for each live linear obligation by transfer or an authorized terminal disposition." | Paths and control flow | Each normal control-flow path must dispose each live linear obligation by transfer or authorized terminal disposition | on every edge | 2
LANGUAGE_OWNERSHIP35 | "At an actual reconvergence, predecessor paths must agree on the live ownership frontier." | Paths and control flow | At actual reconvergence, predecessor paths must agree on live ownership frontier | on every edge | 2
LANGUAGE_OWNERSHIP36 | "Independently terminating outcome cases need not choose the same disposition: success may consume an input while failure returns it, provided each case satisfies its exact published contract." | Paths and control flow | Independently terminating outcomes may differ in disposition if each satisfies its published contract | not stated | 3
LANGUAGE_OWNERSHIP37 | "A path cannot consume an obligation and also return it or silently omit it." | Paths and control flow | A single path must not both consume and return the same obligation nor omit it without accounting | on every edge | 2
LANGUAGE_OWNERSHIP38 | "[Process exit](process_exit.md#abandonment-and-survivors) explicitly ends its bound domain with abandonment, including outstanding linear obligations." | Paths and control flow | Process exit must end bound domain with abandonment including outstanding linear obligations | at normal return | 3
LANGUAGE_OWNERSHIP39 | "It is not an ordinary consumer or proof of release." | Paths and control flow | Process exit must not count as ordinary consumer or release proof for obligations | not stated | 3
LANGUAGE_OWNERSHIP40 | "No post-exit frontier merges into a returning branch, and no successful disposition receipt is fabricated." | Paths and control flow | Post-exit frontier must not merge into returning branches; must not fabricate disposition receipts | on every edge | 2
LANGUAGE_OWNERSHIP41 | "Cross-domain survivor guarantees still require their specified evidence; domain-ending authority cannot waive them." | Paths and control flow | Cross-domain survivor claims must still meet specified evidence; domain-ending authority must not waive them | not stated | 8
LANGUAGE_OWNERSHIP42 | "Partial moves retain unselected siblings and make ancestors unavailable for whole-value use or disposal." | Paths and control flow | Partial moves must retain unselected siblings and block ancestors from whole-value use or disposal | at the operation | 2
LANGUAGE_OWNERSHIP43 | "Transparent child paths do not duplicate a nominal root obligation." | Paths and control flow | Transparent child paths must not duplicate nominal root obligation | not stated | 2
LANGUAGE_OWNERSHIP44 | "They cannot evade a nominal cleanup hook's whole-value entitlement." | Paths and control flow | Child paths must not evade nominal whole-value cleanup hook entitlement | not stated | 2
LANGUAGE_OWNERSHIP45 | "Duplicate and overlapping moves reject." | Paths and control flow | Duplicate or overlapping moves must be rejected | at the operation | 2
LANGUAGE_OWNERSHIP46 | "An owned extraction must identify the unique moved element and retain its residual frontier." | Paths and control flow | Owned extraction must name unique moved element and retain residual ownership frontier | at the operation | 2
LANGUAGE_OWNERSHIP47 | "Unsupported runtime-indexed extraction cannot fall back to forgetting the root." | Paths and control flow | Runtime-indexed extraction without support must reject rather than forget root obligation | at the operation | 2
LANGUAGE_OWNERSHIP48 | "Source and result claims keep their provenance through calls, terminal transfers, wrappers, and generic substitutions." | Paths and control flow | Claims must preserve provenance through calls, transfers, wrappers, and generic substitution | across a pause | 2
LANGUAGE_OWNERSHIP49 | "A moved input returned in an outcome remains that same obligation." | Paths and control flow | Obligation returned in outcome must be the same as moved input obligation | at normal return | 2
LANGUAGE_OWNERSHIP50 | "Ambiguous opaque multi-resource output maps reject; equal bytes or matching result types are not transfer evidence." | Paths and control flow | Ambiguous opaque multi-resource output maps must reject; byte equality or matching result types must not prove transfer | at normal return | 2
LANGUAGE_OWNERSHIP51 | "Shared observation under its loan contract." | Borrows and aliases | `&T` access must be shared observation under the loan contract | at the operation | 2
LANGUAGE_OWNERSHIP52 | "Exclusive observation and mutation." | Borrows and aliases | `&mut T` access must be exclusive observation and mutation | at the operation | 2
LANGUAGE_OWNERSHIP53 | "Exclusive mutation without observation of existing content." | Borrows and aliases | `&write T` access must allow exclusive mutation without observing existing content | at the operation | 2
LANGUAGE_OWNERSHIP54 | "Reader and writer lists restrict observation and mutation of the exact qualified subject, including a qualified field reached through an owned parent." | Borrows and aliases | Reader/writer lists must restrict observation and mutation to exact qualified subject including qualified fields through owned parents | at the operation | 2
LANGUAGE_OWNERSHIP55 | "A restricted reference may be forwarded to an authorized machine; merely owning or borrowing it does not activate that machine's grant." | Borrows and aliases | Owning or borrowing a restricted reference must not activate machine grant without authorized forwarding | at a call | 10
LANGUAGE_OWNERSHIP56 | "Returning or casting a reference cannot silently make an invocation-scoped grant permanent." | Borrows and aliases | Return or cast of reference must not make invocation-scoped grant permanent without checking | at normal return | 2
LANGUAGE_OWNERSHIP57 | "Shared loans may duplicate or reborrow, while exclusive loans must not overlap incompatibly." | Borrows and aliases | Shared loans may duplicate/reborrow; exclusive loans must not overlap incompatibly | at the operation | 2
LANGUAGE_OWNERSHIP58 | "A by-value reference transfers or copies its loan carrier, not ownership of its referent." | Borrows and aliases | By-value reference move/copy must transfer or copy loan carrier only, not referent ownership | at the operation | 2
LANGUAGE_OWNERSHIP59 | "Borrowed arguments continue to denote the original storage, not a caller-side snapshot chosen because the referent is small." | Borrows and aliases | Borrowed arguments must denote original storage, not caller-side snapshot | at a call | 2
LANGUAGE_OWNERSHIP60 | "`&write T` requires an existing valid `T`, not vacant construction storage." | Borrows and aliases | `&write T` formation must require existing valid `T`, not vacant construction storage | at the operation | 2
LANGUAGE_OWNERSHIP61 | "Explicit attenuation from mutable access cannot be reversed into readable access." | Borrows and aliases | Attenuation from mutable access must not be reversed to readable access | at the operation | 2
LANGUAGE_OWNERSHIP62 | "Exclusivity prevents reading old contents through a second alias." | Borrows and aliases | Exclusive write/mut access must prevent reading old contents via another alias | at the operation | 2
LANGUAGE_OWNERSHIP63 | "Metadata, written inputs, and supplied proof facts may support a write; loading the referent may not." | Borrows and aliases | Writes may use metadata, written inputs, and proof facts but must not load referent contents when forbidden | at the operation | 2
LANGUAGE_OWNERSHIP64 | "Reborrowing retains exact immediate parent lineage." | Borrows and aliases | Reborrows must retain exact immediate parent lineage | at the operation | 2
LANGUAGE_OWNERSHIP65 | "Shared children freeze the parent's mutation; exclusive children suspend the corresponding parent branch." | Borrows and aliases | Shared reborrow children must freeze parent mutation; exclusive children must suspend parent branch | at the operation | 2
LANGUAGE_OWNERSHIP66 | "Restoration needs the complete child/cohort closure, not merely expiration of a convenient alias name." | Borrows and aliases | Parent restoration must require full child/cohort closure, not only alias name expiration | at the operation | 2
LANGUAGE_OWNERSHIP67 | "Access attenuation does not change the parent's original authority." | Borrows and aliases | Access attenuation must not reduce parent's original authority below attenuated child's view incorrectly | not stated | 2
LANGUAGE_OWNERSHIP68 | "Suspension does not end a loan, restore its parent's access or transfer ownership of its referent." | Borrows and aliases | Suspension must not end loan, restore parent access, or transfer referent ownership | across a pause | 2
LANGUAGE_OWNERSHIP69 | "Borrowed arguments and other retained loans remain valid for the whole incomplete call, including parked intervals, under [carry and runtime preservation](../resources/carry.md#borrows-across-suspension)." | Borrows and aliases | Borrowed arguments and retained loans must remain valid for entire incomplete call including parked intervals per carry rules | across a pause | 2
LANGUAGE_OWNERSHIP70 | "Return, cancellation and replacement may release them only through their ordinary checked closure/disposition paths." | Borrows and aliases | Release on return/cancel/replace must use only ordinary checked closure/disposition paths | at normal return | 2
LANGUAGE_OWNERSHIP71 | "A final call in a transition arm supplies no exception and does not allow reclaiming backing still needed by its callee." | Borrows and aliases | Final transition-arm call must not exempt reclaiming backing still needed by callee | at a call | 3
LANGUAGE_OWNERSHIP72 | "The move transfers the removed value's exact custody to its destination and records a restoration obligation on the original place and loan." | Borrowed-storage invariant windows | Move out through `&mut` must transfer removed value custody to destination and record restoration on original place and loan | at the operation | 2
LANGUAGE_OWNERSHIP73 | "The remaining storage does not own a second copy." | Borrowed-storage invariant windows | Remaining storage after partial move through `&mut` must not own a second copy of removed value | not stated | 2
LANGUAGE_OWNERSHIP74 | "Zero-filled or stale bytes do not fill the hole." | Borrowed-storage invariant windows | Hole left by invariant window must not be treated as filled by zero or stale bytes | at the operation | 2
LANGUAGE_OWNERSHIP75 | "While the window is open, the absent subtree cannot be read, borrowed as a value, moved again, or disposed." | Borrowed-storage invariant windows | Open window must forbid read, value-borrow, move, or dispose of absent subtree | at the operation | 2
LANGUAGE_OWNERSHIP76 | "Ancestors are unavailable for whole-value observation, calls, transfer, and cleanup." | Borrowed-storage invariant windows | While window open, ancestors must be unavailable for whole-value observation, calls, transfer, cleanup | at the operation | 2
LANGUAGE_OWNERSHIP77 | "Disjoint established siblings remain usable under ordinary loan and dependent-fact rules." | Borrowed-storage invariant windows | Disjoint siblings remain usable under ordinary loan and dependent-fact rules when window open | at the operation | 2
LANGUAGE_OWNERSHIP78 | "Existing restrictions on partial moves, including nominal whole-value cleanup, still apply; `&write` does not gain permission to extract old content." | Borrowed-storage invariant windows | Partial-move and nominal cleanup restrictions remain; `&write` must not gain extract-old-content permission | at the operation | 2
LANGUAGE_OWNERSHIP79 | "Assignment of a valid owned replacement to the exact vacant place closes the ownership window." | Borrowed-storage invariant windows | Valid owned replacement at exact vacant place must close ownership window | at the operation | 2
LANGUAGE_OWNERSHIP80 | "The replacement need not be the removed value, but both values' obligations must be accounted for." | Borrowed-storage invariant windows | Replacement need not equal removed value; obligations for both must be accounted | at the operation | 2
LANGUAGE_OWNERSHIP81 | "There is no old field value to dispose at that store." | Borrowed-storage invariant windows | Store closing window must not require disposing old field value at that assignment | at the operation | 2
LANGUAGE_OWNERSHIP82 | "Required type, domain, contained-loan, and parent-coupling obligations must also be restored before an observation relying on them." | Borrowed-storage invariant windows | Type, domain, contained-loan, and parent-coupling obligations must be restored before dependent observation | at the operation | 2
LANGUAGE_OWNERSHIP83 | "Predicate evidence alone cannot establish absent content." | Borrowed-storage invariant windows | Predicate evidence alone must not establish content that is absent in the hole | at the operation | 8
LANGUAGE_OWNERSHIP84 | "Every return or terminal transfer from the region owing restoration, scope expiration releasing its custody, or restoration of the borrow to its parent must close the window." | Borrowed-storage invariant windows | Return, terminal transfer, scope expiration, or parent restoration must close restoration window when owed | at normal return | 2
LANGUAGE_OWNERSHIP85 | "Calls may consume the detached value or use disjoint storage only when their complete access and outcome contracts cannot expose the incomplete owner." | Borrowed-storage invariant windows | Calls during open window may consume detached value or use disjoint storage only if contracts cannot expose incomplete owner | at a call | 3
LANGUAGE_OWNERSHIP86 | "Unknown access is not evidence of non-observation." | Borrowed-storage invariant windows | Unknown callee access must not count as proof of non-observation of incomplete owner | not stated | 3
LANGUAGE_OWNERSHIP87 | "Boundary/capability calls and other [invariant-window fences](dependent_values.md#invariant-windows) remain in force." | Borrowed-storage invariant windows | Invariant-window fences must remain enforced during open window | at a call | 2
LANGUAGE_OWNERSHIP88 | "At reconvergence, predecessors must agree on the exact live ownership and open restoration obligations; one repaired branch does not repair another." | Borrowed-storage invariant windows | Reconvergence must agree on live ownership and open restoration obligations per branch | on every edge | 2
LANGUAGE_OWNERSHIP89 | "Suspension must retain the exclusive loan, exact hole, removed-value custody, and restoration obligation without exposing the incomplete owner." | Borrowed-storage invariant windows | Suspension with open window must retain exclusive loan, hole, removed-value custody, and restoration obligation without exposing incomplete owner | across a pause | 2
LANGUAGE_OWNERSHIP90 | "Its resume and cancellation paths must satisfy their ordinary carry and cleanup contracts; otherwise suspension with the window open rejects." | Borrowed-storage invariant windows | Resume/cancel with open window must satisfy carry and cleanup contracts or reject suspension | across a pause | 2
LANGUAGE_OWNERSHIP91 | "Recoverable failure does not cancel restoration debt." | Borrowed-storage invariant windows | Recoverable failure must not cancel restoration obligation | not stated | 2
LANGUAGE_OWNERSHIP92 | "Crash and process exit follow their existing abandonment and survivor contracts, not an invented cleanup successor or rollback; abandoning a window supplies no survivor-safety theorem." | Borrowed-storage invariant windows | Crash/exit must use existing abandonment/survivor contracts; must not invent cleanup successor; abandoning window must not prove survivor safety | not stated | 3
LANGUAGE_OWNERSHIP93 | "[Terminal ownership](../terminal-psi/ownership.md#borrowed-storage-restoration) must independently reconstruct the window rather than trust producer approval." | Borrowed-storage invariant windows | Terminal lowering must independently reconstruct borrowed-storage window | not stated | 2
LANGUAGE_OWNERSHIP94 | "Ownership is determined by each ordinary parameter's type: `value: T` receives an owned value; `value: &T` and `value: &mut T` receive references." | Consumers and cleanup | Parameter ownership category must follow type: `T` owned, `&T`/`&mut T` borrowed | at a call | 3
LANGUAGE_OWNERSHIP95 | "A qualified machine name adds no hidden argument or ownership category." | Consumers and cleanup | Qualified machine name must not add hidden argument or ownership category | at a call | 3
LANGUAGE_OWNERSHIP96 | "All calls preserve the parameter's ordinary transfer, loan, cleanup and disposition obligations." | Consumers and cleanup | Calls must preserve transfer, loan, cleanup, and disposition obligations from parameter types | at a call | 3
LANGUAGE_OWNERSHIP97 | "There is no terminal-consumer annotation and no inference from the machine's name." | Consumers and cleanup | Consumption must not be inferred from machine name or terminal-consumer annotation | at a call | 3
LANGUAGE_OWNERSHIP98 | "A call returning an outcome containing the obligation transfers it back." | Consumers and cleanup | Outcome containing obligation must transfer obligation back to caller | at normal return | 3
LANGUAGE_OWNERSHIP99 | "A normal return without it requires its authorized disposition within the callee." | Consumers and cleanup | Normal return without obligation in result requires authorized disposition inside callee | at normal return | 3
LANGUAGE_OWNERSHIP100 | "Pending or failure outcomes of incomplete consuming operations retain the live input rather than discard it." | Consumers and cleanup | Pending/failure outcomes of incomplete consuming ops must retain live input | at normal return | 3
LANGUAGE_OWNERSHIP101 | "A call with a borrowed argument cannot silently become an owned consumer." | Consumers and cleanup | Borrowed-argument call must not silently consume owned value | at a call | 3
LANGUAGE_OWNERSHIP102 | "Affine ownership permits eligible automatic cleanup." | Consumers and cleanup | Affine types may use eligible automatic cleanup when rules satisfied | not stated | 2
LANGUAGE_OWNERSHIP103 | "Linear ownership permits it only when the type owner declares that exact plan as a valid terminal disposition." | Consumers and cleanup | Linear automatic cleanup allowed only when type owner declares exact plan as valid terminal disposition | not stated | 2
LANGUAGE_OWNERSHIP104 | "The hook must return normally and be infallible, nonblocking, non-suspending, free of abnormal outcomes, and require no runtime authority beyond its explicit value parameter." | Consumers and cleanup | Cleanup hook must be normal-return, infallible, nonblocking, non-suspending, no abnormal outcomes, no extra runtime authority | not stated | 2
LANGUAGE_OWNERSHIP105 | "Termination alone is insufficient: the transitive hook contract must exclude process exit as well as crashes." | Consumers and cleanup | Cleanup hook transitive contract must exclude process exit and crashes as sufficient termination | not stated | 2
LANGUAGE_OWNERSHIP106 | "The compiler begins consumption before temporarily lending the value to the hook's ordinary `value: &mut T` parameter; this is not consumption by an ordinary borrowed call." | Consumers and cleanup | Nominal cleanup must begin consumption before `&mut T` lend to hook; must not model as ordinary borrowed call consumption | at a call | 2
LANGUAGE_OWNERSHIP107 | "Inheritance and conjunction permit zero or one distinct resolved machine application: identical selections coalesce and differing selections conflict, without overrides or sequencing." | Consumers and cleanup | `dropped by` inheritance/conjunction must resolve zero or one cleanup machine; identical coalesce, differing conflict | not stated | 2
LANGUAGE_OWNERSHIP108 | "Its selected cleanup operation is an independent static machine with an explicit `value: &mut T` parameter, not a data-owned method." | Consumers and cleanup | Selected cleanup must be independent static machine with `value: &mut T`, not data method | not stated | 2
LANGUAGE_OWNERSHIP109 | "The exact owner/type/disposition identity, not a receiver flag or `T::drop` name lookup, selects the compiler cleanup edge." | Consumers and cleanup | Cleanup edge selection must use owner/type/disposition identity, not receiver flag or `T::drop` lookup | not stated | 2
LANGUAGE_OWNERSHIP110 | "The legacy `machine T::drop` attachment is retired under `no-data-code-attachment`; source invocation of the selected edge-only hook rejects." | Consumers and cleanup | Legacy `machine T::drop` attachment must reject; source invocation of edge-only hook must reject | at a call | 2
LANGUAGE_OWNERSHIP111 | "The hook receives and must return a whole valid value before structural field cleanup." | Consumers and cleanup | Cleanup hook must receive and return whole valid value before structural field cleanup | at a call | 2
LANGUAGE_OWNERSHIP112 | "An owned occurrence with a selected whole-value hook cannot be partially moved; meaningful decomposition requires an explicit consuming machine." | Consumers and cleanup | Whole-value hook blocks partial move; decomposition requires explicit consuming machine | at the operation | 2
LANGUAGE_OWNERSHIP113 | "Prerequisites must be proved locally or already authored in `requires`; neither public nor private body analysis may invent a new caller demand." | Consumers and cleanup | Cleanup prerequisites must be local proof or authored `requires`; body analysis must not invent new caller demands | not stated | 8
LANGUAGE_OWNERSHIP114 | "Generic rows remain symbolic until substitution supplies the exact structural plans and contracts." | Consumers and cleanup | Generic cleanup rows stay symbolic until substitution gives exact plans and contracts | not stated | 7
LANGUAGE_OWNERSHIP115 | "Containers need no nominal `Disposable` bound, and an instantiation lacking legal element disposition rejects." | Consumers and cleanup | Container instantiation without legal element disposition must reject; no required `Disposable` bound | not stated | 2
LANGUAGE_OWNERSHIP116 | "Source `suspend`/`block` acknowledgements apply when needed but do not discharge custody." | Consumers and cleanup | `suspend`/`block` acknowledgements must not discharge custody | across a pause | 2
LANGUAGE_OWNERSHIP117 | "A task without automatic disposition must be finished or transferred; `request_cancel` retains its claim." | Consumers and cleanup | Tasks without auto disposition must be finished or transferred; `request_cancel` keeps claim | not stated | 2
LANGUAGE_OWNERSHIP118 | "Scope exit cannot implicitly wait, detach, or lose a bound task." | Consumers and cleanup | Scope exit must not implicitly wait, detach, or lose bound task | not stated | 2
LANGUAGE_OWNERSHIP119 | "Automatic abandonment requires a contract declaring implicit disposal harmless." | Consumers and cleanup | Automatic abandonment allowed only with contract that implicit disposal is harmless | not stated | 2
LANGUAGE_OWNERSHIP120 | "A claim whose loss permanently withholds external capacity remains linear with an explicit terminal choice: release, or an authorized abandonment recording the loss." | Consumers and cleanup | Claims whose loss withholds external capacity must stay linear with explicit release or authorized abandonment | not stated | 2
LANGUAGE_OWNERSHIP121 | "Enqueuing deferred reclamation transfers the obligation rather than discharging it; the queue needs capacity, servicing, progress and resource contracts covering eventual discharge." | Consumers and cleanup | Deferred reclamation queue must transfer obligation; queue must meet capacity/servicing/progress/resource contracts | at the operation | 2
LANGUAGE_OWNERSHIP122 | "Assignment follows the [destination-first schedule](expressions.md#evaluation-schedule): capture the destination, evaluate the replacement, then replace there under the ordinary ownership rules." | Construction and disposal order | Assignment must use destination-first schedule with ordinary ownership on replace | at the operation | 0
LANGUAGE_OWNERSHIP123 | "Any displaced-value cleanup and installation name that same captured place, even when the right-hand side changes its selector." | Construction and disposal order | Displaced-value cleanup/installation must target captured destination place | at the operation | 2
LANGUAGE_OWNERSHIP124 | "Capturing does not itself dispose of or move the old value, and does not waive the loans or backing-lifetime obligations that keep the destination valid." | Construction and disposal order | Destination capture must not dispose/move old value or waive loans/lifetimes keeping destination valid | at the operation | 2
LANGUAGE_OWNERSHIP125 | "Independent roots clean up in reverse declaration order, so later locals precede earlier by-value parameters." | Construction and disposal order | Independent roots cleanup must follow reverse declaration order | not stated | 2
LANGUAGE_OWNERSHIP126 | "The nominal whole-value hook precedes structural field cleanup; only the active sum payload is present." | Construction and disposal order | Whole-value hook must run before structural field cleanup with only active sum payload present | not stated | 2
LANGUAGE_OWNERSHIP127 | "Borrow and ownership dependencies must fit this order: an owner cannot die before cleanup depending on its borrow." | Construction and disposal order | Cleanup order must respect borrow dependencies; owner must not die before dependent cleanup | not stated | 2
LANGUAGE_OWNERSHIP128 | "A different release protocol requires an explicit owner or consumer, not reconstruction of dynamic acquisition history." | Construction and disposal order | Alternate release protocol must use explicit owner/consumer, not inferred acquisition history | not stated | 2
LANGUAGE_OWNERSHIP129 | "Named record/case literals establish fields once in authored expression order." | Construction and disposal order | Record/case literals must establish fields once in authored expression order | at the operation | 1
LANGUAGE_OWNERSHIP130 | "Abandoning partial construction disposes the established prefix in reverse establishment order; partial call-argument staging follows the same rule." | Construction and disposal order | Partial construction abandon must dispose established prefix reverse order; partial args same | not stated | 2
LANGUAGE_OWNERSHIP131 | "After successful construction, aggregate cleanup instead follows recursive reverse declaration order." | Construction and disposal order | Post-construction aggregate cleanup must follow recursive reverse declaration order | not stated | 2
LANGUAGE_OWNERSHIP132 | "Canonical field identity and physical layout do not reorder evaluation or partial-construction cleanup." | Construction and disposal order | Layout/field identity must not reorder evaluation or partial-construction cleanup | not stated | 1
LANGUAGE_OWNERSHIP133 | "[Runtime-count owned runs](../resources/typed_runs.md) retain established elements and their obligations in an erased ordered-cell custody summary." | Construction and disposal order | Runtime-count owned runs must retain elements and obligations in ordered-cell custody summary | not stated | 6
LANGUAGE_OWNERSHIP134 | "An explicit consuming traversal can discharge cells one at a time while preserving the remaining run; it needs no runtime ownership bitmap." | Construction and disposal order | Consuming traversal may discharge cells one at a time preserving remainder without ownership bitmap | at the operation | 6
LANGUAGE_OWNERSHIP135 | "A mutable element borrow uses the same restoration windows as other borrowed storage." | Construction and disposal order | Mutable element borrow in runs must use same borrowed-storage restoration windows | at the operation | 2
LANGUAGE_OWNERSHIP136 | "Removing a cell repairs the dense occupied range or replaces the value before releasing the loan; permanent holes require an explicit optional-element representation." | Construction and disposal order | Cell removal must repair dense range or replace value before loan release; permanent holes need optional representation | at the operation | 6
LANGUAGE_OWNERSHIP137 | "Literal fixed arrays establish elements in increasing index order and clean the statically known live residual set in decreasing order, recursively and excluding moved elements." | Construction and disposal order | Fixed array literal establish increasing index; cleanup decreasing order on static live residual, exclude moved | not stated | 6
LANGUAGE_OWNERSHIP138 | "Authored moves retain authored order." | Construction and disposal order | Authored moves in fixed arrays must retain authored order | at the operation | 6
LANGUAGE_OWNERSHIP139 | "No runtime liveness bitmap or data-dependent cleanup loop replaces this static schedule." | Construction and disposal order | Static cleanup schedule must not be replaced by runtime liveness bitmap or data-dependent loop | not stated | 2
LANGUAGE_OWNERSHIP140 | "Each normal edge retains its own exact transfers and cleanup; a crash has no cleanup successor." | Construction and disposal order | Normal edges keep exact transfers/cleanup; crash paths must not imply cleanup successor | on every edge | 2
LANGUAGE_OWNERSHIP141 | "Only an exact qualification's owner-unique `Content<Owner, A>` projection adds [content conservation](../resources/content_custody.md)." | Content and carry | Content conservation applies only to exact qualification's owner-unique `Content<Owner, A>` projection | not stated | 8
LANGUAGE_OWNERSHIP142 | "Its n-ary theorem is additional to accounting for each whole claim; per-child containment or equal scalar totals cannot rule out overlapping children." | Content and carry | N-ary content theorem is additional; per-child containment or equal scalars must not disprove overlapping children | not stated | 8
LANGUAGE_OWNERSHIP143 | "Borrowed subranges and access views may retain one owned root without splitting its content." | Content and carry | Borrowed subranges/views may keep one owned root without splitting content | not stated | 2
LANGUAGE_OWNERSHIP144 | "Independently owned children require conservation." | Content and carry | Independently owned children must meet content conservation requirements | not stated | 2
LANGUAGE_OWNERSHIP145 | "Merging cannot restore discarded permissions; authority that must return is a separate loan or claim." | Content and carry | Merge must not restore discarded permissions; returning authority must be separate loan/claim | at the operation | 2
LANGUAGE_OWNERSHIP146 | "A counted pool of `n` units is not a protocol requiring delivery to `n` distinct destinations." | Content and carry | Counted pool of `n` units must not be enforced as delivery to `n` distinct destinations | not stated | 2
LANGUAGE_OWNERSHIP147 | "[Carry](../resources/carry.md) independently checks suspension, CPU, thread, and address transitions." | Content and carry | Carry obligations for suspension, CPU, thread, address transitions must be checked independently of multiplicity | not stated | 2
LANGUAGE_OWNERSHIP148 | "Copyability is not concurrent shareability." | Content and carry | `[copy]` must not be treated as concurrent shareability permission | not stated | 2
LANGUAGE_OWNERSHIP149 | "Erasure or qualification forgetting cannot drop carry obligations or grant migration." | Content and carry | Erasure/qualification forgetting must not drop carry obligations or grant migration | not stated | 2
LANGUAGE_OWNERSHIP150 | "Multiplicity may leave executable representation only after complete ownership and cleanup checking; artifacts retain the required conservation evidence." | Content and carry | Executable representation from multiplicity allowed only after full ownership/cleanup checking; artifacts keep conservation evidence | not stated | 2
LANGUAGE_OWNERSHIP151 | "Every published permission event needs exact realization or a checked no-code reason." | Content and carry | Each published permission event must have exact realization or checked no-code reason | not stated | 2
LANGUAGE_OWNERSHIP152 | "Missing or invalid realization cannot publish a partial permission ledger." | Content and carry | Missing/invalid realization must not publish partial permission ledger | not stated | 2
LANGUAGE_OWNERSHIP153 | "Lowercase bracket properties attach to the data declaration or type parameter, as in `data Box<T [copy]> [copy]`." | Property declarations | `[copy]`/`[linear]` bracket properties must attach only to data declarations or type parameters | not stated | 2
LANGUAGE_OWNERSHIP154 | "They generate no callable behavior." | Property declarations | Multiplicity bracket properties must not generate callable behavior | not stated | 2
LANGUAGE_OWNERSHIP155 | "`sized` and structural carry are derived judgments, not authored requests." | Property declarations | `sized` and structural carry must be derived judgments only, not accepted as authored bracket requests | not stated | 2
LANGUAGE_OWNERSHIP156 | "Authorized calls may delegate bounded access to helpers through existing argument and reborrow rules." | Borrows and aliases | Delegation of bounded access to helpers must use only existing argument and reborrow rules | at a call | 2
LANGUAGE_OWNERSHIP157 | "The permission context retains that authorization separately from multiplicity, loan polarity, and predicate facts." | Borrows and aliases | Invocation-scoped authorization must be tracked separately from multiplicity, loan polarity, and predicates | not stated | 2
LANGUAGE_OWNERSHIP158 | "Displaced custody and invariant-window obligations still apply." | Borrows and aliases | Displaced custody and invariant-window obligations must remain enforced during writes | at the operation | 2
LANGUAGE_OWNERSHIP159 | "Loans retain their own compatibility rules; they are not all exact-once linear values." | Borrows and aliases | Loans must be checked under loan compatibility rules, not as uniform exact-once linear values | not stated | 2
LANGUAGE_OWNERSHIP160 | "An owned move out of a place reached through `&mut` may temporarily leave that place unestablished." | Borrowed-storage invariant windows | Partial move out via `&mut` may open window by leaving place unestablished | at the operation | 2
LANGUAGE_OWNERSHIP161 | "Diagnostics identify the opening move and the path or observation that requires restoration." | Borrowed-storage invariant windows | Diagnostics must identify opening move and path/observation requiring restoration | not stated | 2
LANGUAGE_OWNERSHIP162 | "This adds no placeholder value, runtime liveness flag, or special exchange-only syntax." | Borrowed-storage invariant windows | Invariant windows must not rely on placeholder values, liveness flags, or special exchange-only syntax | not stated | 2
LANGUAGE_OWNERSHIP163 | "The data declaration's owning package authorizes its nominal cleanup through [`dropped by`](domains.md#domain-selected-automatic-cleanup) on the default or a named domain." | Consumers and cleanup | Nominal cleanup must be authorized via `dropped by` on default or named domain in owning package | not stated | 2
LANGUAGE_OWNERSHIP164 | "Every owned death edge derives an internal contextual cleanup row: Type-side disposition eligibility, proposition prerequisites, operational effects/reach/work, and derived guarantees remain separate." | Consumers and cleanup | Each owned death edge must derive separate contextual cleanup row facets without conflating eligibility, prerequisites, effects, guarantees | not stated | 2
LANGUAGE_OWNERSHIP165 | "Diagnostics identify the authored hook clause and edge, not an internal synthesized predicate." | Consumers and cleanup | Cleanup diagnostics must cite authored hook clause and edge, not internal synthesized predicates | not stated | 2
LANGUAGE_OWNERSHIP166 | "An unrelated ordinary machine named `drop` has no reserved meaning." | Consumers and cleanup | Ordinary machines named `drop` must not receive reserved cleanup semantics | not stated | 2
LANGUAGE_OWNERSHIP167 | "Cleanup promises only its owner's disposition, not durable output or protocol completion." | Consumers and cleanup | Automatic cleanup must be checked as owner disposition only, not protocol/output completion | not stated | 2
LANGUAGE_OWNERSHIP168 | "Strict use of returned results alone does not enforce that lifecycle." | Consumers and cleanup | Compiler must not enforce task/resource lifecycle from strict use of returned results alone | not stated | 2
LANGUAGE_OWNERSHIP169 | "Deployment policy may forbid abandonment independently of memory safety." | Consumers and cleanup | Abandonment checking must allow deployment policy to forbid abandonment beyond memory-safety rules | not stated | 9
LANGUAGE_OWNERSHIP170 | "Whole-claim conservation does not require dependent types or imply divisibility." | Content and carry | Whole-claim conservation checks must not require dependent types or assume divisibility | not stated | 8
LANGUAGE_OWNERSHIP171 | "Retired content means leaving checked custody, not necessarily destruction or reclamation." | Content and carry | Retired content must be modeled as leaving checked custody without requiring destruction/reclamation | not stated | 8

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Move at most once; discard through eligible cleanup." | A use after move is rejected unless an eligible automatic cleanup edge disposes the value. | "At most once" applies only to explicit moves; implicit paths (e.g. cleanup) are separate and "eligible" is defined only elsewhere. |
"At an actual reconvergence, predecessor paths must agree on the live ownership frontier." | Only merge points that are actual reconvergence points require frontier agreement. | "Actual" excludes some CFG merges the compiler might otherwise treat as reconvergence; which merges qualify is not stated here. |
"Independently terminating outcome cases need not choose the same disposition: success may consume an input while failure returns it, provided each case satisfies its exact published contract." | Different outcome arms may diverge in ownership if each arm's contract is satisfied locally. | Divergence is allowed only when arms are "independently terminating" in a sense defined outside this chapter. |
"Every normal path accounts for each live linear obligation by transfer or an authorized terminal disposition." | Abnormal/crash paths are excluded from this accounting requirement by "normal." | "Normal" might mean all paths except those explicitly classified abnormal in the same chapter (e.g. crash), or all compilable paths. |
"Eligible automatic cleanup" (referenced under Affine multiplicity) | Eligibility is exactly the nominal/affine cleanup rules in Consumers and cleanup. | Eligibility includes additional rules only named in linked terminal-psi and domains chapters. |
"Calls may consume the detached value or use disjoint storage only when their complete access and outcome contracts cannot expose the incomplete owner." | The checker must prove contracts cannot expose the incomplete owner before allowing such calls. | Contracts are taken at face value from authored signatures only; callee body analysis scope is not stated. |
"Suspension with the window open rejects" | Any suspension while a borrowed-storage window is open must be rejected if resume/cancel contracts are not satisfied. | Rejection applies only when the suspended operation itself would violate carry/cleanup, not all suspensions in the region. |
"Automatic abandonment requires a contract declaring implicit disposal harmless." | A specific authored contract form must exist before implicit disposal on scope exit. | Harmlessness may be inferred from domain defaults not named in this section. |
"only the active sum payload is present" (cleanup order) | During whole-value hook, inactive case payloads are not subject to structural field cleanup. | "Present" refers to runtime representation only; compile-time obligations on inactive cases may still exist. |
"Each normal edge retains its own exact transfers and cleanup" | Transfer/cleanup obligations are edge-local in the CFG ownership analysis. | "Normal edge" excludes suspend/block edges whose semantics are only in linked carry/calls chapters. |

## Terms used without definition in this chapter

- multiplicity
- established value / established semantic value
- representation (in "independently of representation")
- qualification / qualified subject / qualified field / qualified machine name
- dependent facts
- carry (as a parallel obligation family; detailed rules deferred)
- type property
- trait / qualifier (contrasted with property)
- structural records / nominal container
- active sum payload / inactive case
- linear debt
- cleanup responsibility / unique cleanup responsibility
- shared reference / borrowed-view fields
- shared loan carriers / referents / backing / provenance
- cross-activation sharing permission
- machine-valued fields
- `some` packages / package witness association
- bracket properties / domain qualifications
- `sized` / structural carry (as derived judgments)
- opaque boundary properties / opaque claims / structural properties
- negative property syntax
- core property set / extensible attribute namespace
- initialized storage bits
- live ownership obligation
- validity conditions
- Type carrier / consumable permission
- conservation (on erased Type witnesses)
- permission context
- weakenable proposition catalog
- live ownership frontier / authorized terminal disposition
- published contract (outcome cases)
- process exit / bound domain / abandonment
- disposition receipt
- cross-domain survivor guarantees / domain-ending authority
- partial moves / unselected siblings / transparent child paths
- nominal root obligation / nominal cleanup hook / whole-value entitlement
- residual frontier
- runtime-indexed extraction
- source and result claims / provenance
- ambiguous opaque multi-resource output maps
- loan modes / loan contract / loan polarity
- reader and writer lists
- invocation-scoped grant
- loan carrier
- caller-side snapshot
- attenuation
- displaced custody
- reborrowing / parent lineage / child/cohort closure
- incomplete call / parked intervals
- transition arm
- ownership invariant window
- restoration obligation
- parent-coupling obligations
- invariant-window fences
- incomplete owner
- terminal consumer (negated feature)
- consuming operations / disposition
- automatic cleanup / terminal disposition / cleanup hook / cleanup edge
- `dropped by` / machine application / coalesce
- `no-data-code-attachment`
- contextual cleanup row / disposition eligibility
- `Disposable` bound
- `request_cancel`
- destination-first schedule
- independent roots
- established prefix
- runtime-count owned runs / erased ordered-cell custody summary
- consuming traversal
- dense occupied range / permanent holes / optional-element representation
- statically known live residual set
- whole-claim conservation
- `Content<Owner, A>` projection / n-ary theorem
- retired content
- counted pool
- permission event / permission ledger / realization / no-code reason

CHAPTER COMPLETE
