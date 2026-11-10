# Standalone Omega compiler request

Source: `wiki/spec/build/compiler_request.md` lines 1–268 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
COMPILER_REQUEST1 | "Refusals publish frames only once producers record the assigned codes." | Standalone Omega compiler request | A `Reject` outcome must not publish refusal frames until the producer has recorded the assigned refusal codes | not stated | 9
COMPILER_REQUEST2 | "Neither implementation may claim a complete interoperable V1 boundary before it implements the semantic phases over the decoded fields" | Standalone Omega compiler request | Implementation must not treat the V1 wire boundary as complete until semantic phases over decoded request fields are implemented | not stated | 9
COMPILER_REQUEST3 | "Bootstrap Alpha tape is an ordinary explicit product, never inferred from a filename, host, or first discovered entry." | Logical inputs | The `alpha_bootstrap_tape` product must be supplied explicitly in the invocation; it must not be inferred from filename, host, or first discovered entry | not stated | 9
COMPILER_REQUEST4 | "Package instances, locks, and source resolution do not certify compiler artifacts." | Logical inputs | Package instances, locks, and source-resolution metadata must not be treated as certifying compiler artifacts | not stated | 9
COMPILER_REQUEST5 | "V1 accepts resolved source, not preaccepted compiled dependency instances; adding the latter requires an explicit request-format change, not a zero or omitted row interpreted as acceptance." | Logical inputs | Requests must carry resolved source; a zero count, omitted row, or absent field must not be read as accepting precompiled dependency instances | not stated | 9
COMPILER_REQUEST6 | "the compiler validates the supplied identity inputs, not a catalog of past compilations." | Logical inputs | Validation must use only identity inputs supplied in the request, not a historical compilation catalog | not stated | 9
COMPILER_REQUEST7 | "Missing build source must remain distinguishable from a malformed authored build declaration." | Logical inputs | Outcomes or diagnostics must keep missing build source distinct from malformed authored build declarations | not stated | 9
COMPILER_REQUEST8 | "Default configuration cannot add ambient dependencies, authority or executable-entry discovery." | Logical inputs | Default configuration applied when no build file is selected must not introduce ambient dependencies, authority, or executable-entry discovery | not stated | 9
COMPILER_REQUEST9 | "The physical OCREQ v1 package rows below encode only named name/lineage owners; they do not encode the anonymous-owner record and default-build provenance." | Logical inputs | Standard OCREQ v1 package rows must not be decoded as anonymous-owner records or default-build provenance without an explicit extension | not stated | 9
COMPILER_REQUEST10 | "Supporting that logical input requires an explicit request extension shared by D and C, not fake names, host-path lineages, or reinterpreting existing tags." | Logical inputs | Anonymous-owner and default-build provenance must not be admitted via fake names, host-path lineages, or re-tagging existing wire fields | not stated | 9
COMPILER_REQUEST11 | "Ordinary artifact production does not require ecosystem receiving-policy input or claim permission to execute the result." | Logical inputs | Ordinary artifact production must not be blocked for lack of receiving-policy input or execute permission on the result | not stated | 9
COMPILER_REQUEST12 | "Component verification receives its exact consumer-accepted assumptions through those canonical external admissions" | Logical inputs | Component verification must use exactly the consumer-accepted assumptions carried in canonical external admissions | not stated | 9
COMPILER_REQUEST13 | "can only narrow the selected product's accepted ceiling; it cannot manufacture admissions or replace them with a source-authored digest list." | Logical inputs | Build attenuation must only narrow the accepted ceiling; it must not create admissions or substitute a source-authored digest list for them | not stated | 9
COMPILER_REQUEST14 | "Preserve subject, profile, occurrence, purpose, and target identity." | Logical inputs | Subject, profile, occurrence, purpose, and target identity must remain unchanged under attenuation | not stated | 9
COMPILER_REQUEST15 | "Product acceptance does not authorize executing a component during Build." | Logical inputs | Product acceptance must not be treated as permission to execute a component during Build | not stated | 9
COMPILER_REQUEST16 | "The retired `Build::accept_component_assumption` call adds no separate request input or approval channel." | Logical inputs | No request field or approval path may stand in for the retired `Build::accept_component_assumption` call | not stated | 9
COMPILER_REQUEST17 | "Discovery, retrieval, ambient traversal, and ambient host defaults are outside the standalone compiler." | Logical inputs | The standalone compiler must not rely on discovery, retrieval, ambient traversal, or ambient host defaults as request inputs | not stated | 9
COMPILER_REQUEST18 | "Compiler-injected build vocabulary belongs to compiler identity, not host-supplied source." | Logical inputs | Compiler-injected build vocabulary must be attributed to compiler identity, not treated as host-supplied source | not stated | 9
COMPILER_REQUEST19 | "the request cannot supply unvalidated conclusions in their place." | Logical inputs | Providers, roots, generated source, subsystem, and optimization choices must not be taken from unvalidated request fields in place of build-execution or default-configuration derivation | not stated | 9
COMPILER_REQUEST20 | "Completed dependencies contribute durable bundles and evidence, not a graph-wide collection of live half-compiled checkpoints." | Logical inputs | Dependency completion must surface durable bundles and evidence only, not a graph-wide set of live half-compiled checkpoints | not stated | 9
COMPILER_REQUEST21 | "Eight identity bytes: `4F 43 52 45 51 01 00 00`." | OCREQ v1 framing | Offset 0 must be exactly the eight listed identity bytes | not stated | 9
COMPILER_REQUEST22 | "Little-endian `u32` subject byte length." | OCREQ v1 framing | Bytes 8–11 must encode subject section length as little-endian `u32` matching the actual subject section | not stated | 9
COMPILER_REQUEST23 | "Little-endian `u32` invocation byte length." | OCREQ v1 framing | Bytes 12–15 must encode invocation section length as little-endian `u32` matching the actual invocation section | not stated | 9
COMPILER_REQUEST24 | "Exact subject section, then exact invocation section, then exact end." | OCREQ v1 framing | After offset 16 the bytes must be subject then invocation with no trailing bytes | not stated | 9
COMPILER_REQUEST25 | "Sections have no independent nested versions." | OCREQ v1 framing | Subject and invocation sections must not carry separate nested version fields | not stated | 9
COMPILER_REQUEST26 | "graph indices are zero-based" | OCREQ v1 framing | Package and edge indices in graph fields must be zero-based and in range | not stated | 9
COMPILER_REQUEST27 | "variants have assigned numeric tags." | OCREQ v1 framing | Every variant field must use an assigned numeric tag; unassigned tags reject in phase 6 | not stated | 9
COMPILER_REQUEST28 | "Reserved slots are zero, including identity bytes 6–7." | OCREQ v1 framing | Reserved bytes including identity bytes 6–7 must be zero | not stated | 9
COMPILER_REQUEST29 | "Counts, lengths, indices, and coordinate-producing extents are at most `INT32_MAX`." | OCREQ v1 framing | Every count, length, index, and coordinate-producing extent must be ≤ `INT32_MAX` | not stated | 5
COMPILER_REQUEST30 | "Language names use their specified UTF-8 grammar" | OCREQ v1 framing | Language names in the request must satisfy the specified UTF-8 grammar | not stated | 6
COMPILER_REQUEST31 | "snapshot paths, file contents, and link targets are raw bytes." | OCREQ v1 framing | Snapshot paths, file contents, and link targets must be decoded as raw bytes without implicit text normalization | not stated | 6
COMPILER_REQUEST32 | "Enum order, serde, pointers, report fingerprints, and private re-encodings cannot define wire bytes." | OCREQ v1 framing | Wire bytes must not be defined by enum order, serde layout, pointers, report fingerprints, or private re-encodings | not stated | 9
COMPILER_REQUEST33 | "package rows are strictly ordered by recomputed `PackageKeyIdentity`." | Canonical subject | Package table rows must be in strict ascending order by recomputed `PackageKeyIdentity` | not stated | 9
COMPILER_REQUEST34 | "Recompute key identities from structural fields; a supplied digest does not establish them." | Canonical subject | `PackageKeyIdentity` must be recomputed from structural name and lineage fields; a digest field alone must not establish identity | not stated | 9
COMPILER_REQUEST35 | "Graph references index the validated table." | Canonical subject | Every package index in edges and root fields must refer to a validated package table row | not stated | 9
COMPILER_REQUEST36 | "Dependency rows are ordered by requester index and local alias." | Canonical subject | Edge rows must be ordered by requester index then local alias (and scope per later rule) | not stated | 9
COMPILER_REQUEST37 | "Duplicate, dangling, foreign, unreachable, cyclic, role-inconsistent, identity-mismatched, and noncanonically ordered rows reject." | Canonical subject | The listed graph and ordering defects must yield `Reject` | not stated | 9
COMPILER_REQUEST38 | "one requester cannot rename an alias inside another package." | Canonical subject | Alias spellings must remain scoped to the requester; cross-package alias renaming must be rejected | not stated | 9
COMPILER_REQUEST39 | "It does not replace that record with a request-specific origin string or define a second package key." | Shared package identity | The structured source-lineage record must not be replaced by a request-specific origin string or a second package key | not stated | 9
COMPILER_REQUEST40 | "the standalone compiler decodes the record and recomputes the same `PackageKeyIdentity` without contacting the origin." | Shared package identity | After decode, `PackageKeyIdentity` must match the package-identity rules without origin contact | not stated | 9
COMPILER_REQUEST41 | "For named packages, only the declared name and canonical lineage determine the key." | Shared package identity | Package key for named packages must depend only on declared name and canonical lineage | not stated | 9
COMPILER_REQUEST42 | "Identical canonical name/lineage inputs must produce identical keys in package management, D and C." | Shared package identity | Equal canonical name and lineage inputs must yield equal keys | not stated | 9
COMPILER_REQUEST43 | "The superseded two-tag `external_local`/`git` encoding with opaque lineage text must not be reinterpreted as this structure." | Shared package identity | Legacy two-tag opaque lineage encoding must not be accepted as the structured source-lineage record | not stated | 9
COMPILER_REQUEST44 | "private implementation fields do not override those rules." | Shared package identity | Private implementation fields must not change package identity outcomes | not stated | 9
COMPILER_REQUEST45 | "Each snapshot is a complete deterministic virtual filesystem." | Source snapshots | Each package snapshot must be a complete deterministic virtual filesystem | not stated | 9
COMPILER_REQUEST46 | "Its closed rows are directories, regular files with executable state and exact bytes, and symbolic links with exact target spelling." | Source snapshots | Snapshot rows must be only directory, regular_file, or symbolic_link with the stated payloads | not stated | 9
COMPILER_REQUEST47 | "Validate root directory, parent closure, unique raw paths, metadata-policy limits, and exact aggregate content." | Source snapshots | Snapshot validation must check root, parent closure, unique raw paths, metadata limits, and aggregate content | not stated | 9
COMPILER_REQUEST48 | "Rows follow raw-path order." | Source snapshots | Snapshot rows must appear in raw-path order | not stated | 9
COMPILER_REQUEST49 | "Do not serialize those derived facts a second time." | Source snapshots | Derived metadata such as child enumeration and lengths must not appear as redundant serialized fields | not stated | 9
COMPILER_REQUEST50 | "the compiler never dereferences it." | Source snapshots | External-local canonical paths in lineage must not be dereferenced at compile time | not stated | 9
COMPILER_REQUEST51 | "Ambient environment, clocks, randomness, network state, or a prior build-operation trace cannot replace snapshot facts." | Source snapshots | Snapshot content must not be substituted from ambient host or prior-build state | not stated | 9
COMPILER_REQUEST52 | "Compiler publication captures immutable bytes; a mutable overlay or a library changeset is not itself a successful product receipt." | Source snapshots | Published snapshot bytes must be immutable; mutable overlay or library changeset alone must not count as a successful product receipt | not stated | 9
COMPILER_REQUEST53 | "old snapshot/facet records must not be reinterpreted as the new meaning." | Source snapshots | Retained snapshot or facet schemas must not reinterpret old records under new semantics | not stated | 9
COMPILER_REQUEST54 | "there are no skipped or self-delimiting fields." | Subject field/tag table | Subject section must be one contiguous positional sequence without gaps or self-delimiting fields | not stated | 9
COMPILER_REQUEST55 | "Every `u32` is little-endian and at most `INT32_MAX`." | Subject field/tag table | Subject `u32` fields must be little-endian and ≤ `INT32_MAX` | not stated | 5
COMPILER_REQUEST56 | "Every `bytes` field is a `u32` length followed by that many bytes." | Subject field/tag table | Every `bytes` field must be length-prefixed with a `u32` count matching the following bytes | not stated | 9
COMPILER_REQUEST57 | "Every table is a `u32` count followed by that many rows in the order its rule assigns." | Subject field/tag table | Tables must use explicit counts with rows in rule-assigned order | not stated | 9
COMPILER_REQUEST58 | "The explicit root role must agree with the root row's declared `role` field; a workspace is never a root role." | Subject field/tag table | `root_role` must match the root package row's `role`; workspace must not appear as root role | not stated | 9
COMPILER_REQUEST59 | "package rows are ordered by unsigned lexicographic comparison of its 32 digest bytes." | Subject field/tag table | Package table ordering must follow unsigned lexicographic order of the 32-byte `PackageKeyIdentity` digest | not stated | 9
COMPILER_REQUEST60 | "the compiler must decode and validate the canonical identity structure rather than treating it as unspecified text." | Subject field/tag table | Lineage must be decoded and validated as the canonical identity structure, not opaque text | not stated | 9
COMPILER_REQUEST61 | "`u32` tag: 1 `directory`, 2 `regular_file`, 3 `symbolic_link`" | Subject field/tag table | Snapshot row `kind` must be one of the assigned tags 1–3 | not stated | 9
COMPILER_REQUEST62 | "`regular_file`: `u32` `executable` tag (0 or 1) then `bytes` content" | Subject field/tag table | Regular file rows must carry executable tag 0 or 1 then exact content bytes | not stated | 9
COMPILER_REQUEST63 | "Edge rows are ordered by requester index, then local alias, then scope." | Subject field/tag table | Edge table rows must be sorted by requester, then alias, then scope | not stated | 9
COMPILER_REQUEST64 | "`u32` tag: 1 `product`, 2 `build`" | Subject field/tag table | Edge `scope` must be tag 1 or 2 only | not stated | 9
COMPILER_REQUEST65 | "`u32` tag: 1 `check`, 2 `terminal_artifact`, 3 `native_artifact`, 4 `alpha_bootstrap_tape`" | Invocation field/tag table | Invocation `product` must be one of tags 1–4 | not stated | 9
COMPILER_REQUEST66 | "`subject_commitment` | exactly 32 bytes, the binding below" | Invocation field/tag table | Subject commitment field must be exactly 32 bytes | not stated | 9
COMPILER_REQUEST67 | "a canonical V1 request's admission table is empty until this contract gains entries; a present row's unassigned kind rejects at phase 6 like every other unassigned tag." | Invocation field/tag table | Until admission kinds are assigned, admission count must be zero; any row with unassigned kind must reject in phase 6 | not stated | 9
COMPILER_REQUEST68 | "Field/tag shape checking is phases 2 and 6: extents, counts, `u32` bounds, variant/tag membership, and the exact section end." | Invocation field/tag table | Phases 2 and 6 must check framing extents, counts, bounds, tags, and exact section end | not stated | 9
COMPILER_REQUEST69 | "Content rules — name and alias grammar, package-key recomputation and strict ordering, graph checks, snapshot row semantics, admission checks, and the commitment binding — remain in their assigned phases." | Invocation field/tag table | Name, alias, package-key, graph, snapshot, admission, and commitment checks must run only in their assigned phases, not in phases 2 and 6 alone | not stated | 9
COMPILER_REQUEST70 | "SHA-256(\"omega.ocreq.subject.sha256.v1\\0\" || exact subject-section bytes)" | Invocation field/tag table | The 32-byte commitment must equal SHA-256 of the stated prefix concatenated with exact subject-section bytes | not stated | 9
COMPILER_REQUEST71 | "Validate outer framing and subject canonicality before checking this binding." | Invocation field/tag table | Framing and subject canonicality checks must complete before subject-commitment verification | not stated | 9
COMPILER_REQUEST72 | "It remains mandatory when the two sections share a frame, and does not replace local package-key checks." | Invocation field/tag table | Subject commitment is required whenever subject and invocation share a frame; package-key checks must still run | not stated | 9
COMPILER_REQUEST73 | "Hashing a reconstructed object is not equivalent." | Invocation field/tag table | Commitment verification must hash exact subject-section bytes, not a re-encoded reconstruction | not stated | 9
COMPILER_REQUEST74 | "Every requested artifact is produced and validated." | Outcomes | `Complete` requires every requested artifact to be produced and validated | not stated | 9
COMPILER_REQUEST75 | "Invalid request/evidence, violated checking/admission rules, or an established violation of a required guarantee. Name which subject failed." | Outcomes | `Reject` applies to the listed failures; diagnostics must name which subject failed | not stated | 9
COMPILER_REQUEST76 | "A named resource/search limit or unsupported valid case prevents completion; no claim that the unproved conclusion is false." | Outcomes | `Incomplete` must not assert that an unproved conclusion is false | not stated | 9
COMPILER_REQUEST77 | "A compiler/checker malfunction or violated implementation invariant; no artifact authority." | Outcomes | `InternalFailure` must carry no artifact authority | not stated | 9
COMPILER_REQUEST78 | "Failure to find a proof is not proof of falsehood." | Outcomes | Proof search failure must not be reported as disproving the goal | not stated | 8
COMPILER_REQUEST79 | "Rejecting a submitted proof does not refute its conclusion." | Outcomes | Invalid or rejected proofs must not be presented as refuting the theorem | not stated | 8
COMPILER_REQUEST80 | "Failure to admit assumptions does not refute the theorem either." | Outcomes | Unadmitted assumptions must not be treated as refuting the theorem | not stated | 8
COMPILER_REQUEST81 | "Contradictory branch premises may legitimately discharge an unreachable path; inconsistent admitted axioms may derive falsehood. Neither is automatically an implementation failure." | Outcomes | Contradictory premises or inconsistent admitted axioms must not automatically be classified as `InternalFailure` | not stated | 8
COMPILER_REQUEST82 | "Diagnostics distinguish failed source checking, invalid proof, unavailable evidence and receiving-policy refusal." | Outcomes | Diagnostics must separate failed source checking, invalid proof, unavailable evidence, and receiving-policy refusal | not stated | 9
COMPILER_REQUEST83 | "These semantic outcomes do not invent new wire tags or host exit codes." | Outcomes | Outcome classification must not add wire tags or host exit codes beyond this contract's representations | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"The selected root is a package index plus its role because roots and entry selection are derived by admitted build execution, not supplied by the request." | The request still supplies `root_package` and `root_role`, but executable entry selection within that root comes only from build execution. | The request must not supply root selection at all; only build execution chooses the root index and role. |
"This wire limitation does not revoke anonymous compilation or require an authored build file in the language." | Anonymous compilation remains valid logically; only the physical OCREQ v1 row shape is limited. | Anonymous compilation may proceed without any build file anywhere in the toolchain, including acquisition. |
"Source canonicalization is acquisition-owned, but the compiler must decode and validate the canonical identity structure rather than treating it as unspecified text." | The compiler validates structure and identity but does not re-run full acquisition canonicalization. | The compiler must perform the same canonicalization acquisition would perform, not merely structural validation. |
"a canonical V1 request's admission table is empty until this contract gains entries" | Any V1 request with a nonempty admissions table is invalid today. | Empty table is canonical default; nonempty tables might be permitted if kinds were assigned elsewhere in the document outside this range. |
"neither is automatically an implementation failure" | Such cases may be `Complete`, `Reject`, or `Incomplete` depending on other rules. | They are never `InternalFailure` but the chapter does not state which non-internal outcome always applies. |
"Retained schemas must preserve these contexts; old snapshot/facet records must not be reinterpreted as the new meaning." | Applies only when reading stored artifacts written under an older schema version. | Applies also when decoding current OCREQ snapshot rows if their encoding resembles an older facet layout. |
"Ordinary artifact production does not require ecosystem receiving-policy input or claim permission to execute the result." | Compilation may complete without those inputs; receiving may still refuse artifacts separately. | Lack of receiving-policy or execute permission cannot block reaching `Complete` for ordinary products. |
"The explicit root role must agree with the root row's declared `role` field; a workspace is never a root role." | `root_role` tag values are only `package` or `application`; workspace is forbidden as a third tag value. | A package row whose semantic role is workspace may exist, but it cannot be selected as the compilation root. |

## Terms used without definition in this chapter

- Epsilon-written bootstrap compiler D
- Omega-written compiler C
- logical request
- observable wire contract
- outer framing
- semantic contents
- commitment preimage
- validation order
- publication rules
- outcome frame layout
- coordinate spaces
- diagnostic phases
- scalar-resource table
- package-origin encoding
- assigned `Reject` inventory
- syntax distinction granularity
- checking codes
- named `Incomplete` coverage provisions
- producers (in refusal-frame sense)
- semantic phases
- OMEGA-D / OMEGA-C
- `OmegaCompilationSubject`
- `OmegaInvocation`
- resolved source graph
- explicit selected root and role
- requester-local aliases/edges
- declaration owners
- immutable resolutions
- build-visible snapshots
- content-identified anonymous roots
- explicit requested product
- canonical target profile
- canonical external admissions
- exact subject commitment
- Bootstrap Alpha tape
- product tag
- Rust reference compiler
- package instances / locks
- preaccepted compiled dependency instances
- request-format change
- default configuration
- anonymous-owner record
- default-build provenance
- explicit request extension
- external admissions
- ecosystem receiving-policy input
- claim permission
- explicit receiver admission
- component verification
- consumer-accepted assumptions
- package acceptance/lock route
- root grants
- Build attenuation
- accepted ceiling
- source-authored digest list
- subject / profile / occurrence / purpose / target identity
- `Build::accept_component_assumption`
- discovery / retrieval / ambient traversal
- ambient host defaults
- compiler-injected build vocabulary
- compiler identity
- admitted build execution
- durable bundles
- evidence
- half-compiled checkpoints
- OCREQ v1
- little-endian `u32`
- flat version
- legacy readers / dual encodings / compatibility adapters
- v1 label
- length-framed
- graph indices
- coordinate-producing extents
- `INT32_MAX`
- specified UTF-8 grammar (for language names)
- report fingerprints
- `PackageKeyIdentity`
- structural name/lineage
- revision / tree / content resolution
- dependency rows
- dangling / foreign / unreachable rows
- role-inconsistent rows
- identity-mismatched rows
- noncanonically ordered rows
- alias spelling
- package acquisition
- structured source-lineage record
- request-specific origin string
- package management
- canonical identity preimage
- `external_local` / `git` (superseded encoding)
- package identity rules
- complete deterministic virtual filesystem
- metadata-policy limits
- exact aggregate content
- direct-child enumeration
- canonical metadata
- external-local canonical path
- prior build-operation trace
- delegated filesystem observations
- writable/CoW backing
- staged Build execution
- execution context
- mutable overlay
- library changeset
- product receipt
- snapshot/facet records
- positional field sequence
- self-delimiting fields
- `root_package` / `root_role`
- workspace (as disallowed root role)
- member projection path
- unsigned lexicographic comparison
- snapshot row `kind` tags
- executable tag
- edge `scope` tags
- `target_profile`
- admission row
- admission `kind` tag
- phase 2 / phase 6
- exact section end
- name and alias grammar
- graph checks
- snapshot row semantics
- admission checks
- `check` / `terminal_artifact` / `native_artifact` / `alpha_bootstrap_tape`
- `Complete` / `Reject` / `Incomplete` / `InternalFailure`
- required guarantee
- named resource/search limit
- unsupported valid case
- unproved conclusion
- compiler/checker malfunction
- implementation invariant
- artifact authority
- contradictory branch premises
- unreachable path
- inconsistent admitted axioms
- receiving-policy refusal
- halt tags
- failure frames

CHAPTER COMPLETE
