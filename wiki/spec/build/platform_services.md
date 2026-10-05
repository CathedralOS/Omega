# Platform services

How a library offers one portable API over operating-system services. The
filesystem is the first instance; clocks, consoles and networking follow the
same shape. The library has no compiler privilege: this is ordinary package
structure under [provider selection](provider_selection.md), where a selected
provider covers its complete schema.

## Three layers

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
