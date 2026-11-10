# Build inputs, observations, and outputs

Source: `wiki/spec/build/observations.md` lines 1–68 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
BUILD_OBSERVATIONS1 | "Observations describe one execution, not a second admission policy." | Build inputs, observations, and outputs | Retained observations must not be applied as a separate admission policy beyond one execution. | not stated | 9
BUILD_OBSERVATIONS2 | "A build uses the dependency resolutions and accepted build requirements recorded in its lock." | One admitted execution | The build must use dependency resolutions and accepted build requirements from its lock. | not stated | 9
BUILD_OBSERVATIONS3 | "A mismatch rejects compilation;" | One admitted execution | Lock resolutions or accepted build requirements that do not match the build must reject compilation. | not stated | 9
BUILD_OBSERVATIONS4 | "compilation cannot silently update the lock or invent acceptance." | One admitted execution | Compilation must not update the lock without an explicit recorded path and must not invent package acceptance. | not stated | 9
BUILD_OBSERVATIONS5 | "Install/update owns changes to recorded decisions." | One admitted execution | Changes to recorded lock decisions must occur only through install/update, not compilation. | not stated | 9
BUILD_OBSERVATIONS6 | "The root build selects the filesystem implementation and delegates scoped access." | One admitted execution | The root build must select the filesystem implementation and delegate scoped access accordingly. | not stated | 9
BUILD_OBSERVATIONS7 | "Every downstream delegation may preserve or reduce that access, never enlarge it." | One admitted execution | Each downstream delegation must preserve or reduce delegated access and must not enlarge it. | not stated | 9
BUILD_OBSERVATIONS8 | "Selecting a dependency or importing a filesystem API supplies no authority." | One admitted execution | Dependency selection and filesystem API import must not grant filesystem or host authority by themselves. | not stated | 9
BUILD_OBSERVATIONS9 | "If the root supplies a virtual filesystem, descendants use that filesystem;" | One admitted execution | When the root supplies a virtual filesystem, descendants must use that filesystem. | not stated | 9
BUILD_OBSERVATIONS10 | "unsupported operations reject rather than falling back to a real host provider." | One admitted execution | Unsupported filesystem operations under a virtual root must reject and must not fall back to a real host provider. | at the operation | 9
BUILD_OBSERVATIONS11 | "The executor must possess the resources the root requests:" | One admitted execution | The executor must actually possess resources the root build requests. | not stated | 9
BUILD_OBSERVATIONS12 | "a root declaration cannot manufacture host authority either." | One admitted execution | Root declarations must not create host authority the executor does not possess. | not stated | 9
BUILD_OBSERVATIONS13 | "Execute each admitted build activation once." | One admitted execution | Each admitted build activation must be executed exactly once. | not stated | 9
BUILD_OBSERVATIONS14 | "Check operations against its supplied capabilities," | One admitted execution | Build operations must be checked against the activation's supplied capabilities. | at the operation | 9
BUILD_OBSERVATIONS15 | "account for resources," | One admitted execution | Resource use during the activation must be accounted. | not stated | 9
BUILD_OBSERVATIONS16 | "and validate completed outputs directly." | One admitted execution | Completed outputs must be validated directly rather than indirectly via replay or comparison execution. | not stated | 9
BUILD_OBSERVATIONS17 | "There is no operation-record playback, second execution for comparison, replay codec, or replay-based release classification." | One admitted execution | The system must not use operation-record playback, second comparison execution, replay codecs, or replay-based release classification. | not stated | 9
BUILD_OBSERVATIONS18 | "Build acceptance does not promise reproducibility or authenticate external input contents." | One admitted execution | Build acceptance must not be treated as promising reproducibility or authenticating external input contents. | not stated | 9
BUILD_OBSERVATIONS19 | "Retain the selected activation, canonical input identity, attempted operations and their actual results where needed for audit/accounting, explicit source handoffs, completed output obligations, and bounded diagnostics." | Retained execution facts | Listed execution facts must be retained for the activation. | not stated | 9
BUILD_OBSERVATIONS20 | "Failed attempts remain failures;" | Retained execution facts | Failed attempts recorded in observations must remain classified as failures. | not stated | 9
BUILD_OBSERVATIONS21 | "an absent observation cannot manufacture success or authority." | Retained execution facts | Missing observation data must not be used to infer success or grant authority. | not stated | 9
BUILD_OBSERVATIONS22 | "These facts neither authorize another activation nor prove a shipped program's runtime behavior." | Retained execution facts | Retained execution facts must not authorize another activation or prove shipped runtime behavior. | not stated | 9
BUILD_OBSERVATIONS23 | "Captured inputs remain immutable for the activation." | Retained execution facts | Inputs captured for an activation must not change for that activation. | not stated | 9
BUILD_OBSERVATIONS24 | "Input identity binds their exact bytes and admitted metadata, not a host pathname that can later change." | Retained execution facts | Canonical input identity must bind exact bytes and admitted metadata, not a mutable host pathname. | not stated | 9
BUILD_OBSERVATIONS25 | "Retaining completed output bytes for publication or cache use does not rerun the build." | Retained execution facts | Using retained output bytes for publication or cache must not trigger another build execution. | not stated | 9
BUILD_OBSERVATIONS26 | "After evaluator/provider teardown, inspect the quiescent sponsored Output tree before deleting the session." | Complete output custody | After teardown, the quiescent sponsored Output tree must be inspected before the session is deleted. | not stated | 9
BUILD_OBSERVATIONS27 | "Retain its complete canonical tree: sorted portable UTF-8 relative paths, entry kinds, empty directories, ordinary/executable mode, file lengths/content commitments, and self-contained relative symlink targets." | Complete output custody | The complete canonical output tree with all listed attributes must be retained. | not stated | 9
BUILD_OBSERVATIONS28 | "An empty tree has an explicit commitment." | Complete output custody | An empty output tree must still carry an explicit commitment in the retained record. | not stated | 9
BUILD_OBSERVATIONS29 | "Exclude host roots, timestamps, ownership, ACLs, ambient permissions, and inode or hard-link topology from canonical tree identity." | Complete output custody | Canonical tree identity must exclude the listed host-specific and topology fields. | not stated | 9
BUILD_OBSERVATIONS30 | "Validate namespace kinds, extents, and sponsored usage;" | Complete output custody | Namespace kinds, extents, and sponsored usage must be validated. | not stated | 9
BUILD_OBSERVATIONS31 | "unknown objects, external links, inconsistent custody, and exceeded provisions reject." | Complete output custody | Unknown objects, external links, inconsistent custody, or exceeded provisions must reject. | not stated | 9
BUILD_OBSERVATIONS32 | "Bind topology-independent unique-content accounting alongside the tree commitment." | Complete output custody | Unique-content accounting independent of topology must be bound together with the tree commitment. | not stated | 9
BUILD_OBSERVATIONS33 | "Materialization consumes retained complete content into an existing empty destination and re-inspects exact paths, kinds, modes, targets, and bytes." | Complete output custody | Materialization must target an existing empty destination and re-inspect exact paths, kinds, modes, targets, and bytes. | not stated | 9
BUILD_OBSERVATIONS34 | "Generated source is only the explicitly selected handoff subset, never whatever happens to have a source filename." | Complete output custody | Generated source admission must include only explicitly selected handoffs, not all files with source-like names. | not stated | 9
BUILD_OBSERVATIONS35 | "Each handoff binds its completed attempt ordinal;" | Complete output custody | Each source handoff must bind to its completed attempt ordinal. | not stated | 9
BUILD_OBSERVATIONS36 | "ordinals are nondecreasing," | Complete output custody | Handoff attempt ordinals must be nondecreasing. | not stated | 9
BUILD_OBSERVATIONS37 | "a path occurs once," | Complete output custody | Each handoff path must appear at most once in the handoff set. | not stated | 9
BUILD_OBSERVATIONS38 | "and the corresponding file is closed first." | Complete output custody | The file for a handoff path must be closed before the handoff is recorded. | not stated | 9
BUILD_OBSERVATIONS39 | "Equal ordinals are legal and handoff order need not match output creation order." | Complete output custody | Handoffs may share ordinals and need not follow output creation order. | not stated | 9
BUILD_OBSERVATIONS40 | "Source extension, reserved-name, regular/non-executable file, and final frontend checks still apply." | Complete output custody | Listed source and frontend checks must still apply to handoff-generated source. | not stated | SHARED
BUILD_OBSERVATIONS41 | "Enforce the input/output isolation contract or require an explicit adequate executor isolation premise." | Complete output custody | Input/output isolation contract enforcement or an explicit adequate executor isolation premise must be present. | not stated | 9
BUILD_OBSERVATIONS42 | "Neither a lock nor a retained observation substitutes for that enforcement." | Complete output custody | Lock contents and retained observations must not replace isolation-contract enforcement or the required premise. | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"attempted operations and their actual results where needed for audit/accounting" | Retention of operation results is mandatory only when audit or accounting requires them; the executor decides need. | The build system must retain all attempted operations and results, using "where needed" only to scope which result fields are stored. |
"quiescent sponsored Output tree" | Inspection occurs once the tree has no active evaluator or provider writers and sponsored usage is idle. | "Quiescent" may allow inspection while some sponsored namespace objects remain in transitional states if writers are torn down. |
"topology-independent unique-content accounting" | Content commitments deduplicate identical bytes regardless of hard-link or inode topology in the captured tree. | A separate accounting ledger must bind content hashes or commitments in a form that can differ from the tree commitment's path structure; how the two relate is not stated in this sentence. |
"require an explicit adequate executor isolation premise" | The admission or observation record must name a documented premise class judged adequate for same-user disk races. | Any executor that lacks disk isolation must be rejected unless the root records a premise; "adequate" is evaluated outside this chapter. |
"Equal ordinals are legal and handoff order need not match output creation order." | Multiple handoffs may share one ordinal and may be ordered differently from filesystem creation order without rejection. | Equal ordinals mark a single logical handoff step; output files created in another order still must be grouped under ordinals that the executor assigns, and legality of equal ordinals does not relax other ordering checks named elsewhere. |
"final frontend checks still apply." | The same frontend checks applied to ordinary source apply to generated handoff source at admission. | Only a subset of frontend checks labeled "final" apply; which checks count as final is defined only outside this range. |
"Build acceptance does not promise reproducibility or authenticate external input contents." | Tools must not claim reproducibility or input authentication from acceptance records alone. | The compiler may still require reproducibility or authentication elsewhere; this sentence only limits what build acceptance promises, not what compilation may enforce. |
"unsupported operations reject rather than falling back to a real host provider." | Any operation not supported by the virtual filesystem implementation must fail the build operation. | Only operations explicitly unsupported must reject; implementations may treat some operations as supported via emulation without using the host provider. |

## Terms used without definition in this chapter

- observations / observation
- build admission
- generated-source handoff
- package acceptance
- `omega.lock`
- dependency resolutions
- accepted build requirements
- install/update
- root build
- filesystem implementation
- scoped access / delegation
- virtual filesystem
- host provider / host authority
- executor
- build activation / admitted build activation
- supplied capabilities
- operation-record playback
- replay codec
- replay-based release classification
- build acceptance
- selected activation
- canonical input identity
- attempted operations
- audit/accounting
- explicit source handoffs
- completed output obligations
- bounded diagnostics
- captured inputs
- admitted metadata
- scoped execution contract
- output-cache eligibility
- evaluator/provider teardown
- quiescent sponsored Output tree
- canonical tree
- portable UTF-8 relative paths
- entry kinds
- ordinary/executable mode
- content commitments
- self-contained relative symlink targets
- host roots
- ACLs
- ambient permissions
- inode or hard-link topology
- namespace kinds
- extents
- sponsored usage
- external links
- inconsistent custody
- exceeded provisions
- topology-independent unique-content accounting
- tree commitment
- materialization
- generated source
- completed attempt ordinal
- source extension checks
- reserved-name checks
- regular/non-executable file checks
- final frontend checks
- scoped grants
- deterministic resource accounts
- hostile same-user processes
- disk-backed storage
- input/output isolation contract
- adequate executor isolation premise

CHAPTER COMPLETE
