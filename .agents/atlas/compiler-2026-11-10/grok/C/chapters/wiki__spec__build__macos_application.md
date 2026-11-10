# macOS application publication

Source: `wiki/spec/build/macos_application.md` lines 1–120 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
MACOS_APPLICATION1 | "GUI intent, spelled `builder.subsystem = Subsystem::Gui`, is the opt-in; there is no second bundle flag." | Intent and deliverable | macOS GUI bundling must depend only on `builder.subsystem = Subsystem::Gui`; no additional bundle flag may gate bundling | not stated | 9
MACOS_APPLICATION2 | "Complete selected macOS GUI application output requires a bundle." | Intent and deliverable | When macOS GUI application output is selected and complete, deliverable must be a bundle (`.app`), not only a flat executable | not stated | 9
MACOS_APPLICATION3 | "Do not publish a redundant flat copy beside the bundle." | Intent and deliverable | Publication must not emit a flat executable alongside the bundle for the same macOS GUI application | not stated | 9
MACOS_APPLICATION4 | "Terminal-only and retained-native stops remain before packaging." | Intent and deliverable | Terminal-only and retained-native pipeline stops must occur before packaging or publication assembly | not stated | 9
MACOS_APPLICATION5 | "An unused macOS entry imposes no macOS publication on other selected targets; each target has its own result." | Intent and deliverable | macOS publication obligations must not apply to non-macOS selected targets merely because macOS is listed unused | not stated | 9
MACOS_APPLICATION6 | "Portable console/GUI intent remains semantic data through Build evaluation, not PE integers 3/2." | Intent and deliverable | Console versus GUI intent must be represented as Build semantic data, not as raw PE subsystem integers 3 or 2 | not stated | 9
MACOS_APPLICATION7 | "Raw PE settings belong to PE-specific configuration; `Unspecified(2)` cannot acquire GUI meaning." | Intent and deliverable | PE-specific raw settings must not confer GUI meaning; `Unspecified(2)` must never be interpreted as GUI intent | not stated | 9
MACOS_APPLICATION8 | "One validated application name supplies executable leaf and `.app` basename." | Names and metadata ownership | One validated application name must determine both executable leaf filename and `.app` bundle basename | not stated | 9
MACOS_APPLICATION9 | "Reject invalid path components; do not use independent sanitizers or derive names from source folders." | Names and metadata ownership | Invalid path components in the application name must be rejected; names must not come from independent sanitizers or source-folder derivation | not stated | 9
MACOS_APPLICATION10 | "The universal `omega-program` filename is replaced." | Names and metadata ownership | Default `omega-program` executable naming must be replaced by the validated application name | not stated | 9
MACOS_APPLICATION11 | "The ordinary Build field is `builder.identifier`, a `&[u8]` member validated separately from the filename as nonempty ASCII in `A-Z a-z 0-9 . -` with no empty `.-separated segment;" | Names and metadata ownership | `builder.identifier` must be validated as nonempty ASCII in `A-Z a-z 0-9 . -` with no empty dot-separated segment, separately from filename validation | not stated | 9
MACOS_APPLICATION12 | "Require the identifier before signed macOS GUI image emission, including retained-native output." | Names and metadata ownership | `builder.identifier` must be present before emitting a signed macOS GUI image, including retained-native output | not stated | 9
MACOS_APPLICATION13 | "Absence is an early build/realization configuration error, not source-semantic rejection." | Names and metadata ownership | Missing required identifier for signed macOS GUI emission must be reported as build or realization configuration error, not as source-semantic rejection | not stated | 9
MACOS_APPLICATION14 | "Merely declaring macOS does not require it for console output or other selected targets." | Names and metadata ownership | macOS target selection alone must not require `builder.identifier` for console output or for other selected targets | not stated | 9
MACOS_APPLICATION15 | "Changing it requires image finalization/signing, not necessarily parsing, proof, optimization, or instruction generation." | Names and metadata ownership | Identifier change must trigger image finalization or signing only; parsing, proof, optimization, or instruction generation need not rerun solely for identifier change | not stated | 9
MACOS_APPLICATION16 | "Renaming a GUI executable alone does not change it." | Names and metadata ownership | Executable leaf rename alone must not change the bundle identifier used for signing and `CFBundleIdentifier` | not stated | 9
MACOS_APPLICATION17 | "Console output honors an explicit identifier, otherwise using the validated executable leaf as its ad-hoc label;" | Names and metadata ownership | macOS console output must use explicit `builder.identifier` when present, otherwise the validated executable leaf as ad-hoc label | not stated | 9
MACOS_APPLICATION18 | "The plist uses package type `APPL`, executable leaf as `CFBundleExecutable`, application name as `CFBundleName`, and retained realization identity as `CFBundleIdentifier`." | Assembly | `Info.plist` must set `CFBundlePackageType` to `APPL`, `CFBundleExecutable` to executable leaf, `CFBundleName` to application name, and `CFBundleIdentifier` to retained realization identity | not stated | 9
MACOS_APPLICATION19 | "Encoding, key order, and escaping are fixed, without timestamps or ambient facts." | Assembly | Plist encoding, key order, and escaping must be deterministic and fixed; timestamps and ambient facts must not appear | not stated | 9
MACOS_APPLICATION20 | "Publication cannot replace the signed identifier." | Assembly | Publication metadata changes must not replace or override the identifier already bound in signed realization output | not stated | 9
MACOS_APPLICATION21 | "Carry artifact-bound realization inputs and publication metadata in the envelope or a strongly bound companion." | Assembly | Cross-invocation publication must carry realization inputs and publication metadata in the envelope or a strongly bound companion | not stated | 9
MACOS_APPLICATION22 | "Bind the request to that artifact and metadata; conflicts reject." | Assembly | Publication request must bind to the carried artifact and metadata; conflicting bindings must reject | not stated | 9
MACOS_APPLICATION23 | "Never recover missing inputs from the original frontend or live project." | Assembly | Missing publication or realization inputs must not be recovered from frontend or live project state | not stated | 9
MACOS_APPLICATION24 | "Stage and validate the complete package before success, then replay the same contents at the final destination." | Assembly | Complete package must be staged and validated before success; final destination must receive the same validated contents | not stated | 9
MACOS_APPLICATION25 | "Partial assembly is not a deliverable." | Assembly | Partially assembled macOS application packages must not be reported or exposed as successful deliverables | not stated | 9
MACOS_APPLICATION26 | "The implementation must define failure cleanup and replacement behavior." | Assembly | Failure during assembly must have defined cleanup and replacement behavior | not stated | 9
MACOS_APPLICATION27 | "Missing, extra, substituted, or partial contents reject." | Validation and reports | Package validation must reject missing, extra, substituted, or partial executable or plist bytes or directory shape | not stated | 9
MACOS_APPLICATION28 | "Independently compare the plist identifier, retained realization identity, and identifier in the actual published executable signature." | Validation and reports | Validation must compare plist `CFBundleIdentifier`, retained realization identity, and executable signature identifier independently; matching producer assertions alone is insufficient | not stated | 9
MACOS_APPLICATION29 | "Matching producer assertions is insufficient." | Validation and reports | Package acceptance must not rely only on matching internal producer assertions; independent cross-check is required | not stated | 9
MACOS_APPLICATION30 | "Validate their structural relationship and installed bytes; they are not independently trusted paths." | Validation and reports | Checked package root and inner executable path accessors must be validated for structural relationship and installed bytes; neither path is trusted without validation | not stated | 9
MACOS_APPLICATION31 | "Consumers use accessors, not hardcoded names or directory walking." | Validation and reports | External consumers must obtain publication paths through accessors, not hardcoded bundle layout names or directory walking | not stated | 9
MACOS_APPLICATION32 | "Invalid publication exposes neither a successful package nor its executable path." | Validation and reports | Failed or invalid publication must not expose a successful package root or inner executable path | not stated | 9
MACOS_APPLICATION33 | "Executable ad-hoc signing does not sign the external plist." | Validation and reports | Ad-hoc signing on the executable must not be treated as signing `Info.plist` | not stated | 9
MACOS_APPLICATION34 | "Preserve execution-required ad-hoc signing; credential-backed signing, notarization, and installation remain separate operations with separate authority." | Validation and reports | Execution-required ad-hoc signing must be preserved; credential signing, notarization, and installation must remain separate operations with separate authority | not stated | 9
MACOS_APPLICATION35 | "A future external assembler may supply contents, which the publisher checks as untrusted input;" | Validation and reports | Contents from an external assembler must be validated as untrusted input by the publisher | not stated | 9
MACOS_APPLICATION36 | "the first publisher is internal and requires no general extension mechanism." | Validation and reports | Initial publication path must be internal without requiring a general external-assembler extension mechanism | not stated | 9
MACOS_APPLICATION37 | "V1 includes executable and plist; no application resources." | Bounded acceptance | Version-one macOS GUI bundle validation scope includes only executable and plist; application resources are out of scope | not stated | 9
MACOS_APPLICATION38 | "Do not silently change process directory or claim Finder coverage for that sample." | Bounded acceptance | For deferred `image_viewer`, publication must not silently change process working directory or claim Finder coverage without bundle-relative resource inclusion | not stated | 9
MACOS_APPLICATION39 | "Report unavailable macOS runtime coverage explicitly." | Bounded acceptance | When macOS runtime validation coverage is unavailable, reporting must state that explicitly | not stated | 9
MACOS_APPLICATION40 | "Concrete macOS assembly belongs to post-compilation product publication, not Psi, instruction lowering, a plugin registry, or a generic packaging framework." | Intent and deliverable | macOS bundle assembly must occur only in post-compilation product publication, not in Psi, lowering, plugin registry, or generic packaging | not stated | 9
MACOS_APPLICATION41 | "`Build::application(builder, ...)` supplies the name." | Intent and deliverable | Application naming for publication must come from `Build::application(builder, ...)` | not stated | 9
MACOS_APPLICATION42 | "A package record covers exact executable/plist bytes and directory shape." | Validation and reports | Package validation record must cover exact executable and plist bytes and directory shape | not stated | 9
MACOS_APPLICATION43 | "Expose the checked package root separately from `checked_native_executable_path()`, which returns the inner `Contents/MacOS/window-app`." | Validation and reports | API must expose checked package root separately from `checked_native_executable_path()` returning inner `Contents/MacOS/<executable-leaf>` | not stated | 9
MACOS_APPLICATION44 | "Copying resources alone does not repair it." | Bounded acceptance | Adding resources alone must not be treated as fixing working-directory-relative BMP access for deferred `image_viewer` | not stated | 9
MACOS_APPLICATION45 | "Source inspection is not a runtime pass." | Bounded acceptance | Procedural sample acceptance on macOS must not substitute source inspection for runtime validation where runtime validation is required | not stated | 9

Deliverable table rows (lines 17–22) as explicit output obligations when the corresponding application kind is selected:

MACOS_APPLICATION46 | "Windows GUI | Flat executable with GUI PE subsystem." | Intent and deliverable | Selected Windows GUI application output must be a flat executable with GUI PE subsystem | not stated | 9
MACOS_APPLICATION47 | "macOS GUI | One `.app` containing the executable." | Intent and deliverable | Selected macOS GUI application output must be one `.app` bundle containing the executable | not stated | 9
MACOS_APPLICATION48 | "Linux GUI | Flat ELF." | Intent and deliverable | Selected Linux GUI application output must be a flat ELF | not stated | 9
MACOS_APPLICATION49 | "macOS console | Flat Mach-O." | Intent and deliverable | Selected macOS console application output must be a flat Mach-O, not a bundle | not stated | 9

Assembly tree (lines 64–69) as structural obligation:

MACOS_APPLICATION50 | "For `window-app`, the complete v1 tree is: `window-app.app/Contents/Info.plist` and `Contents/MacOS/window-app`." | Assembly | v1 macOS GUI bundle directory shape must match the stated `Contents/Info.plist` and `Contents/MacOS/<executable-leaf>` layout | not stated | 9

Line 52-54 about identifier entering native realization - rule:

MACOS_APPLICATION51 | "The identifier enters native realization inputs, signed bytes, and artifact identity." | Names and metadata ownership | Validated `builder.identifier` must flow into native realization inputs, signed bytes, and artifact identity | not stated | 9

Line 55-57 display metadata:

MACOS_APPLICATION52 | "Display-only metadata and destination are publication inputs." | Names and metadata ownership | Display metadata and destination must be treated as publication inputs distinct from signed realization identity | not stated | 9

Line 58 fallback properties - might be informational not enforcement... "that fallback is neither globally unique nor rename-stable" - documents properties, not necessarily a check.

Line 80: "Display changes change the package, not unchanged native bytes." - rule about what changes on display-only edits

MACOS_APPLICATION53 | "Display changes change the package, not unchanged native bytes." | Assembly | Display-only metadata changes must alter the package without requiring unchanged native executable bytes to change | not stated | 9

Line 100-101 package consistency:

MACOS_APPLICATION54 | "Package consistency is not a distribution signature or authenticity claim." | Validation and reports | Package consistency validation must not be presented or treated as distribution signature or authenticity claim | not stated | 9

Line 109-110 procedural validation - obligation on test/CI not compiler per se but "Validate procedural... on macOS after adding identifiers" - still a rule someone must enforce

MACOS_APPLICATION55 | "Validate procedural `window_app`, `window_demo`, and `windowed_calculator` on macOS after adding identifiers and reported-path consumers." | Bounded acceptance | `window_app`, `window_demo`, and `windowed_calculator` must be validated procedurally on macOS after identifiers and reported-path consumers exist | not stated | 9

Line 111-112 image_viewer deferred:

MACOS_APPLICATION56 | "`image_viewer` is deferred: working-directory-relative BMP access needs inclusion and explicit bundle-relative lookup." | Bounded acceptance | `image_viewer` must remain deferred until bundle-relative resource inclusion and lookup exist for working-directory-relative BMP access | not stated | 9

Line 116-119 controls - descriptive of test coverage, not individual compiler rules - could skip or note as meta

"Controls cover deterministic assembly; identifier requiredness; ..." - describes what tests should cover, not a single enforceable rule unless we extract each as obligation on the validation harness.

I'll include line 116-118 as one rule about what validation must cover if interpreted as requirement:

Actually re-read STREAM C: "Extract every rule a compiler must enforce". The controls paragraph might be about acceptance criteria for the implementation project, not compiler rules. I'll include MACOS_APPLICATION39 for runtime coverage and the procedural validation rules.

Line 4-5: "This is the required contract" - meta
Line 5-7: bundling is delivery not GUI authority - architectural, lane 9 maybe not compiler check

EFI line 31-32: "EFI retains its distinct entry, storage, and target-owned environment contract, not another environment switch or a hosted console/GUI classification." - rule that EFI must not be folded into console/GUI classification

MACOS_APPLICATION57 | "EFI retains its distinct entry, storage, and target-owned environment contract, not another environment switch or a hosted console/GUI classification." | Intent and deliverable | EFI target must keep separate entry, storage, and environment contract; must not be classified via hosted console or GUI switches | not stated | 9

An explicit authored identifier line 40:

MACOS_APPLICATION58 | "An explicit authored identifier supplies GUI CodeDirectory signing identity and `CFBundleIdentifier`." | Names and metadata ownership | GUI CodeDirectory signing identity and `CFBundleIdentifier` must come from the explicit authored identifier | not stated | 9

Line 43-44: "it proves neither globally unique ownership nor authenticity" - negative capability, compiler shouldn't claim uniqueness - maybe not enforcement

Line 48-49: "Terminal production needs none; a later consumer may supply it under its authority." - permissive rule

MACOS_APPLICATION59 | "Terminal production needs none;" | Names and metadata ownership | Terminal-only macOS production must not require `builder.identifier` | not stated | 9

I should also handle line 106-107 parenthetical on An explicit identifier - already covered.

Let me refine MACOS_APPLICATION50 to quote exact sentences from the doc.

For the assembly tree, the spec says:
```
For `window-app`, the complete v1 tree is:
...
window-app.app/
  Contents/
    Info.plist
    MacOS/window-app
```

Quote: "For `window-app`, the complete v1 tree is:" and the structure - I'll use the sentence before the code block.

Actually rule format wants exact sentence quoted - for the tree, the intro sentence is "For `window-app`, the complete v1 tree is:" - the structure is in a code block not a sentence. I could add a rule from line 84 "The implementation must define failure cleanup and replacement behavior." already have.

Double-check I didn't miss rejects from line 88-91.

"Display-only metadata and destination are publication inputs." - have 52

Line 77-78: "Realization and publication may run in separate invocations/machines." - not a must-check rule

I think I have good coverage. Let me add ambiguous sentences and undefined terms.

Ambiguous:
- "Complete selected macOS GUI application output requires a bundle." - Complete might mean fully built vs successfully selected
- "Terminal-only and retained-native stops remain before packaging." - what counts as terminal-only stop
- "early build/realization configuration error, not source-semantic rejection" - boundary of what is source vs config
- "retained-native output" vs retained-native stops
- "strongly bound companion"
- "Stage and validate the complete package before success"
- "Partial assembly is not a deliverable" vs interim artifacts during staging
- "Independently compare" - what comparison means on mismatch (reject vs warn)
- "Invalid publication exposes neither" - expose to whom/API
- "The first publisher is internal"
- "Source inspection is not a runtime pass" - what validates procedurally means
- "Report unavailable macOS runtime coverage explicitly"

Terms undefined:
- MACOS-APPLICATION-PUBLICATION
- Psi
- instruction lowering
- plugin registry
- generic packaging framework
- Build::application
- builder.subsystem
- Subsystem::Gui
- bundle
- retained-native stops
- Terminal-only stops
- PE subsystem
- Unspecified(2)
- EFI
- builder.identifier
- CodeDirectory
- CFBundleIdentifier
- image emission
- realization
- publication
- envelope
- companion
- frontend
- Info.plist
- CFBundleExecutable
- CFBundleName
- CFBundlePackageType
- ad-hoc signing
- notarization
- checked_native_executable_path
- package record
- window-app / window_app
- procedural validation
- Finder coverage
- flat v1 digests
- both checked paths

User asked to write the chapter to the path but INSTRUCTIONS say do not create files - return in message only. Good.

Fix table row MACOS_APPLICATION50 quote to be exact from source.

Also verify every rule in line range - line 121 is empty, range is 1-120 complete.

CHAPTER COMPLETE if every rule finished - I believe I've extracted all enforceable rules from 1-120.

Let me tighten the rules table - some rows might be duplicate (MACOS_APPLICATION28 and 29 both from same area).

MACOS_APPLICATION50 - use exact quote: "For `window-app`, the complete v1 tree is:" only, and plain words describe the tree from the fenced block.

I'll produce the final file content for the user.

===== FILE .agents/atlas/compiler-2026-11-10/grok/C/chapters/wiki__spec__build__macos_application.md =====

# macOS application publication

Source: `wiki/spec/build/macos_application.md` lines 1–120 (read).

## Rules

rule id | the exact sentence, quoted | the heading it is under | what must be checked, in plain words | when it must hold | lane
--- | --- | --- | --- | --- | ---
MACOS_APPLICATION1 | "Concrete macOS assembly belongs to post-compilation product publication, not Psi, instruction lowering, a plugin registry, or a generic packaging framework." | Intent and deliverable | macOS `.app` assembly must run only in post-compilation product publication, not in Psi, lowering, plugins, or generic packaging | not stated | 9
MACOS_APPLICATION2 | "`Build::application(builder, ...)` supplies the name." | Intent and deliverable | Publication naming must come from `Build::application(builder, ...)` | not stated | 9
MACOS_APPLICATION3 | "GUI intent, spelled `builder.subsystem = Subsystem::Gui`, is the opt-in; there is no second bundle flag." | Intent and deliverable | macOS GUI bundling must hinge only on `builder.subsystem = Subsystem::Gui`; no separate bundle opt-in flag | not stated | 9
MACOS_APPLICATION4 | "Complete selected macOS GUI application output requires a bundle." | Intent and deliverable | When macOS GUI application output is selected and complete, output must be a bundle (`.app`) | not stated | 9
MACOS_APPLICATION5 | "Do not publish a redundant flat copy beside the bundle." | Intent and deliverable | Must not emit a flat executable alongside the `.app` for the same macOS GUI product | not stated | 9
MACOS_APPLICATION6 | "Terminal-only and retained-native stops remain before packaging." | Intent and deliverable | Terminal-only and retained-native pipeline stops must complete before packaging or publication | not stated | 9
MACOS_APPLICATION7 | "An unused macOS entry imposes no macOS publication on other selected targets; each target has its own result." | Intent and deliverable | Unused macOS selection must not impose macOS publication on other selected targets | not stated | 9
MACOS_APPLICATION8 | "Portable console/GUI intent remains semantic data through Build evaluation, not PE integers 3/2." | Intent and deliverable | Console versus GUI intent must stay Build semantic data, not raw PE subsystem integers 3 or 2 | not stated | 9
MACOS_APPLICATION9 | "Raw PE settings belong to PE-specific configuration; `Unspecified(2)` cannot acquire GUI meaning." | Intent and deliverable | PE-specific raw settings must not assign GUI meaning; `Unspecified(2)` must never mean GUI | not stated | 9
MACOS_APPLICATION10 | "EFI retains its distinct entry, storage, and target-owned environment contract, not another environment switch or a hosted console/GUI classification." | Intent and deliverable | EFI must keep its own entry, storage, and environment contract; must not use hosted console/GUI classification switches | not stated | 9
MACOS_APPLICATION11 | "One validated application name supplies executable leaf and `.app` basename." | Names and metadata ownership | One validated application name must set both executable leaf and `.app` basename | not stated | 9
MACOS_APPLICATION12 | "Reject invalid path components; do not use independent sanitizers or derive names from source folders." | Names and metadata ownership | Invalid path components in the application name must fail; no independent sanitizers or source-folder-derived names | not stated | 9
MACOS_APPLICATION13 | "The universal `omega-program` filename is replaced." | Names and metadata ownership | Default `omega-program` leaf name must be replaced by the validated application name | not stated | 9
MACOS_APPLICATION14 | "An explicit authored identifier supplies GUI CodeDirectory signing identity and `CFBundleIdentifier`." | Names and metadata ownership | GUI signing identity and `CFBundleIdentifier` must come from the explicit authored identifier | not stated | 9
MACOS_APPLICATION15 | "The ordinary Build field is `builder.identifier`, a `&[u8]` member validated separately from the filename as nonempty ASCII in `A-Z a-z 0-9 . -` with no empty `.-separated segment;" | Names and metadata ownership | `builder.identifier` must be validated separately from the filename as nonempty ASCII `A-Z a-z 0-9 . -` with no empty dot-separated segment | not stated | 9
MACOS_APPLICATION16 | "Require the identifier before signed macOS GUI image emission, including retained-native output." | Names and metadata ownership | `builder.identifier` must be present before signed macOS GUI image emission, including retained-native output | not stated | 9
MACOS_APPLICATION17 | "Absence is an early build/realization configuration error, not source-semantic rejection." | Names and metadata ownership | Missing required identifier for signed macOS GUI must be a build/realization configuration error, not source-semantic rejection | not stated | 9
MACOS_APPLICATION18 | "Terminal production needs none;" | Names and metadata ownership | Terminal-only macOS production must not require `builder.identifier` | not stated | 9
MACOS_APPLICATION19 | "Merely declaring macOS does not require it for console output or other selected targets." | Names and metadata ownership | macOS target declaration alone must not require identifier for console or other selected targets | not stated | 9
MACOS_APPLICATION20 | "The identifier enters native realization inputs, signed bytes, and artifact identity." | Names and metadata ownership | Identifier must flow into native realization inputs, signed bytes, and artifact identity | not stated | 9
MACOS_APPLICATION21 | "Changing it requires image finalization/signing, not necessarily parsing, proof, optimization, or instruction generation." | Names and metadata ownership | Identifier change must require image finalization or signing only, not necessarily re-parse, proof, optimize, or codegen | not stated | 9
MACOS_APPLICATION22 | "Renaming a GUI executable alone does not change it." | Names and metadata ownership | Executable leaf rename alone must not change bundle signing identifier | not stated | 9
MACOS_APPLICATION23 | "Display-only metadata and destination are publication inputs." | Names and metadata ownership | Display metadata and destination must be publication inputs, separate from signed realization identity | not stated | 9
MACOS_APPLICATION24 | "Console output honors an explicit identifier, otherwise using the validated executable leaf as its ad-hoc label;" | Names and metadata ownership | macOS console output must use explicit identifier when set, else validated executable leaf as ad-hoc label | not stated | 9
MACOS_APPLICATION25 | "For `window-app`, the complete v1 tree is:" | Assembly | v1 bundle directory shape must be `.app/Contents/Info.plist` and `Contents/MacOS/<executable-leaf>` as in the spec tree | not stated | 9
MACOS_APPLICATION26 | "The plist uses package type `APPL`, executable leaf as `CFBundleExecutable`, application name as `CFBundleName`, and retained realization identity as `CFBundleIdentifier`." | Assembly | `Info.plist` must set package type `APPL`, `CFBundleExecutable`, `CFBundleName`, and `CFBundleIdentifier` from the stated sources | not stated | 9
MACOS_APPLICATION27 | "Encoding, key order, and escaping are fixed, without timestamps or ambient facts." | Assembly | Plist encoding, key order, and escaping must be fixed and deterministic; no timestamps or ambient facts | not stated | 9
MACOS_APPLICATION28 | "Publication cannot replace the signed identifier." | Assembly | Publication must not replace or override the identifier already bound in signed realization | not stated | 9
MACOS_APPLICATION29 | "Carry artifact-bound realization inputs and publication metadata in the envelope or a strongly bound companion." | Assembly | Cross-invocation publication must carry realization inputs and metadata in envelope or strongly bound companion | not stated | 9
MACOS_APPLICATION30 | "Bind the request to that artifact and metadata; conflicts reject." | Assembly | Publication request must bind to carried artifact and metadata; conflicts must reject | not stated | 9
MACOS_APPLICATION31 | "Never recover missing inputs from the original frontend or live project." | Assembly | Missing inputs must not be recovered from frontend or live project state | not stated | 9
MACOS_APPLICATION32 | "Display changes change the package, not unchanged native bytes." | Assembly | Display-only changes must change the package without requiring native executable bytes to change | not stated | 9
MACOS_APPLICATION33 | "Stage and validate the complete package before success, then replay the same contents at the final destination." | Assembly | Complete package must be staged and validated before success; final destination gets the same validated contents | not stated | 9
MACOS_APPLICATION34 | "Partial assembly is not a deliverable." | Assembly | Partial packages must not count as successful deliverables | not stated | 9
MACOS_APPLICATION35 | "The implementation must define failure cleanup and replacement behavior." | Assembly | Assembly failures must have defined cleanup and replacement behavior | not stated | 9
MACOS_APPLICATION36 | "A package record covers exact executable/plist bytes and directory shape." | Validation and reports | Package validation must cover exact executable and plist bytes and directory shape | not stated | 9
MACOS_APPLICATION37 | "Missing, extra, substituted, or partial contents reject." | Validation and reports | Missing, extra, substituted, or partial package contents must reject | not stated | 9
MACOS_APPLICATION38 | "Independently compare the plist identifier, retained realization identity, and identifier in the actual published executable signature." | Validation and reports | Must independently compare plist identifier, retained realization identity, and executable signature identifier | not stated | 9
MACOS_APPLICATION39 | "Matching producer assertions is insufficient." | Validation and reports | Acceptance must not rely only on matching producer assertions | not stated | 9
MACOS_APPLICATION40 | "Expose the checked package root separately from `checked_native_executable_path()`, which returns the inner `Contents/MacOS/window-app`." | Validation and reports | Must expose checked package root separately from inner `Contents/MacOS/<executable-leaf>` via `checked_native_executable_executable_path()` | not stated | 9
MACOS_APPLICATION41 | "Validate their structural relationship and installed bytes; they are not independently trusted paths." | Validation and reports | Package root and inner path must be validated for structural relationship and bytes; paths are not trusted alone | not stated | 9
MACOS_APPLICATION42 | "Consumers use accessors, not hardcoded names or directory walking." | Validation and reports | Consumers must use accessors, not hardcoded layout names or directory walking | not stated | 9
MACOS_APPLICATION43 | "Invalid publication exposes neither a successful package nor its executable path." | Validation and reports | Invalid publication must not expose successful package root or executable path | not stated | 9
MACOS_APPLICATION44 | "Executable ad-hoc signing does not sign the external plist." | Validation and reports | Executable ad-hoc signing must not be treated as signing external `Info.plist` | not stated | 9
MACOS_APPLICATION45 | "Package consistency is not a distribution signature or authenticity claim." | Validation and reports | Package consistency checks must not be treated as distribution signature or authenticity claims | not stated | 9
MACOS_APPLICATION46 | "Preserve execution-required ad-hoc signing; credential-backed signing, notarization, and installation remain separate operations with separate authority." | Validation and reports | Execution-required ad-hoc signing must be preserved; credential signing, notarization, and install stay separate with separate authority | not stated | 9
MACOS_APPLICATION47 | "A future external assembler may supply contents, which the publisher checks as untrusted input;" | Validation and reports | External assembler contents must be checked as untrusted input | not stated | 9
MACOS_APPLICATION48 | "the first publisher is internal and requires no general extension mechanism." | Validation and reports | First publication path must be internal without a general external-assembler extension mechanism | not stated | 9
MACOS_APPLICATION49 | "V1 includes executable and plist; no application resources." | Bounded acceptance | V1 bundle scope is executable and plist only; no application resources in v1 validation | not stated | 9
MACOS_APPLICATION50 | "Validate procedural `window_app`, `window_demo`, and `windowed_calculator` on macOS after adding identifiers and reported-path consumers." | Bounded acceptance | Those samples must be validated procedurally on macOS after identifiers and reported-path consumers exist | not stated | 9
MACOS_APPLICATION51 | "Source inspection is not a runtime pass." | Bounded acceptance | Procedural macOS validation must not substitute source inspection for required runtime validation | not stated | 9
MACOS_APPLICATION52 | "`image_viewer` is deferred: working-directory-relative BMP access needs inclusion and explicit bundle-relative lookup." | Bounded acceptance | `image_viewer` must stay deferred until bundle-relative resource inclusion and lookup exist for cwd-relative BMP access | not stated | 9
MACOS_APPLICATION53 | "Copying resources alone does not repair it." | Bounded acceptance | Adding resources alone must not be treated as fixing deferred `image_viewer` BMP access | not stated | 9
MACOS_APPLICATION54 | "Do not silently change process directory or claim Finder coverage for that sample." | Bounded acceptance | Must not silently change process cwd or claim Finder coverage for `image_viewer` without bundle-relative fix | not stated | 9
MACOS_APPLICATION55 | "Report unavailable macOS runtime coverage explicitly." | Bounded acceptance | Unavailable macOS runtime test coverage must be reported explicitly | not stated | 9
MACOS_APPLICATION56 | "Windows GUI | Flat executable with GUI PE subsystem." | Intent and deliverable | Selected Windows GUI output must be flat executable with GUI PE subsystem | not stated | 9
MACOS_APPLICATION57 | "macOS GUI | One `.app` containing the executable." | Intent and deliverable | Selected macOS GUI output must be one `.app` containing the executable | not stated | 9
MACOS_APPLICATION58 | "Linux GUI | Flat ELF." | Intent and deliverable | Selected Linux GUI output must be flat ELF | not stated | 9
MACOS_APPLICATION59 | "macOS console | Flat Mach-O." | Intent and deliverable | Selected macOS console output must be flat Mach-O, not a bundle | not stated | 9

## Ambiguous sentences

Sentence | Reading A | Reading B
--- | --- | ---
"Complete selected macOS GUI application output requires a bundle." | "Complete" means the build produced a full GUI app for macOS; bundling is mandatory for that outcome. | "Complete" means every facet of the macOS GUI deliverable contract is satisfied; unclear whether incomplete or failed builds still must emit partial bundles. |
"Terminal-only and retained-native stops remain before packaging." | Packaging never runs until terminal-only and retained-native pipeline stages have finished successfully. | Those stops are ordering constraints on the pipeline graph; "remain before" might allow overlap if publication only reads finalized artifacts. |
"Absence is an early build/realization configuration error, not source-semantic rejection." | Missing identifier is always reported at build/realization, never as a Psi source type or proof error. | "Early" might allow the same missing identifier to surface at different phases depending on invocation; boundary between build and realization is not fixed here. |
"including retained-native output" | Retained-native macOS GUI emission has the same identifier requirement as ordinary signed GUI image emission. | "Retained-native output" might name a distinct artifact class with additional unstated identifier rules. |
"Realization and publication may run in separate invocations/machines." | Separate runs are allowed when envelope binding rules are met. | Separate runs are optional; the spec does not say whether a single invocation must also remain supported. |
"Bind the request to that artifact and metadata; conflicts reject." | Any mismatch between request metadata and carried artifact metadata must fail closed. | "Conflicts" might mean only certain classes of mismatch; unspecified whether benign display-only drift counts as conflict. |
"Stage and validate the complete package before success, then replay the same contents at the final destination." | Success is reported only after staging validation passes; final install is a byte-identical replay. | Staging might occur on a temp path while "success" is reported before final destination write; replay timing is unspecified. |
"Partial assembly is not a deliverable." | No API or report may treat partial trees as shippable products. | Partial trees may exist on disk during assembly if they are never labeled successful; whether interim paths are visible is not stated. |
"Independently compare the plist identifier, retained realization identity, and identifier in the actual published executable signature." | All three must be equal or validation rejects. | Independent comparison might mean separate checks that can fail independently; exact mismatch handling (reject vs diagnostic) is not stated. |
"Invalid publication exposes neither a successful package nor its executable path." | Failed validation clears or withholds all success-path accessors. | "Expose" might mean only public CLI output while internal logs may still mention paths. |
"The first publisher is internal and requires no general extension mechanism." | V1 need not ship a plugin ABI for external assemblers. | "First publisher" might mean only the initial release milestone, not forever; future internal refactor requirements are unclear. |
"Validate procedural `window_app`, `window_demo`, and `windowed_calculator` on macOS after adding identifiers and reported-path consumers." | Acceptance requires macOS runtime procedural runs using reported-path APIs. | "Procedural" might mean scripted build or install steps without mandating interactive GUI runtime on every CI host. |
"Source inspection is not a runtime pass." | Runtime validation cannot be waived by reading source alone. | Static review might still gate merge while runtime is best-effort when macOS hosts are absent. |
"Report unavailable macOS runtime coverage explicitly." | CI or reports must state when macOS runtime tests were skipped. | "Coverage" might mean test breadth versus host availability; what must be reported when tests run on macOS but omit GUI is not stated. |
"A flat executable can display a window: bundling is delivery, not GUI execution authority." | GUI capability does not require bundling at execution time; bundling is publication-only. | Flat executables might still be rejected as deliverables for macOS GUI even though they can run; execution vs delivery distinction for validation is unclear. |
"Display changes change the package, not unchanged native bytes." | Display-only publication input edits must reassemble package metadata without recompiling native code. | "Unchanged native bytes" might allow re-signing or re-linking when only display fields change if bytes happen to stay identical. |

## Terms used without definition in this chapter

- MACOS-APPLICATION-PUBLICATION
- Component publication
- native/installed-product boundary
- Psi
- instruction lowering
- plugin registry
- generic packaging framework
- `Build::application`
- `builder.subsystem`
- `Subsystem::Gui`
- bundle / `.app`
- flat executable
- GUI PE subsystem
- retained-native stops
- terminal-only stops
- unused macOS entry
- portable console/GUI intent
- Build evaluation
- PE integers 3/2
- `Unspecified(2)`
- EFI
- validated application name
- executable leaf
- `omega-program`
- explicit authored identifier
- CodeDirectory
- `CFBundleIdentifier`
- `builder.identifier`
- signed macOS GUI image emission
- retained-native output
- build/realization configuration error
- source-semantic rejection
- native realization inputs
- image finalization/signing
- publication inputs
- ad-hoc label
- `window-app`
- `Info.plist`
- `CFBundleExecutable`
- `CFBundleName`
- `CFBundlePackageType` / `APPL`
- retained realization identity
- envelope
- strongly bound companion
- frontend
- live project
- package record
- `checked_native_executable_path()`
- checked package root
- accessors
- ad-hoc signing
- distribution signature
- credential-backed signing
- notarization
- external assembler
- v1
- application resources
- procedural validation
- reported-path consumers
- `window_app`, `window_demo`, `windowed_calculator`
- `image_viewer`
- bundle-relative lookup
- Finder coverage
- flat v1 digests
- both checked paths
- macOS runtime coverage

CHAPTER COMPLETE
