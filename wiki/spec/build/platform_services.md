# Platform services

How a library offers one portable API over operating-system services. The
filesystem is the first instance; clocks, consoles and networking follow the
same shape. The library has no compiler privilege: this is ordinary package
structure under [provider selection](provider_selection.md), where a selected
provider covers its complete schema.

## Three layers

Named decision: `portable-service-per-family-binding-carrier`, settled
2026-10-13.

1. **Portable service.** The only API applications use (for example
   `Filesystem`). It is shaped for the ideal target, Cathedral, and is
   identical on every target.
2. **Raw per-OS services.** One boundary trait per operating-system family
   (for example `PosixFilesystemHost`, `WindowsFilesystemHost`, later
   `CathedralFilesystemHost`). Each declares exactly what that OS provides, and
   the provider bound on that target covers all of it. A raw trait never mixes
   families, so no provider needs stub or sentinel rows to cover operations
   its OS lacks. Raw handle types are per family (`PosixDescriptor`,
   `WindowsHandle`) and never appear in the portable API. Operations that exist
   on only one family are reachable only through that family's raw trait.
3. **Target implementations.** Target-scoped machines
   (`source/library/std/targets/<target>/`) implement the portable service over
   the raw service for that target. All translation between the portable shape
   and the OS shape happens here.

## Provider-closure transport

Named decision: `provider-body-boundary-authority`, settled 2026-10-14.

An application calls only the exact `PortableService` requirement application.
The selected target implementation's checked provider closure retains every exact raw-family
requirement call and selected raw provider it needs. Installing the portable
slot pins that complete transitive closure to the portable provider era;
changing a raw provider, its contract, or its target family changes the closure
and requires fresh admission. Artifact/cache identity, cycle detection,
replacement checks, review, and authority evidence all include those transitive
dependencies.

The closure dependency is not a source field in portable data, a target-varying
portable type argument, or an implicitly manufactured routing value. It is
static provider-plan structure beneath the one portable slot. Entered portable calls may invoke only the raw rows retained in that
checked closure, under their declared reach and physical-authority evidence.
No provider name, target default, or family spelling grants additional calls.

Code that genuinely needs a separately selected raw-family role names a distinct
exact requirement application or wrapper trait and calls it statically. Runtime
multiplicity uses the raw protocol's ordinary handles/sessions. That is a
different API from the portable facade. Provider closure transport cannot expose
private raw slots to application source, hide an ambient service, or weaken
authority and review checks.

Consequently the portable declaration and all application-visible data remain
identical on every target. Only provider planning chooses the applicable raw
family. Unsupported or missing target closure rejects rather than adding a
sentinel provider, demanding every family's binding, or changing the portable
carrier.

Raw requirements explicitly declare the capability identities their operations
reach. Filesystem reads, writes, metadata queries/mutations, directory enumeration
and namespace mutations use the corresponding core service identities; a
multi-purpose operation declares their conservative union. Platform family names
are provider-selection identities, not automatically added service effects.
Pure error translation or arithmetic does not acquire service reach merely
because its implementation is selected. Checked providers must fit the declared
rows; foreign supply and exercised physical authority remain separately audited.
The exact operation classifications in
[filesystem authority](permissions.md#portable-filesystem-control-and-lifecycle-authority)
still govern locking, durability, release and other operations; grouping an
operation under a filesystem interface does not turn it into content access.

## Zero-initialized portable shapes

Portable inputs and results are ZII-shaped:

- **Handles:** the zero value means absent. There are no sentinel values such as
  `-1` in the portable API.
- **Inputs:** a zero-initialized options or mode value means the default
  behavior, so the common call passes zeroed storage.
- **Results:** the zero case of a result is the failure case, so untouched
  result storage fails closed.
- **Unsupported operations:** an operation a target cannot perform returns the
  portable error kind `Unsupported`, which is part of the portable contract.
  It is never produced by a raw-service stub.

On Cathedral the portable shape is the native shape. On other operating
systems the target implementation pays a small translation cost, which is the
accepted price of a unified API shaped for the ideal form.

## Filesystem mapping

| Concern | Linux, macOS | Windows | Cathedral |
| --- | --- | --- | --- |
| `File` storage | descriptor + 1 (descriptor 0 is a valid file) | the `HANDLE`; success is never 0 and `INVALID_HANDLE_VALUE` maps to the failure result | native handle; zero is absent |
| Zero `OpenOptions` | read-only, mode `0o666` before the process umask | read-only, default sharing | native defaults |
| Paths | NUL-free bytes | re-encoded to UTF-16 | native |
| Errors | `errno` mapped to `ErrorKind` | Win32 error mapped to `ErrorKind` | native |
| Directory enumeration | `ReadDir` zero means not started; the target implementation owns the OS cursor | same | same |
