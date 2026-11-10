# Platform services

Source: `wiki/spec/build/platform_services.md` lines 1–103 (read).

## Rules

| rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane |
| --- | --- | --- | --- | --- | --- |
| PLATFORM_SERVICES1 | "The library has no compiler privilege: this is ordinary package structure under [provider selection](provider_selection.md), where a selected provider covers its complete schema." | Platform services | Platform service libraries must be ordinary packages without compiler privilege; the selected provider must cover its complete schema | not stated | 9 |
| PLATFORM_SERVICES2 | "It is shaped for the ideal target, Cathedral, and is identical on every target." | Three layers | The portable service API shape must be identical on every target and match the Cathedral ideal shape | not stated | 9 |
| PLATFORM_SERVICES3 | "The only API applications use (for example `Filesystem`)." | Three layers | Application source must use only the portable service API, not bypass it | not stated | 9 |
| PLATFORM_SERVICES4 | "One boundary trait per operating-system family (for example `PosixFilesystemHost`, `WindowsFilesystemHost`, later `CathedralFilesystemHost`)." | Three layers | Raw service boundaries must be exactly one trait per OS family | not stated | 9 |
| PLATFORM_SERVICES5 | "Each declares exactly what that OS provides, and the provider bound on that target covers all of it." | Three layers | Each raw trait's declared operations must be fully covered by the provider bound on that target | not stated | 9 |
| PLATFORM_SERVICES6 | "A raw trait never mixes families, so no provider needs stub or sentinel rows to cover operations its OS lacks." | Three layers | A raw trait must not mix OS families; providers must not rely on stubs or sentinels for missing OS operations | not stated | 9 |
| PLATFORM_SERVICES7 | "Raw handle types are per family (`PosixDescriptor`, `WindowsHandle`) and never appear in the portable API." | Three layers | Family-specific raw handle types must not appear in the portable API surface | not stated | 1 |
| PLATFORM_SERVICES8 | "Operations that exist on only one family are reachable only through that family's raw trait." | Three layers | Family-exclusive operations must be reachable only via that family's raw trait, not via the portable API | not stated | 3 |
| PLATFORM_SERVICES9 | "Build selects them through explicit target-keyed provider rows;" | Three layers | Target implementations must be selected only by explicit target-keyed provider rows | not stated | 9 |
| PLATFORM_SERVICES10 | "neither the folder name nor an OS-prefixed machine declaration supplies selection." | Three layers | Folder names and OS-prefixed machine declarations must not determine implementation selection | not stated | 9 |
| PLATFORM_SERVICES11 | "All translation between the portable shape and the OS shape happens here." | Three layers | Portable-to-OS translation must occur only in target implementations, not elsewhere | not stated | 9 |
| PLATFORM_SERVICES12 | "An application calls only the exact `PortableService` requirement application." | Provider-closure transport | Application calls must target only the exact portable-service requirement application | at a call | 3 |
| PLATFORM_SERVICES13 | "The selected target implementation's checked provider closure retains every exact raw-family requirement call and selected raw provider it needs." | Provider-closure transport | The checked provider closure for the selected target implementation must retain every needed exact raw-family requirement call and its selected raw provider | not stated | 9 |
| PLATFORM_SERVICES14 | "Installing the portable slot pins that complete transitive closure to the portable provider era;" | Provider-closure transport | Installing the portable slot must pin the full transitive closure to the portable provider era | not stated | 9 |
| PLATFORM_SERVICES15 | "changing a raw provider, its contract, or its selected target row changes the closure and requires fresh admission." | Provider-closure transport | Any change to a raw provider, its contract, or its selected target row must change the closure and require fresh admission | not stated | 9 |
| PLATFORM_SERVICES16 | "Artifact/cache identity, cycle detection, replacement checks, review, and authority evidence all include those transitive dependencies." | Provider-closure transport | Artifact and cache identity, cycle detection, replacement checks, review, and authority evidence must include those transitive closure dependencies | not stated | 9 |
| PLATFORM_SERVICES17 | "The closure dependency is not a source field in portable data, a target-varying portable type argument, or an implicitly manufactured routing value." | Provider-closure transport | Closure dependency must not appear as portable source data, a target-varying portable type argument, or an implicitly manufactured routing value | not stated | 9 |
| PLATFORM_SERVICES18 | "It is static provider-plan structure beneath the one portable slot." | Provider-closure transport | Closure dependency must be static provider-plan structure under the single portable slot | not stated | 9 |
| PLATFORM_SERVICES19 | "Entered portable calls may invoke only the raw rows retained in that checked closure, under their declared reach and physical-authority evidence." | Provider-closure transport | Calls entered through the portable facade may invoke only raw rows in the checked closure, within declared reach and physical-authority evidence | at a call | 3 |
| PLATFORM_SERVICES20 | "No provider name, target default, or family spelling grants additional calls." | Provider-closure transport | Provider names, target defaults, and family spellings must not authorize calls beyond the checked closure | at a call | 3 |
| PLATFORM_SERVICES21 | "Code that genuinely needs a separately selected raw-family role names a distinct exact requirement application or wrapper trait and calls it statically." | Provider-closure transport | A separately selected raw-family role must use a distinct exact requirement application or wrapper trait and static calls | at a call | 3 |
| PLATFORM_SERVICES22 | "Provider closure transport cannot expose private raw slots to application source, hide an ambient service, or weaken authority and review checks." | Provider-closure transport | Provider closure transport must not expose private raw slots to applications, hide ambient services, or weaken authority and review checks | not stated | SHARED |
| PLATFORM_SERVICES23 | "Consequently the portable declaration and all application-visible data remain identical on every target." | Provider-closure transport | Portable declarations and all application-visible data must be identical on every target | not stated | 1 |
| PLATFORM_SERVICES24 | "Only provider planning chooses the applicable raw family." | Provider-closure transport | The applicable raw family must be chosen only by provider planning | not stated | 9 |
| PLATFORM_SERVICES25 | "Unsupported or missing target closure rejects rather than adding a sentinel provider, demanding every family's binding, or changing the portable carrier." | Provider-closure transport | Unsupported or missing target closure must reject compilation rather than add a sentinel provider, demand every family's binding, or change the portable carrier | not stated | 9 |
| PLATFORM_SERVICES26 | "Raw requirements explicitly declare the capability identities their operations reach." | Provider-closure transport | Raw requirements must explicitly declare the capability identities their operations reach | not stated | 3 |
| PLATFORM_SERVICES27 | "Filesystem reads, writes, metadata queries/mutations, directory enumeration and namespace mutations use the corresponding core service identities;" | Provider-closure transport | Filesystem operations of those kinds must use the corresponding core service identities | not stated | 3 |
| PLATFORM_SERVICES28 | "a multi-purpose operation declares their conservative union." | Provider-closure transport | Multi-purpose operations must declare the conservative union of the relevant service identities | not stated | 3 |
| PLATFORM_SERVICES29 | "Platform family names are provider-selection identities, not automatically added service effects." | Provider-closure transport | Platform family names must count only as provider-selection identities and must not automatically add service reach effects | not stated | 3 |
| PLATFORM_SERVICES30 | "Pure error translation or arithmetic does not acquire service reach merely because its implementation is selected." | Provider-closure transport | Pure error translation or arithmetic must not gain service reach solely because its implementation is selected | not stated | 3 |
| PLATFORM_SERVICES31 | "Checked providers must fit the declared rows;" | Provider-closure transport | Checked providers must match their declared provider rows | not stated | 9 |
| PLATFORM_SERVICES32 | "foreign supply and exercised physical authority remain separately audited." | Provider-closure transport | Foreign supply and exercised physical authority must remain subject to separate audit | not stated | 8 |
| PLATFORM_SERVICES33 | "grouping an operation under a filesystem interface does not turn it into content access." | Provider-closure transport | Placing an operation under a filesystem interface must not by itself classify it as content access; permissions classifications still govern | not stated | 3 |
| PLATFORM_SERVICES34 | "Handles: the zero value means absent." | Zero-initialized portable shapes | Portable handles must treat zero as absent | not stated | 1 |
| PLATFORM_SERVICES35 | "There are no sentinel values such as `-1` in the portable API." | Zero-initialized portable shapes | The portable API must not use sentinel values such as `-1` for handles | not stated | 1 |
| PLATFORM_SERVICES36 | "Inputs: a zero-initialized options or mode value means the default behavior, so the common call passes zeroed storage." | Zero-initialized portable shapes | Zero-initialized options or mode inputs must mean default behavior | not stated | 1 |
| PLATFORM_SERVICES37 | "Results: the zero case of a result is the failure case, so untouched result storage fails closed." | Zero-initialized portable shapes | The zero variant of a result type must be the failure case so uninitialized result storage fails closed | not stated | 1 |
| PLATFORM_SERVICES38 | "an operation a target cannot perform returns the portable error kind `Unsupported`, which is part of the portable contract." | Zero-initialized portable shapes | Operations a target cannot perform must return the portable `Unsupported` error kind per the portable contract | at normal return | 1 |
| PLATFORM_SERVICES39 | "It is never produced by a raw-service stub." | Zero-initialized portable shapes | The `Unsupported` error kind must not be produced by a raw-service stub | at normal return | 9 |
| PLATFORM_SERVICES40 | "native handle; zero is absent" | Filesystem mapping | On Cathedral, native file handles in portable storage must treat zero as absent | not stated | 1 |
| PLATFORM_SERVICES41 | "`ReadDir` zero means not started; the target implementation owns the OS cursor" | Filesystem mapping | Portable `ReadDir` zero must mean enumeration not started; the target implementation must own the OS cursor | not stated | 6 |

## Ambiguous sentences

| Sentence | Reading A | Reading B |
| --- | --- | --- |
| "The only API applications use (for example `Filesystem`)." | Application source must be rejected if it calls raw-family traits or other non-portable entry points. | This states design intent for the portable layer; whether the compiler rejects raw API use in application code is not stated here. |
| "Each declares exactly what that OS provides, and the provider bound on that target covers all of it." | The provider must implement every operation listed on the raw trait for that target or compilation fails. | The raw trait lists only what the OS provides; a provider may be incomplete if the trait itself omits operations the OS lacks. |
| "One implementation may serve several compatible targets." | Provider selection may bind one implementation row to multiple target keys when they are compatible. | Several targets may share code at the source level while still requiring distinct provider rows per target. |
| "Installing the portable slot pins that complete transitive closure to the portable provider era;" | After install, the closure is fixed until admission is repeated; no incremental edits without fresh admission. | Install records an era label; whether later builds may reuse partial closure without full re-admission is not stated. |
| "Entered portable calls may invoke only the raw rows retained in that checked closure, under their declared reach and physical-authority evidence." | Static checking must prove every raw call from portable code is in the retained closure with valid reach and authority. | Portable entry is dynamically limited to retained rows, but the sentence does not state whether all such calls must be provable at compile time. |
| "Unsupported or missing target closure rejects rather than adding a sentinel provider, demanding every family's binding, or changing the portable carrier." | Missing closure is a hard compile-time reject with no fallback provider or portable shape change. | "Rejects" might mean admission or provider review rejects while leaving another build path open; not stated. |
| "a multi-purpose operation declares their conservative union." | The declared reach must be a superset sufficient for every effect the operation might exercise. | "Conservative union" might allow a smaller declared set if runtime paths never exercise some members; criteria not stated. |
| "Results: the zero case of a result is the failure case, so untouched result storage fails closed." | Zero-initialized result storage must be treated as failure without implying success was proved. | "Fails closed" might mean runtime behavior only; whether the type system distinguishes initialized success is not stated. |
| "It is never produced by a raw-service stub." | Raw stubs must not map missing OS support to `Unsupported`; only genuine portable unsupported paths may return it. | Stubs must not exist at all per earlier rules; this sentence might only forbid stub-generated `Unsupported` if stubs appear elsewhere. |
| "On Cathedral the portable shape is the native shape." | Portable types and layouts on Cathedral must match native OS types with no translation layer. | Portable API types are conceptually the same; target implementations might still perform identity translation at compile time. |

## Terms used without definition in this chapter

- library (platform service library)
- portable API
- ideal target
- Cathedral
- operating-system family
- boundary trait
- provider (bound provider, raw provider, sentinel provider)
- complete schema
- raw per-OS services
- `PosixFilesystemHost` / `WindowsFilesystemHost` / `CathedralFilesystemHost`
- raw handle types (`PosixDescriptor`, `WindowsHandle`)
- target implementations
- `source/library/std/targets/<target>/`
- target-keyed provider rows
- OS-prefixed machine declaration
- provider-closure transport
- `PortableService`
- requirement application
- checked provider closure
- portable slot
- portable provider era
- fresh admission
- artifact/cache identity
- cycle detection
- replacement checks
- review
- authority evidence
- transitive dependencies
- portable data
- target-varying portable type argument
- routing value
- provider-plan structure
- reach
- physical-authority evidence
- raw-family role
- wrapper trait
- private raw slots
- ambient service
- portable declaration
- application-visible data
- portable carrier
- capability identities
- core service identities
- conservative union
- platform family names
- provider-selection identities
- service effects / service reach
- pure error translation
- declared rows
- foreign supply
- exercised physical authority
- filesystem authority / permissions classifications
- content access
- ZII-shaped / zero-initialized portable shapes
- sentinel values
- options or mode value
- portable error kind `Unsupported`
- raw-service stub
- translation cost
- `File` storage / descriptor / `HANDLE` / `INVALID_HANDLE_VALUE`
- `OpenOptions`
- process umask
- `ErrorKind`
- `errno` / Win32 error
- `ReadDir`
- OS cursor
- provider selection (linked chapter)
- filesystem mapping table targets (Linux, macOS, Windows)

CHAPTER COMPLETE
