# macOS application publication

This is the required contract; implementation remains under
MACOS-APPLICATION-PUBLICATION in [TASKS.md](../../../TASKS.md). Current macOS
output is flat, with std requesting foreground activation. A flat executable
can display a window: bundling is delivery, not GUI execution authority.
[Component publication](component_publication.md) owns the separate
native/installed-product boundary.

## Intent and deliverable

Concrete macOS assembly belongs to post-compilation product publication, not
Psi, instruction lowering, a plugin registry, or a generic packaging framework.
`builder.application(...)` supplies the name. GUI intent, currently spelled
`builder.subsystem = Subsystem::Gui`, is the opt-in; there is no second bundle
flag. Complete selected macOS GUI application output requires a bundle.

| Selected application | Deliverable |
| --- | --- |
| Windows GUI | Flat executable with GUI PE subsystem. |
| macOS GUI | One `.app` containing the executable. |
| Linux GUI | Flat ELF. |
| macOS console | Flat Mach-O. |

Do not publish a redundant flat copy beside the bundle. Terminal-only and
retained-native stops remain before packaging. An unused macOS entry imposes
no macOS publication on other selected targets; each target has its own result.

Portable console/GUI intent remains semantic data through Build evaluation,
not PE integers 3/2. Raw PE settings belong to PE-specific configuration;
`Unspecified(2)` cannot acquire GUI meaning. EFI retains its distinct entry,
storage, and target-owned environment contract, not another environment switch
or a hosted console/GUI classification.

## Names and metadata ownership

One validated application name supplies executable leaf and `.app` basename.
Reject invalid path components; do not use independent sanitizers or derive
names from source folders. The universal `omega-program` filename is replaced.

An explicit authored identifier supplies GUI CodeDirectory signing identity and
`CFBundleIdentifier`. Validate it separately from the filename; it proves neither
globally unique ownership nor authenticity. Its ordinary Build field spelling
remains to be specified by the implementation task.

Require the identifier before signed macOS GUI image emission, including
retained-native output. Absence is an early build/realization configuration error,
not source-semantic rejection. Terminal production needs none; a later consumer
may supply it under its authority. Merely declaring macOS does not require it
for console output or other selected targets.

The identifier enters native realization inputs, signed bytes, and artifact
identity. Changing it requires image finalization/signing, not necessarily
parsing, proof, optimization, or instruction generation. Renaming a GUI executable
alone does not change it. Display-only metadata and destination are publication
inputs. Console output honors an explicit identifier, otherwise using the
validated executable leaf as its ad-hoc label; that fallback is neither globally
unique nor rename-stable.

## Assembly

For `window-app`, the complete v1 tree is:

```text
window-app.app/
  Contents/
    Info.plist
    MacOS/window-app
```

The plist uses package type `APPL`, executable leaf as `CFBundleExecutable`,
application name as `CFBundleName`, and retained realization identity as
`CFBundleIdentifier`. Encoding, key order, and escaping are fixed, without
timestamps or ambient facts. Publication cannot replace the signed identifier.

When [native PCC](../proofs/publication.md) is requested, also publish
`Contents/MacOS/window-app.proof`, bound to the finalized executable bytes.
Otherwise that file is absent. This is a proof about the executable under its
declared environment, not a signature or proof of the entire bundle.

Realization and publication may run in separate invocations/machines. Carry
artifact-bound realization inputs and publication metadata in the envelope or
a strongly bound companion. Bind the request to that artifact and metadata;
conflicts reject. Never recover missing inputs from the original frontend or
live project. Display changes change the package, not unchanged native bytes.

Stage and validate the complete package before success, then replay the same
contents at the final destination. Partial assembly is not a deliverable. The
implementation must define failure cleanup and replacement behavior.

## Validation and reports

A package record covers exact executable/plist bytes, the proof sidecar when
requested, and directory shape.
Missing, extra, substituted, or partial contents reject. Independently compare
the plist identifier, retained realization identity, and identifier in the
actual published executable signature. Matching producer assertions is insufficient.

Expose the checked package root separately from `checked_native_executable_path()`,
which returns the inner `Contents/MacOS/window-app`. Validate their structural
relationship and installed bytes; they are not independently trusted paths.
Consumers use accessors, not hardcoded names or directory walking. Invalid
publication exposes neither a successful package nor its executable path.

Executable ad-hoc signing does not sign the external plist. Package consistency
is not a distribution signature or authenticity claim. Preserve execution-required
ad-hoc signing; credential-backed signing, notarization, and installation remain
separate operations with separate authority. A future external assembler may
supply contents for independent verification; the first publisher is internal
and requires no general extension mechanism.

## Bounded acceptance

V1 includes executable, plist, and the independently requested native proof
sidecar; no application resources. Validate procedural `window_app`,
`window_demo`, and `windowed_calculator` on macOS after adding identifiers and
reported-path consumers. Source inspection is not a runtime pass. `image_viewer`
is deferred: working-directory-relative BMP access needs inclusion and explicit
bundle-relative lookup. Copying resources alone does not repair it. Do not
silently change process directory or claim Finder coverage for that sample.

Controls cover deterministic assembly; identifier requiredness; Windows, console,
Terminal and retained-native stops; cross-invocation publication without frontend
state; identifier, plist, executable, path and shape tampering; partial output;
unchanged flat v1 digests; and both checked paths. Report unavailable macOS runtime
coverage explicitly.
