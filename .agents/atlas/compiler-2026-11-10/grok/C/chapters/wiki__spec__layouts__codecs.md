# Codecs and format policies

Source: `wiki/spec/layouts/codecs.md` lines 1–203 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
CODECS1 | "Encoding, decoding, and validation are ordinary checked library-machine requirements." | Codecs and format policies | Encoding, decoding, and validation must be treated and checked as ordinary library-machine requirements | not stated | 10
CODECS2 | "Bodies may be authored or generated from a validated plan; neither origin bypasses conformance checks." | Codecs and format policies | Authored and generated bodies must both satisfy conformance checks; origin must not skip those checks | not stated | 8
CODECS3 | "Their names are not compiler built-ins." | Codecs and format policies | Format package names (for example protobuf) must not be treated as compiler built-ins | not stated | 9
CODECS4 | "Fixed-memory layout and variable-length wire encoding have distinct consuming capabilities even when both expose a library family such as `CLayout<T>` or `ProtoLayout<T>`." | Codecs and format policies | Fixed-memory layout and variable-length wire encoding must not be conflated into one consuming capability merely because they share a library family name | not stated | 7
CODECS5 | "The standard ordinary trait `TryFrom<Source, Destination, Error>` supplies a checked conversion returning `Result<Destination, Error>`." | Representation conversions | `TryFrom` conversions must be checked and return `Result<Destination, Error>` | at the operation | 7
CODECS6 | "Both endpoints are explicit complete types; there is no implicit `Self`, data subject or search for a visible implementation." | Representation conversions | Conversion endpoints must be explicit complete types; implicit `Self`, data subject, and visible-implementation search must be rejected | at a call | 7
CODECS7 | "Full-conversion representation families provide selected conversions to and from their semantic T under their stated resource and value preconditions." | Representation conversions | Full-conversion families must expose only selected conversions under stated resource and value preconditions | not stated | 7
CODECS8 | "To/Into-style helpers delegate to that same selected implementation rather than defining a competing conversion map." | Representation conversions | To/Into-style helpers must delegate to the selected implementation and must not define a separate conversion map | not stated | 7
CODECS9 | "A named implementation supplies this map; the storage data type does not itself satisfy the trait." | Representation conversions | The storage data type must not satisfy `Representation`; only a named implementation may supply the conversion map | not stated | 1
CODECS10 | "Fixed geometry alone does not satisfy Representation or authorize a decoder." | Representation conversions | Fixed geometry without the selected map must not satisfy `Representation` or authorize decoding | not stated | 1
CODECS11 | "Generic callers require the selected map and its public guarantees instead of inferring conversion from a layout name or the existence of a plan machine." | Representation conversions | Generic calls must use the selected map and public guarantees; inference from layout name or plan-machine existence must be rejected | at a call | 7
CODECS12 | "Buffers, allocator state, work sponsorship and output custody are explicit inputs." | Representation conversions | Buffer, allocator, sponsorship, and custody inputs must be explicit in conversion interfaces | at a call | 2
CODECS13 | "A buffered or allocating conversion carries them in its explicit source request or uses the corresponding explicitly parameterized codec requirement; the simple source-to-result form does not grant an ambient allocator, hidden capture environment or host-service access." | Representation conversions | Buffered or allocating conversions must thread explicit inputs; the simple source-to-result form must not imply ambient allocator, hidden capture, or host-service access | at a call | 3
CODECS14 | "Failure retains or disposes of borrowed/owned inputs under the declared custody contract and never exposes an unestablished destination as a valid value." | Representation conversions | On failure, inputs must follow the custody contract and an unestablished destination must never be exposed as a valid value | at the operation | 2
CODECS15 | "In-place failure and partially written output obey the particular codec's documented contract." | Representation conversions | In-place failure and partial output must match the codec's documented contract | at the operation | 3
CODECS16 | "A selected conversion may additionally expose an infallible `From` convenience only when totality is established for every admitted input under its complete preconditions." | Representation conversions | Infallible `From` may appear only when totality is proved for every admitted input under complete preconditions | not stated | 7
CODECS17 | "It delegates to the same implementation; the compiler may eliminate a proved-unreachable error path, never assume one away." | Representation conversions | Infallible convenience must delegate to the same implementation; unreachable error paths may be eliminated only when proved unreachable, not assumed away | at the operation | 8
CODECS18 | "There is no silent layout coercion at a call or foreign boundary." | Representation conversions | Silent layout coercion at calls or foreign boundaries must be rejected | at a call | 3
CODECS19 | "A full conversion contract requires actual coverage, not just signatures: supported values encode successfully with sufficient declared resources; valid representations decode to established values; malformed or nonrepresentable inputs fail; and the selected round-trip or preservation law holds." | Representation conversions | Implementations claiming full conversion must actually encode supported values, decode to established values, fail on malformed input, and satisfy the selected round-trip or preservation law | not stated | 8
CODECS20 | "Returning Unsupported for every input does not implement such a contract." | Representation conversions | A conversion that always returns Unsupported must not be accepted as implementing a full conversion contract | not stated | 8
CODECS21 | "Geometry-only, sparse, read-only and write-only policies remain usable under their actual capabilities without claiming complete bidirectional conversion." | Representation conversions | Policies with partial capabilities must not be checked as claiming complete bidirectional conversion unless they meet full-contract requirements | not stated | 8
CODECS22 | "Cross-representation adapters compose through semantic T instead of requiring a separately authored adapter for every pair." | Representation conversions | Cross-representation adapters must compose through semantic T rather than requiring a distinct adapter per pair | not stated | 7
CODECS23 | "Optimizations may remove intermediate storage only while preserving behavior, custody, effects and failure contracts." | Representation conversions | Optimizations that remove intermediate storage must preserve behavior, custody, effects, and failure contracts | not stated | 3
CODECS24 | "Unsupported members reject with their exact field/case and missing operation; no member is silently skipped." | Representation conversions | Unsupported members must reject with exact field or case and missing operation; silent skipping is forbidden | at the operation | 1
CODECS25 | "Semantic field changes, historical migrations and custom normalization still require explicit selected rules." | Representation conversions | Semantic field changes, migrations, and custom normalization must have explicit selected rules | not stated | 8
CODECS26 | "Generated source text, macros, a format-specific derive operation and spelling-based proof rules are not the mechanism, and specialization alone establishes no new trust classification." | Representation conversions | Trust classification must not rest on generated text, macros, derive ops, spelling-based proofs, or specialization alone | not stated | 8
CODECS27 | "Raw bytes are not automatically an established T." | Representation conversions | Raw bytes must not be treated as an established `T` without establishment | at the operation | 8
CODECS28 | "A representation carrier's physical geometry and its current value-validity facts remain separate; checked construction/conversion establishes the latter before an API relies on them." | Representation conversions | Value-validity must be established by checked construction or conversion before APIs may rely on it; geometry alone is insufficient | before use after construction/conversion | 8
CODECS29 | "Exclusive byte writes and deferred establishment use the existing invariant-window and restoration rules." | Representation conversions | Exclusive byte writes and deferred establishment must follow invariant-window and restoration rules | at the operation | 2
CODECS30 | "Layout names mint neither backing nor authority." | Representation conversions | Layout names must not confer backing storage or authority by themselves | not stated | 8
CODECS31 | "Variable-length representations need explicit encoded extent and owned or borrowed storage, with their ordinary lifetime and allocation obligations; they do not become fixed-size or addressable C records." | Representation conversions | Variable-length representations must carry explicit extent and storage with lifetime and allocation obligations; they must not be treated as fixed-size addressable C records | not stated | 6
CODECS32 | "If ProtoLayout names protobuf, its library must implement and document protobuf's actual field-number, wire-type, presence, unknown-member and compatibility rules." | Representation conversions | A protobuf-named `ProtoLayout` library must implement and document protobuf's field-number, wire-type, presence, unknown-member, and compatibility rules | not stated | 8
CODECS33 | "No arbitrary community format is installed or implemented by this ratification." | Representation conversions | This ratification must not install or implement arbitrary community formats | not stated | 9
CODECS34 | "A concrete codec conformance proves its public agreement requirements, including the current-shape law `decode(encode(value)) == value` where required by the codec contract." | Agreement and trust | Codec conformance must prove public agreement requirements including the current-shape round-trip law when the contract requires it | not stated | 8
CODECS35 | "Historical migration belongs to the format lineage and composes outside that law." | Agreement and trust | Historical migration must be modeled outside the current-shape round-trip law, in the format lineage | not stated | 8
CODECS36 | "Authored or generated body independently checked against the public requirement" | Agreement and trust | Bodies independently checked against the public requirement must be classified as Derived trust | not stated | 8
CODECS37 | "Generator accepted as correct by construction" | Agreement and trust | Generator-accepted correct-by-construction realizations must be classified Admitted with the compiler as trusted party | not stated | 8
CODECS38 | "Opaque foreign realization" | Agreement and trust | Opaque foreign realizations must be classified Admitted naming the provider | not stated | 8
CODECS39 | "Artifacts retain normalized plan, requirement identity, realization origin, trust class, and evidence independently." | Agreement and trust | Artifacts must retain plan, requirement identity, realization origin, trust class, and evidence as separate retained facts | not stated | 8
CODECS40 | "Generated origin does not mean derived trust." | Agreement and trust | Generated origin alone must not imply Derived trust classification | not stated | 8
CODECS41 | "`Derived` requires independently checked evidence establishing the public requirement for every input covered by its contract, tied to the exact codec realization and schema." | Agreement and trust | Derived classification requires independent checked evidence for every contract-covered input, tied to exact realization and schema | not stated | 8
CODECS42 | "Differential probes over selected values and malformed frames are regression checks, not a general agreement argument." | Agreement and trust | Differential probes must not be treated as general agreement proof | not stated | 8
CODECS43 | "Reporting their finite scope does not qualify the realization as `Derived`." | Agreement and trust | Finite-scope differential reports must not qualify a realization as Derived | not stated | 8
CODECS44 | "Without sufficient agreement evidence, a generated realization remains `Admitted`, naming the compiler as the trusted party." | Agreement and trust | Generated realizations without sufficient agreement evidence must stay Admitted with compiler as trusted party | not stated | 8
CODECS45 | "A validator may establish a predicate only because its checked contract proves it; user code cannot construct a supposedly valid payload without that proof." | Boundary establishment | Validators may establish predicates only via checked contract proof; user code must not construct supposedly valid payloads without that proof | at the operation | 8
CODECS46 | "Decoding alone establishes no trust, authority, or device-correspondence fact." | Boundary establishment | Decoding must not establish trust, authority, or device-correspondence facts by itself | at the operation | 8
CODECS47 | "Outbound code forgets facts or executes an explicit encoding/conversion before crossing the boundary." | Boundary establishment | Outbound boundary crossing must forget facts or perform explicit encoding or conversion first | at a call | 3
CODECS48 | "A provider's foreign representation does not change ordinary application types." | Boundary establishment | Foreign provider representations must not alter ordinary application types | not stated | 3
CODECS49 | "[Recasts](recasts.md) preserve representation; they cannot fabricate stronger validity or replace executable conversion." | Boundary establishment | Recasts must preserve representation only; stronger validity fabrication and replacement of executable conversion must be rejected | at the operation | 5
CODECS50 | "A codec call uses the exact implementation already selected by its representation application or names the implementation explicitly." | Policy selection and durability | Codec calls must use the representation-selected implementation or an explicit name | at a call | 9
CODECS51 | "Domain qualification does not choose storage geometry or encoding policy." | Policy selection and durability | Domain qualification must not select storage geometry or encoding policy | not stated | 5
CODECS52 | "Adding an unrelated import or conformance cannot change candidate meaning; third-party conformances are callable by name, not visible-conformance search." | Policy selection and durability | Unrelated imports or conformances must not change candidate resolution; third-party conformances must be resolved by name only, not visible-conformance search | at a call | 7
CODECS53 | "Durability and self-description are properties of the normalized plan and its consuming API contract, not semantic domains on arbitrary bytes." | Policy selection and durability | Durability and self-description must not be inferred as semantic domains on arbitrary bytes | not stated | 8
CODECS54 | "Channel/store compatibility checks must join published schemas, plans, historical shapes, and migrations; they are not implied by one codec round-trip law." | Policy selection and durability | Channel or store compatibility checks must combine schemas, plans, historical shapes, and migrations; a single round-trip law must not imply them | not stated | 8
CODECS55 | "A missing required field is invalid." | Presence and representation | Missing required fields must be treated as invalid | at the operation | 1
CODECS56 | "An omitted `Optional<T>` field decodes as `None`; omission is its canonical encoding, while `Some(value)` encodes the field and value." | Presence and representation | Omitted `Optional<T>` must decode as `None`; `Some(value)` must encode field and value | at the operation | 1
CODECS57 | "New required semantic values need an authored migration, not an implicit wire default." | Presence and representation | New required semantic values must require an authored migration; implicit wire defaults must be rejected | not stated | 8
CODECS58 | "Exact arrays contain exactly their declared count;" | Presence and representation | Exact arrays must contain exactly the declared element count | at the operation | 6
CODECS59 | "bounded-live-length, growable owned, and borrowed sequences retain their own length, allocation, loan, and work obligations." | Presence and representation | Bounded-live-length, growable owned, and borrowed sequences must retain their length, allocation, loan, and work obligations | not stated | 2
CODECS60 | "Stable case identities do not select those encodings." | Presence and representation | Stable case identities must not select home-layout case encodings | not stated | 1
CODECS61 | "A representation-sensitive API states the relationship it requires as an ordinary checked obligation." | Presence and representation | Representation-sensitive APIs must express required relationships as ordinary checked obligations | not stated | 8
CODECS62 | "The `Optional<T>` home-representation obligation that zero denotes `None` is separate from the codec's omission rule." | Presence and representation | Home-representation zero-means-`None` obligations must be kept separate from codec omission rules | not stated | 1
CODECS63 | "A layout that violates an authored representation obligation rejects where that obligation is declared." | Presence and representation | Layouts violating an authored representation obligation must reject at the obligation declaration site | not stated | 8
CODECS64 | "`StrictDecode<Policy, Value>` validates the complete input and rejects unknown fields or cases." | Unknown members and remainder custody | `StrictDecode` must validate complete input and reject unknown fields or cases | at the operation | 1
CODECS65 | "`ProjectingDecode<Policy, Value>` validates known members and discards unknown ones." | Unknown members and remainder custody | `ProjectingDecode` must validate known members and discard unknown ones | at the operation | 1
CODECS66 | "`PreservingDecode<Policy, Value>` preserves them for relay." | Unknown members and remainder custody | `PreservingDecode` must preserve unknown members for relay | at the operation | 1
CODECS67 | "These are distinct normalized requirements, not an unrecorded decoder option." | Unknown members and remainder custody | Strict, projecting, and preserving decode policies must be distinct normalized requirements, not an implicit decoder option | not stated | 8
CODECS68 | "`DecodeResult<T>` is the fail-closed result sum; preserving decode returns `DecodeResult<Relayed<T>>`." | Unknown members and remainder custody | Decoding must use fail-closed `DecodeResult<T>`; preserving decode must return `DecodeResult<Relayed<T>>` | at the operation | 1
CODECS69 | "Facts about `T` describe only the known validated value." | Unknown members and remainder custody | Facts about `T` must apply only to the known validated value, not remainder bytes | not stated | 8
CODECS70 | "A zero-copy remainder or decoded view retains an input-buffer loan; an owned copy carries explicit allocation and resource obligations." | Unknown members and remainder custody | Zero-copy remainders or views must retain input-buffer loans; owned copies must carry explicit allocation and resource obligations | not stated | 2
CODECS71 | "Packed-varint decoding cannot substitute a borrowed scalar view for the required owned or caller-provided mutable destination." | Unknown members and remainder custody | Packed-varint decoding must not use a borrowed scalar view when an owned or caller-provided mutable destination is required | at the operation | 6
CODECS72 | "Published historical shapes are immutable ordinary declarations." | Historical lineages and migration | Published historical shapes must remain immutable ordinary declarations | not stated | 8
CODECS73 | "The standard `FormatMigration<Lineage, Old, New>::migrate` requirement selects an explicitly bound checked conversion for the exact lineage and shapes." | Historical lineages and migration | Migration must be an explicitly bound checked conversion for the exact lineage and shapes | at a call | 7
CODECS74 | "Reverse and fallible conversions are separate requirements; an upgrade promises neither reversibility nor a downgrade." | Historical lineages and migration | Reverse and fallible migrations must be separate requirements; upgrades must not implicitly promise reversibility or downgrade | not stated | 7
CODECS75 | "Changing a field's semantic domain is a type change even with the same carrier." | Historical lineages and migration | Semantic domain changes must be treated as type changes even when the carrier is unchanged | not stated | 5
CODECS76 | "Decode policy handles unknown eras explicitly: reject, preserve, negotiate, or another contracted choice." | Historical lineages and migration | Decode policy for unknown eras must be an explicit contracted choice | not stated | 8
CODECS77 | "Exhaustive matching checks known cases, but a wildcard does not prove that every known era has a migration route." | Historical lineages and migration | Wildcard match exhaustiveness must not be treated as proving migration coverage for every known era | not stated | 8
CODECS78 | "Such completeness needs the selected migration evidence." | Historical lineages and migration | Claims of complete era coverage must be backed by selected migration evidence | not stated | 8
CODECS79 | "Generators may traverse schemas; they do not choose persistent fields, atomic snapshots, or migration meaning." | Historical lineages and migration | Generators must not select persistent fields, atomic snapshots, or migration meaning | not stated | 9
CODECS80 | "The `compact_binary` grammar starts with its policy discriminator and emits fields in increasing stable-identity order." | Compact binary policy | `compact_binary` encoding must use policy discriminator first and field order by increasing stable identity | at the operation | 6
CODECS81 | "Integers use canonical minimal unsigned LEB128 groups, with zigzag for signed values; Boolean encodings are exactly `0` and `1`." | Compact binary policy | Integers must use canonical minimal unsigned LEB128 and zigzag where signed; Booleans must encode only as `0` or `1` | at the operation | 5
CODECS82 | "Nested records are length-delimited and their decoding must finish exactly at the declared sub-region end." | Compact binary policy | Nested record decode must be length-delimited and end exactly at the declared sub-region boundary | at the operation | 6
CODECS83 | "Reads are bounds-checked." | Compact binary policy | Compact-binary reads must be bounds-checked | at the operation | 6
CODECS84 | "Its strict decoder returns `Invalid` on malformed values, unexpected identities, truncation, range violations, or noncanonical encoding." | Compact binary policy | Strict `compact_binary` decoder must return `Invalid` for malformed input, bad identities, truncation, range violations, or noncanonical encodings | at the operation | 6
CODECS85 | "That failure verdict is authoritative; partially written output fields and consumed-byte count are unspecified." | Compact binary policy | On strict decode failure, the failure verdict is authoritative; partial output field state and consumed-byte count must not be specified as observable | at the operation | 3
CODECS86 | "`Sound` establishes every destination carrier and declared field domain." | Compact binary policy | `Sound` decoding must establish every destination carrier and declared field domain | at normal return | 8
CODECS87 | "Generated origin does not independently prove this contract; an unverified generated realization remains compiler-admitted." | Compact binary policy | Unverified generated `compact_binary` realizations must remain compiler-admitted; generated origin must not alone prove the contract | not stated | 8

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"A format package publishes its selected [layout plan](plans.md), codec requirements, realizations, and trust evidence." | The compiler must require a format package to publish all listed artifacts before use. | This describes package authoring expectations; what the compiler enforces at compile time is not stated. |
"A C policy may ship in core or std; protobuf and other formats may be independently authored packages." | Any of these distribution locations is allowed without extra compiler checks. | "May ship" might constrain which packages are valid format providers versus merely describing where policies can live. |
"Full-conversion representation families provide selected conversions to and from their semantic T under their stated resource and value preconditions." | Families without selected conversions cannot claim full-conversion status. | "Provide" might mean the package must supply implementations, or only that the family design includes such conversions when complete. |
"The standard ordinary `Representation` trait groups those exact selected conversion applications and their agreement laws, with error and resource contracts explicit." | Implementations must expose explicit error and resource contracts on the trait. | Grouping might be organizational; whether implicit contracts are rejected is not stated in this sentence. |
"Geometry-only, sparse, read-only and write-only policies remain usable under their actual capabilities without claiming complete bidirectional conversion." | The compiler must allow partial policies and reject only false full-conversion claims. | "Remain usable" might be a portability statement for library authors, not a compiler acceptance rule. |
"The representation author supplies generic reading/materialization and writing/encoding rules once." | One generic rule set per representation is required. | Authors "supply" rules in documentation or generated code; compiler enforcement of "once" is not stated. |
"[Typed reflection](../language/reflection.md) and ordinary specialization elaborate the authorized fields, active cases and exact callback selections for each supported T." | Specialization must match authorized fields, cases, and callbacks. | Elaboration might happen only in library tooling; which elaborations the compiler must verify is not stated here. |
"Inbound code receives raw bytes or inert pointers under a boundary contract, validates or materializes the selected representation, establishes proven predicates and any separately authorized qualification, then exposes Omega values or checked borrowed views." | Inbound pipelines must follow this ordered establishment sequence. | This might be normative library behavior; compiler ordering obligations at foreign boundaries are not fully specified. |
"A durable store may require stable identity and reader-tolerance guarantees. An ephemeral cache may explicitly select a policy without them." | Durability features are optional per store kind. | "May require" and "may explicitly select" leave unclear whether missing guarantees reject compilation or only affect runtime contracts. |
"Home-layout facts describe case encodings and the meaning of zero storage;" | Home-layout facts are the authority for those encodings and zero meaning. | "Describe" might be documentary; linkage to compile-time facts versus runtime layout is not stated in this sentence. |
"case identity need not occupy a separate discriminant under [home-layout realization](plans.md#home-layout-realization)." | Encodings without separate discriminants are permitted when home-layout rules allow. | "Need not" permits but does not require omission; when omission is mandatory is not stated. |
"`Relayed<T>` separates its validated value from an `OpaqueWireRemainder` binding the producing codec identity and a codec-private envelope containing exact unknown-member bytes and their relay-ordering sidecar." | Implementations must physically separate value and remainder with the stated bindings. | The sentence defines a type shape; whether partial or incorrect separation is a compile-time or runtime violation is not stated. |
"Opaque means semantically uninterpreted, not confidential or unforgeable." | Remainder bytes carry no semantic facts in the proof system. | "Opaque" might still allow cryptographic or access-control handling outside semantics; scope of "uninterpreted" is not stated. |
"Independent lineages may share carriers without sharing migration edges." | Shared carriers must not imply shared migration graphs. | "May share" allows sharing; whether the compiler must track lineage separation when carriers unify is not stated. |
"It adds no intrinsic version identity to either type." | Migration types must not gain built-in version fields from the trait alone. | Version identity might still be carried elsewhere; what "intrinsic" excludes is not fully stated. |
"The compact-binary policy below is a different format, not a protobuf synonym." | `compact_binary` and protobuf must not be treated as interchangeable names or codecs. | This is naming clarity only; whether conflating them is a compile-time error is not stated. |

## Terms used without definition in this chapter

- checked library-machine requirements
- format package
- layout plan
- codec requirements
- realizations
- trust evidence
- validated plan
- conformance checks
- `standard-fallible-representation-conversions`
- representation package
- C policy
- compiler built-ins
- fixed-memory layout
- variable-length wire encoding
- consuming capabilities
- `CLayout<T>` / `ProtoLayout<T>`
- `TryFrom<Source, Destination, Error>`
- explicit complete types
- data subject
- visible implementation
- full-conversion representation families
- semantic T
- resource and value preconditions
- To/Into-style helpers
- `Representation` trait
- agreement laws
- named implementation
- storage data type
- fixed geometry
- plan machine
- work sponsorship
- output custody
- source request
- explicitly parameterized codec requirement
- simple source-to-result form
- ambient allocator
- hidden capture environment
- host-service access
- custody contract
- unestablished destination
- in-place failure
- partially written output
- infallible `From`
- totality
- complete preconditions
- proved-unreachable error path
- silent layout coercion
- foreign boundary
- full conversion contract
- Unsupported
- geometry-only / sparse / read-only / write-only policies
- generic reading/materialization and writing/encoding rules
- typed reflection
- ordinary specialization
- authorized fields / active cases / callback selections
- cross-representation adapters
- intermediate storage
- format-specific derive operation
- spelling-based proof rules
- trust classification
- raw bytes
- established T
- representation carrier
- physical geometry
- value-validity facts
- invariant-window and restoration rules
- layout names
- encoded extent
- addressable C records
- field-number / wire-type / presence / unknown-member / compatibility rules (protobuf)
- `compact_binary` (as distinct format name)
- community format
- ratification
- concrete codec conformance
- public agreement requirements
- current-shape law
- format lineage
- Derived / Admitted trust classification
- generator accepted as correct by construction
- opaque foreign realization
- trusted party / provider
- normalized plan
- requirement identity
- realization origin
- differential probes
- malformed frames
- boundary contract
- inert pointers
- proven predicates
- separately authorized qualification
- checked borrowed views
- validator
- device-correspondence fact
- provider's foreign representation
- ordinary application types
- Recasts
- representation application
- domain qualification
- storage geometry
- encoding policy
- candidate meaning
- third-party conformances
- visible-conformance search
- durability
- self-description
- semantic domains
- durable store
- stable identity
- reader-tolerance guarantees
- ephemeral cache
- channel/store compatibility checks
- published schemas
- historical shapes
- required field
- `Optional<T>`
- implicit wire default
- exact arrays
- bounded-live-length / growable owned / borrowed sequences
- home-layout facts
- case encodings
- zero storage
- home-layout realization
- stable case identities
- discriminant
- representation-sensitive API
- home-representation obligation
- authored representation obligation
- `StrictDecode<Policy, Value>`
- `ProjectingDecode<Policy, Value>`
- `PreservingDecode<Policy, Value>`
- normalized requirements
- unrecorded decoder option
- `DecodeResult<T>`
- `Relayed<T>`
- `OpaqueWireRemainder`
- producing codec identity
- codec-private envelope
- relay-ordering sidecar
- zero-copy remainder
- decoded view
- input-buffer loan
- packed-varint decoding
- borrowed scalar view
- caller-provided mutable destination
- published historical shapes
- immutable ordinary declarations
- independent lineages
- migration edges
- `FormatMigration<Lineage, Old, New>::migrate`
- version identity
- reverse and fallible conversions
- semantic domain
- unknown eras
- negotiate (decode policy)
- wildcard (exhaustive matching)
- migration route
- migration evidence
- generators (schema traversal)
- persistent fields
- atomic snapshots
- policy discriminator
- stable-identity order
- canonical minimal unsigned LEB128
- zigzag
- length-delimited nested records
- sub-region end
- bounds-checked reads
- strict decoder
- `Invalid` failure verdict
- noncanonical encoding
- consumed-byte count
- `Sound`
- destination carrier
- declared field domain
- compiler-admitted
- unverified generated realization

CHAPTER COMPLETE
