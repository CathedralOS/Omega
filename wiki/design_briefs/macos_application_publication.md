# macOS application publication

Owner-ratified contract, 2026-09-05. This describes the required implementation,
not a claim that application-bundle publication already works.

## Current implementation and replacement

Native publication currently writes a flat executable, including for macOS GUI
programs. `std/macos_gui.omg` requests application activation; publication does
not supply foreground behavior. A flat executable can display a window, so
bundle assembly is an application-delivery feature, not a prerequisite for GUI
execution.

The retained optional bundle receipt has no production writer. It describes a
second executable installation, not a complete application package. Retire that
slot, its destination variant, pair matcher, bundle-name/path helpers, and
bundle-specific tests and module notes. Preserve flat installation validation,
output-kind consistency, native-artifact validation, and their negative tests.
The pair matcher's early return on an absent bundle does not make the preceding
flat check or the enclosing report validator a no-op.

Keep the flat installation digest's existing fixed `0` destination byte under
its v1 domain when removing the enum, and pin byte-identical flat digests. It is
the fixed v1 flat-destination tag, not a promise of future variants. The new
whole-package record must not reinterpret the old executable-copy receipt.
Keep the report's read-only compilation-root accessor: it retains the root of
the reported compilation and has consumers outside bundle publication. Remove
only its obsolete bundle-specific rationale.

This contract supersedes the flat-plus-optional-bundle-copy publication text in
the calling-plans brief. Cleanup can land before the replacement producer;
until that producer lands, document flat output honestly rather than claiming
that a directory containing an executable is a completed `.app`.

## Build intent and selected output

Omega ships concrete macOS application assembly in its existing post-compilation
product-publication owner. There is no new generic packaging framework, runtime
plugin registry, or second bundle-enable flag.

`builder.application(...)` supplies the application name. GUI application intent,
currently spelled `builder.subsystem = Subsystem::Gui`, is the opt-in. For a
complete selected macOS GUI application build, `.app` publication is required,
not best-effort. The target realizations are:

| Selected application | Deliverable |
| --- | --- |
| Windows GUI | Flat executable with the GUI PE subsystem |
| macOS GUI | One `.app`, with its executable inside |
| Linux GUI | Flat ELF executable |
| macOS console | Flat Mach-O executable |

Do not publish a redundant flat executable beside the macOS GUI package. Stopping
at Terminal Psi or at a retained native artifact still stops before packaging.
An unused macOS entry binding does not turn a Windows-only invocation into a
macOS publication request. Each selected target in a multi-target invocation
retains its own realization and publication result.

Portable console/GUI intent must survive build evaluation as semantic data; it
must not collapse immediately to PE integers `3` and `2`. Raw PE loader settings
belong in PE-specific target configuration, not in the portable intent type.
`Unspecified(2)` must not accidentally acquire `Gui` packaging meaning. EFI is
an execution environment with its own entry/storage contract, not merely a PE
override and not a hosted console/GUI application. Preserve that distinction
and the existing target-owned hosted/freestanding policy rather than introducing
another independent environment switch.

## Names and native realization inputs

Use one validated application-name value for the executable leaf and `.app`
basename. Consumers reuse that value; they do not run independent sanitizers or
derive identity from the source folder. Invalid path components reject rather
than being silently rewritten. The executable is no longer universally named
`omega-program`.

An explicit authored application identifier supplies both the macOS GUI signing
identifier and `CFBundleIdentifier`. Its source field spelling remains to be
specified in the ordinary `build(builder)` vocabulary; no new syntax is claimed
implemented here. Validate it separately from the application filename. An
identifier is not proof of globally unique ownership or publisher authenticity.

The identifier is required before emitting a signed macOS GUI native image,
including a retained-native-artifact output. Missing identity is an early build
or realization configuration error, not a source-semantic rejection. Terminal
Psi production does not require it: a later consumer may supply the realization
inputs under its own authority. Merely declaring a macOS target does not require
the field for other selected targets or for a console build.

The identifier travels with build-owned native realization inputs and enters
the signed bytes and native artifact identity. Changing it requires native image
finalization/signing again, not necessarily parsing, proof, optimization, or
instruction generation. A filename rename alone must not select a different GUI
signing identifier. Display-only metadata and package placement remain
publication inputs rather than native semantic inputs.

For macOS console output, honor an explicit application identifier when supplied;
otherwise use the validated executable leaf as the ad-hoc signing label. This
removes the hardcoded signing label without making an identifier mandatory for
console programs. The fallback is neither globally unique nor rename-stable;
applications needing those identity properties should author the identifier.

## Assembly and cross-invocation publication

For application name `window-app`, v1 assembles:

```text
window-app.app/
  Contents/
    Info.plist
    MacOS/window-app
```

The plist contains `CFBundlePackageType = APPL`, `CFBundleExecutable` equal to
the validated executable leaf, `CFBundleName` from the application name, and
`CFBundleIdentifier` from the retained native realization identity. Use a fixed
encoding, key order, and escaping rule with no timestamps or ambient host facts.
Publication does not independently supply a replacement identifier.

Realization and publication may run in different invocations or on different
machines. Retain the artifact-bound realization inputs and explicit
publication-only metadata in the product envelope or a strongly bound companion,
following the existing Terminal-Psi consumer boundary. A publication request
binds the intended artifact and metadata; a conflicting companion rejects.
Changing display metadata makes a new package, not a new identity for unchanged
native bytes. No phase reaches back into an original frontend object or reads
the original project to recover missing inputs implicitly.

Assembly consumes the completed native artifact. Stage and validate the complete
package before reporting publication success; partial assembly must not be
reported as a completed deliverable. Final-destination replay must establish the
same validated contents, with failure cleanup and replacement behavior defined
by the publication implementation. Do not weaken whole-package validation to
checking only a staged executable.

## Package validation and report roles

A package record covers the executable bytes, generated plist bytes, and exact
directory shape. Missing, substituted, extra, or partial contents reject. The
validator independently checks that the plist identifier, retained realization
identity, and identifier in the actual published executable's code signature
agree. Matching two producer assertions without checking the bytes is not enough.

Expose the package root (`.../window-app.app`) separately from
`checked_native_executable_path()`, which returns
`.../window-app.app/Contents/MacOS/window-app` for bundled output. Consumers use
these checked paths, not hardcoded output names or parent-directory walking.
Validation still checks their structural relationship and exact installed bytes;
they are not independently trusted paths. Invalid publication exposes neither a
successful package result nor an executable path through that result.

Package consistency and executable ad-hoc signing establish different facts.
The executable signature does not thereby sign the external plist, and a package
record is not a distribution signature or publisher-authenticity claim. Preserve
existing execution-required ad-hoc signing. Credential-backed distribution
signing, notarization, and installation remain separate explicit operations with
separately supplied authority.

## Scope and acceptance

V1 includes executable and generated plist only. `window_app`, `window_demo`, and
`windowed_calculator` are the initial procedural-GUI customers, subject to actual
macOS launch validation. Update their authored identifiers and harness/documented
paths when enabling mandatory bundle publication; a source inspection is not a
runtime pass.

`image_viewer` is the concrete deferred resource customer: it opens BMP files
relative to its working directory. Resource support needs both inclusion and
explicit bundle-relative lookup; copying files into `Contents/Resources` alone
does not repair relative-path code. Launch method can change the working
directory. Do not silently change the process directory to conceal that
difference, or claim the resource-dependent sample works from Finder in v1.

Acceptance includes deterministic repeat assembly, missing-identifier failures at
the correct stage, Windows-only and console controls, Terminal-only stopping,
retained-native stopping, and separate-invocation publication without frontend
state. Pin identifier mismatch, plist/bytes/path/shape tampering, partial-output
failure, unchanged flat v1 digests, and both checked report paths. Verify actual
GUI launch on macOS; explicitly report when that host is unavailable.

The first publisher stays internal. A future external assembler may supply
contents for independent verification without losing the whole-package contract;
no external plugin architecture is required now. Likewise, dependencies can
supply platform services through supported mechanisms, and another consumer can
lower Terminal Psi. The first-party path must use that same semantic/authority
boundary. A new backend may be statically registered and rebuilt; packaging does
not require solving general target registration or its identity namespace here.
