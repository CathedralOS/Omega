# Admitted executable installation

Source: `wiki/spec/build/executable_installation.md` lines 1–110 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| EXECUTABLE_INSTALLATION1 | "There is no general executable-memory capability, arbitrary bytes-to-code conversion, JIT, or self-modifying-code route." | Admitted executable installation | Programs must not be accepted on general executable memory, arbitrary bytes-to-code, JIT, or self-modifying-code routes | not stated | 9 |
| EXECUTABLE_INSTALLATION2 | "An immutable reusable `Artifact` is eligible only through the sealed `Artifact::AdmittedExecutable` qualification." | Admitted executable installation | Immutable reusable artifacts qualify for executable use only via sealed `Artifact::AdmittedExecutable` | not stated | 9 |
| EXECUTABLE_INSTALLATION3 | "Admission binds exact content, identity, relocation and required evidence, footprint, and placement plan." | Admitted executable installation | Admission must bind exact content, identity, relocation, required evidence, footprint, and placement plan | not stated | 9 |
| EXECUTABLE_INSTALLATION4 | "Packages cannot self-establish it;" | Admitted executable installation | Packages must not establish `AdmittedExecutable` qualification for themselves | not stated | 9 |
| EXECUTABLE_INSTALLATION5 | "mutation destroys it." | Admitted executable installation | Mutation must destroy the `AdmittedExecutable` qualification | not stated | 9 |
| EXECUTABLE_INSTALLATION6 | "Ordinary compilation grants no installation or execution authority" | Admitted executable installation | Ordinary compilation must grant neither installation nor execution authority | not stated | 9 |
| EXECUTABLE_INSTALLATION7 | "and requires no receiving policy." | Admitted executable installation | Ordinary compilation must not require a receiving policy | not stated | 9 |
| EXECUTABLE_INSTALLATION8 | "Immutable admitted artifact." | Placement lifecycle | Canonical decode and receiver-required contract/evidence admission must yield an immutable admitted artifact | not stated | 9 |
| EXECUTABLE_INSTALLATION9 | "Materialized declared sections/relocations." | Placement lifecycle | Linear `CodePlacement` (write, no execute) plus borrowed artifact must materialize declared sections and relocations | not stated | 9 |
| EXECUTABLE_INSTALLATION10 | "Readable, non-executable immutable placement." | Placement lifecycle | Freeze must yield a readable, non-executable immutable placement | not stated | 9 |
| EXECUTABLE_INSTALLATION11 | "Validated placement bound to those bytes and authority." | Placement lifecycle | Validating exact final bytes and footprint must bind placement to those bytes and authority | not stated | 9 |
| EXECUTABLE_INSTALLATION12 | "Linear installed code, readable and executable." | Placement lifecycle | Installer provider and visibility completion must yield linear installed code that is readable and executable | not stated | 9 |
| EXECUTABLE_INSTALLATION13 | "The artifact is borrowed, not consumed." | Placement lifecycle | Placement lifecycle must borrow the artifact and must not consume it | not stated | 2 |
| EXECUTABLE_INSTALLATION14 | "Placement is linear: validation cannot be transplanted to other bytes or spent twice." | Placement lifecycle | Placement validation must be linear; must reject transplanting validation to other bytes or spending it twice | not stated | 9 |
| EXECUTABLE_INSTALLATION15 | "A certificate may remain reportable/reusable while installation consumes the validated placement." | Placement lifecycle | Installation consuming validated placement must not require consuming a still-reportable/reusable certificate | not stated | 9 |
| EXECUTABLE_INSTALLATION16 | "Failed linear transitions return every input." | Placement lifecycle | Failed linear transitions must return every input | not stated | 9 |
| EXECUTABLE_INSTALLATION17 | "Content, proof, and final-byte commitments are distinct domain-framed strong digests;" | Placement lifecycle | Content, proof, and final-byte commitments must remain distinct domain-framed strong digests | not stated | 8 |
| EXECUTABLE_INSTALLATION18 | "compact compatibility values are report keys, and exact bytes/placement evidence remain available for acceptance comparison." | Placement lifecycle | Compact compatibility values must act as report keys only; exact bytes and placement evidence must stay available for acceptance comparison | not stated | 9 |
| EXECUTABLE_INSTALLATION19 | "Materialization resolves only sealed entry/data identities into a private copy." | Placement lifecycle | Materialization must resolve only sealed entry/data identities into a private copy | not stated | 9 |
| EXECUTABLE_INSTALLATION20 | "Installation separately validates W^X, target cache/order and instruction-fetch visibility, and audience through one contracted provider operation." | Placement lifecycle | Installation must validate W^X, target cache/order, instruction-fetch visibility, and audience in one contracted provider operation | not stated | 9 |
| EXECUTABLE_INSTALLATION21 | "Scope binds the admitted artifact, placement, and audience." | Placement lifecycle | Scope must bind admitted artifact, placement, and audience together | not stated | 9 |
| EXECUTABLE_INSTALLATION22 | "`CodePlacement` composes physical/virtual extent authority and constraints, not a parallel authority family or replaceable dispatch binding." | Placement lifecycle | `CodePlacement` must compose physical/virtual extent authority and constraints only; must not be a parallel authority family or replaceable dispatch binding | not stated | 9 |
| EXECUTABLE_INSTALLATION23 | "Requirement binding is separate and later." | Placement lifecycle | Requirement binding must be separate from placement composition and must occur later in the lifecycle | not stated | 9 |
| EXECUTABLE_INSTALLATION24 | "Every route to execute permission follows this rule." | Placement lifecycle | Every path that grants execute permission must follow the placement lifecycle rule described here | not stated | 9 |
| EXECUTABLE_INSTALLATION25 | "Translation providers need admitted-artifact provenance;" | Placement lifecycle | Translation providers must require admitted-artifact provenance | not stated | 9 |
| EXECUTABLE_INSTALLATION26 | "checked assembly retains the same installation authority and reach obligations." | Placement lifecycle | Checked assembly must keep the same installation authority and reach obligations as other routes | not stated | 10 |
| EXECUTABLE_INSTALLATION27 | "Device firmware/GPU/NIC uploads belong to device providers, not host executable artifacts." | Placement lifecycle | Device firmware, GPU, and NIC uploads must use device providers, not host executable artifact installation | not stated | 9 |
| EXECUTABLE_INSTALLATION28 | "Secondary-processor startup uses an explicitly selected, consumer-owned boot-protocol provider." | Placement lifecycle | Secondary-processor startup must use an explicitly selected consumer-owned boot-protocol provider | not stated | 9 |
| EXECUTABLE_INSTALLATION29 | "Any required trampoline is compiler-produced admitted executable content, not runtime-generated host code." | Placement lifecycle | Required trampolines must be compiler-produced admitted executable content; runtime-generated host code must not substitute | not stated | 9 |
| EXECUTABLE_INSTALLATION30 | "Low-memory placement and vector geometry belong to that provider's exact startup contract, not every startup mechanism." | Placement lifecycle | Low-memory placement and vector geometry must follow the selected provider's exact startup contract only | not stated | 9 |
| EXECUTABLE_INSTALLATION31 | "The compiler checks entry, installation, and custody;" | Placement lifecycle | The compiler must check entry, installation, and custody | not stated | SHARED |
| EXECUTABLE_INSTALLATION32 | "it does not own processor discovery or the boot protocol." | Placement lifecycle | Processor discovery and boot protocol selection must remain outside compiler ownership | not stated | 9 |
| EXECUTABLE_INSTALLATION33 | "Local installation completion." | Visibility and retirement | Dormant/local target audience must be gated on local installation completion | not stated | 9 |
| EXECUTABLE_INSTALLATION34 | "Visibility completion before entry." | Visibility and retirement | Future remote fetcher audience must complete visibility before entry | before entry | 9 |
| EXECUTABLE_INSTALLATION35 | "Component replacement and quiescence." | Visibility and retirement | Possible current executor audience must gate on component replacement and quiescence | not stated | 9 |
| EXECUTABLE_INSTALLATION36 | "Visibility enables future entry; quiescence permits retirement of existing code." | Visibility and retirement | Visibility and quiescence must be modeled as separate capabilities: visibility for future entry, quiescence for retirement | not stated | 9 |
| EXECUTABLE_INSTALLATION37 | "They are distinct facts, not interchangeable tokens." | Visibility and retirement | Visibility and quiescence must remain distinct facts and must not be interchangeable tokens | not stated | 8 |
| EXECUTABLE_INSTALLATION38 | "Patching live code uses admitted fragments at declared sites through the replacement path." | Visibility and retirement | Patching live code must use admitted fragments at declared sites only via the replacement path | not stated | 9 |
| EXECUTABLE_INSTALLATION39 | "Installation reports `HardwareEnforced` or `ConventionOnly` W^X;" | Visibility and retirement | Installation must report W^X as `HardwareEnforced` or `ConventionOnly` | not stated | 9 |
| EXECUTABLE_INSTALLATION40 | "an `Unsupported` provider rejects." | Visibility and retirement | `Unsupported` W^X providers must reject installation | not stated | 9 |
| EXECUTABLE_INSTALLATION41 | "A synchronous visibility operation may suspend under its declared contract;" | Visibility and retirement | Synchronous visibility may suspend only under its declared contract | at a call | 10 |
| EXECUTABLE_INSTALLATION42 | "nonblocking completion needs an explicit provider protocol." | Visibility and retirement | Nonblocking visibility completion must use an explicit provider protocol | not stated | 9 |
| EXECUTABLE_INSTALLATION43 | "Retirement binds the exact installed realization and proves quiescence, execute removal, restored write authority, and required provider-canonical completion facts before returning placement." | Visibility and retirement | Retirement must bind exact installed realization and prove quiescence, execute removal, restored write authority, and provider-canonical completion facts before returning placement | not stated | 9 |
| EXECUTABLE_INSTALLATION44 | "Incomplete drain keeps the installed realization and accepted receipt quarantined." | Visibility and retirement | Incomplete drain must keep installed realization and accepted receipt quarantined | not stated | 9 |
| EXECUTABLE_INSTALLATION45 | "A stale-entry fault presents that exact opaque context, not a compact report ID." | Visibility and retirement | Stale-entry faults must present exact opaque context, not a compact report ID | not stated | 9 |
| EXECUTABLE_INSTALLATION46 | "Installed roots borrow installed code and therefore prevent reclaiming it while entry remains possible." | Visibility and retirement | Installed roots borrowing installed code must prevent reclaim while entry remains possible | not stated | 2 |
| EXECUTABLE_INSTALLATION47 | "Preventing code injection does not establish legal forward-edge targets within already-admitted code." | Control-flow integrity | Code-injection prevention must not be treated as establishing legal forward-edge targets within admitted code | not stated | 3 |
| EXECUTABLE_INSTALLATION48 | "Indirect calls require sealed requirement-compatible entry references/descriptors retaining satisfier and contract identity;" | Control-flow integrity | Indirect calls must use sealed requirement-compatible entry references or descriptors that retain satisfier and contract identity | at a call | 3 |
| EXECUTABLE_INSTALLATION49 | "component boundaries use bindings, not exported local descriptors." | Control-flow integrity | Component boundaries must use bindings, not exported local descriptors | not stated | 3 |
| EXECUTABLE_INSTALLATION50 | "Checked return integrity derives from memory safety, sufficient WCSU, and compiler-owned live/parked control state that ordinary source cannot address." | Control-flow integrity | Checked return integrity must derive from memory safety, sufficient WCSU, and compiler-owned live/parked control state inaccessible to ordinary source | not stated | SHARED |
| EXECUTABLE_INSTALLATION51 | "Assembly retains stack/control effects;" | Control-flow integrity | Assembly must retain stack and control effects in the checked model | not stated | 10 |
| EXECUTABLE_INSTALLATION52 | "opaque providers need admitted call/state exits or hardware isolation." | Control-flow integrity | Opaque providers must have admitted call/state exits or hardware isolation | not stated | 9 |
| EXECUTABLE_INSTALLATION53 | "Missing evidence rejects." | Control-flow integrity | Missing required evidence must reject | not stated | 8 |
| EXECUTABLE_INSTALLATION54 | "Independent final-byte transfer validation and optional hardware hardening are separate assurance work, not substitutes for those rules." | Control-flow integrity | Final-byte transfer validation and optional hardware hardening must not substitute for the stated control-flow integrity rules | not stated | 9 |
| EXECUTABLE_INSTALLATION55 | "The canonical native component container has bounded length-delimited tables, checked arithmetic/ranges, nonoverlap, a closed relocation vocabulary, and required contract/footprint sections." | Artifact admission and boot | Canonical native component containers must meet bounded tables, checked arithmetic/ranges, nonoverlap, closed relocation vocabulary, and required contract/footprint sections | not stated | 9 |
| EXECUTABLE_INSTALLATION56 | "It has no constructors, scripts, ambient imports, recursive metadata, or permissive semantic extensions." | Artifact admission and boot | Component containers must not include constructors, scripts, ambient imports, recursive metadata, or permissive semantic extensions | not stated | 9 |
| EXECUTABLE_INSTALLATION57 | "Informational sections grant no authority;" | Artifact admission and boot | Informational sections must grant no authority | not stated | 9 |
| EXECUTABLE_INSTALLATION58 | "meaning/trust-bearing sections are required." | Artifact admission and boot | Meaning/trust-bearing sections must be present | not stated | 9 |
| EXECUTABLE_INSTALLATION59 | "A UEFI PE/COFF envelope is not the component format." | Artifact admission and boot | UEFI PE/COFF envelopes must not be accepted as the component format | not stated | 9 |
| EXECUTABLE_INSTALLATION60 | "That trust does not bypass sealed admission, final-byte/placement validation, footprint checks or the W^X lifecycle." | Artifact admission and boot | Producer trust accepted by a receiver must not bypass sealed admission, final-byte/placement validation, footprint checks, or the W^X lifecycle | not stated | 9 |
| EXECUTABLE_INSTALLATION61 | "Decode/structural validation yields only an immutable candidate." | Artifact admission and boot | Decode/structural validation must produce only an immutable candidate, not admission | not stated | 9 |
| EXECUTABLE_INSTALLATION62 | "Admission is separate." | Artifact admission and boot | Admission must remain a separate step from decode/structural validation | not stated | 9 |
| EXECUTABLE_INSTALLATION63 | "Exact content and normalized semantic promises define identity;" | Artifact admission and boot | Identity must come from exact content and normalized semantic promises | not stated | 9 |
| EXECUTABLE_INSTALLATION64 | "presentation order and informational sections do not." | Artifact admission and boot | Presentation order and informational sections must not define identity | not stated | 9 |
| EXECUTABLE_INSTALLATION65 | "Backend translation takes the validated carrier and rejects target or relocation mismatch atomically." | Artifact admission and boot | Backend translation must reject target or relocation mismatch atomically when consuming the validated carrier | not stated | 9 |
| EXECUTABLE_INSTALLATION66 | "trusted build validates and signs an admitted identity;" | Artifact admission and boot | Boot base case must validate and sign admitted identity through trusted build | not stated | 9 |
| EXECUTABLE_INSTALLATION67 | "secure boot authenticates and gates entry;" | Artifact admission and boot | Secure boot must authenticate and gate entry in the boot base case | at entry | 9 |
| EXECUTABLE_INSTALLATION68 | "measured boot records the entered identity;" | Artifact admission and boot | Measured boot must record the entered identity | at entry | 9 |
| EXECUTABLE_INSTALLATION69 | "the boot-admitted installer admits later artifacts." | Artifact admission and boot | Boot-admitted installer must admit later artifacts in the boot base case | not stated | 9 |
| EXECUTABLE_INSTALLATION70 | "Measurement alone never establishes admission." | Artifact admission and boot | Measurement by itself must not establish admission | not stated | 9 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "An immutable reusable `Artifact` is eligible only through the sealed `Artifact::AdmittedExecutable` qualification." | Eligibility for installation or execution always requires the sealed qualification on the artifact. | "Eligible" names install-time gating only; emitting or holding artifacts without the qualification may still be allowed elsewhere. |
| "Packages cannot self-establish it; mutation destroys it." | Any package-initiated operation that would mint `AdmittedExecutable` must fail; any mutation after admission voids the qualification. | "Self-establish" forbids admission without an external receiver step; mutation might destroy only the installation binding while the certificate remains reportable per EXECUTABLE_INSTALLATION15. |
| "Ordinary compilation grants no installation or execution authority and requires no receiving policy." | Compiled artifacts must lack install/execute authority until receiver admission; compile must not require receiver policy. | "Ordinary compilation" excludes special build modes that might attach authority; this chapter does not state whether non-ordinary builds differ. |
| "A certificate may remain reportable/reusable while installation consumes the validated placement." | Certificate facts may persist for reporting after the validated placement is consumed by installation. | Certificate reuse may imply a new installation may skip decode; whether a second install must repeat lifecycle steps is not stated. |
| "Requirement binding is separate and later." | Requirement binding occurs after placement-scope binding in the ordered lifecycle. | Requirement binding is a distinct mechanism that may occur in source or build phases without a fixed ordering relative to materialization. |
| "Every route to execute permission follows this rule." | All execute permission must go through the full placement lifecycle in this section. | "This rule" might mean only the scope/provider binding sentence immediately above; other paragraphs could add compatible exceptions. |
| "Installation separately validates W^X, target cache/order and instruction-fetch visibility, and audience through one contracted provider operation." | A single provider call must perform all listed validations together. | Validations are logically separate but packaged into one operation; partial failure handling is not stated. |
| "checked assembly retains the same installation authority and reach obligations." | Checked assembly paths must meet identical installation checks as non-assembly translation. | "Same" obligates reach summaries only; assembly might omit steps that translation performs before install. |
| "The compiler checks entry, installation, and custody; it does not own processor discovery or the boot protocol." | Compiler must enforce entry/install/custody checks but must not select processors or boot protocols. | Compiler performs static checks only; runtime custody enforcement may belong to providers without compiler ownership of discovery. |
| "Visibility enables future entry; quiescence permits retirement of existing code." | Future entry requires visibility facts; retirement requires quiescence facts. | Visibility and quiescence might be recorded in either order; which must precede the other for concurrent audiences is not stated. |
| "Installation reports `HardwareEnforced` or `ConventionOnly` W^X; an `Unsupported` provider rejects." | Install must classify W^X into one of the two reported kinds or reject on unsupported. | Reporting might be informational while execution still proceeds under convention-only enforcement elsewhere. |
| "A synchronous visibility operation may suspend under its declared contract;" | Synchronous visibility may block under an explicit suspend contract. | "May suspend" is permissive; providers might also complete without suspending when the contract allows. |
| "Patching live code uses admitted fragments at declared sites through the replacement path." | Live patching is permitted only via admitted fragments, declared sites, and the replacement path. | Patching describes one route; whether cold install avoids replacement-path rules is not stated in this sentence. |
| "Preventing code injection does not establish legal forward-edge targets within already-admitted code." | Admitted code still needs sealed indirect-call targets for forward edges. | Injection prevention and forward-edge rules are independent; absence of injection does not imply CFI is complete. |
| "Checked return integrity derives from memory safety, sufficient WCSU, and compiler-owned live/parked control state that ordinary source cannot address." | Return integrity is proven from listed mechanisms only. | "Sufficient WCSU" might be an external predicate; which compiler checks establish sufficiency is not stated here. |
| "Assembly retains stack/control effects; opaque providers need admitted call/state exits or hardware isolation." | Assembly-checked code keeps stack/control effects; opaque providers must expose admitted exits or isolation. | "Retains" might mean assembly lowering preserves effects in Psi only; provider obligations might be receiver-side only. |
| "Decode/structural validation yields only an immutable candidate. Admission is separate." | Candidates after decode must not be treated as admitted until a separate admission step. | Structural validation might run repeatedly; whether re-decode invalidates prior candidates is not stated. |
| "Exact content and normalized semantic promises define identity; presentation order and informational sections do not." | Identity hashing or comparison must ignore presentation order and informational sections. | Identity for reporting might ignore order while install-time byte comparison still uses exact layout bytes. |
| "A receiver may explicitly trust an accepted producer for checks it does not repeat." | Trusted producer skips only named checks; other checks remain mandatory. | Trust might waive decode checks entirely if the producer is on an accept list; scope of waivable checks is not enumerated here. |
| "The boot base case keeps the same boundary: trusted build validates and signs an admitted identity; secure boot authenticates and gates entry; measured boot records the entered identity; the boot-admitted installer admits later artifacts." | All four mechanisms participate without replacing sealed admission rules. | Boot narrative is descriptive; which steps the compiler enforces versus firmware enforces is not split in this sentence. |
| "Measurement alone never establishes admission." | Measured-boot records cannot substitute for sealed admission. | Measurement might be required evidence accompanying admission established by another step; whether absence of measurement rejects install is not stated. |

## Terms used without definition in this chapter

- general executable-memory capability
- arbitrary bytes-to-code conversion
- JIT
- self-modifying-code route
- `Artifact`
- `Artifact::AdmittedExecutable`
- admission
- relocation
- required evidence
- footprint
- placement plan
- receiving environment
- installation boundary
- emitting an executable file
- installation authority
- execution authority
- receiving policy
- ecosystem
- admitted evidence
- policy (receiver)
- semantic states
- canonical decode
- receiver-required contract/evidence admission
- `CodePlacement`
- borrowed artifact
- freeze
- validate exact final bytes
- installer provider
- visibility completion
- certificate
- validated placement
- domain-framed strong digests
- compact compatibility values
- report keys
- acceptance comparison
- materialization
- sealed entry/data identities
- private copy
- W^X
- target cache/order
- instruction-fetch visibility
- audience
- contracted provider operation
- scope
- physical/virtual extent authority
- replaceable dispatch binding
- requirement binding
- execute permission
- translation providers
- admitted-artifact provenance
- checked assembly
- reach obligations
- device providers
- device firmware / GPU / NIC uploads
- host executable artifacts
- secondary-processor startup
- boot-protocol provider
- trampoline
- low-memory placement
- vector geometry
- startup contract
- entry / installation / custody
- processor discovery
- boot protocol
- dispatch / confirmed arrival / safe resource release (linked section)
- dormant/local target
- future remote fetcher
- possible current executor
- visibility
- quiescence
- component replacement
- replacement path
- admitted fragments
- declared sites
- `HardwareEnforced` / `ConventionOnly` / `Unsupported`
- synchronous visibility operation
- explicit provider protocol
- retirement
- installed realization
- execute removal
- restored write authority
- provider-canonical completion facts
- incomplete drain
- accepted receipt
- quarantined
- stale-entry fault
- opaque context
- compact report ID
- installed roots
- component publication (linked chapter)
- code injection
- forward-edge targets
- indirect calls
- entry references/descriptors
- satisfier
- contract identity
- bindings
- exported local descriptors
- checked return integrity
- memory safety
- sufficient WCSU
- live/parked control state
- opaque providers
- admitted call/state exits
- hardware isolation
- final-byte transfer validation
- hardware hardening
- canonical native component container
- bounded length-delimited tables
- closed relocation vocabulary
- contract/footprint sections
- constructors / scripts / ambient imports
- recursive metadata
- permissive semantic extensions
- informational sections
- meaning/trust-bearing sections
- UEFI PE/COFF envelope
- accepted producer
- trusted build
- immutable candidate
- normalized semantic promises
- presentation order
- validated carrier
- backend translation
- secure boot
- measured boot
- boot-admitted installer
- measurement

CHAPTER COMPLETE
