# Platform services

How a library offers one portable API over operating-system services. The
filesystem is the first instance; clocks, consoles and networking follow the
same shape. The library has no compiler privilege: this is ordinary package
structure under [provider selection](provider_selection.md), where a selected
provider covers its complete schema.

## Standard streams

The standard library's `Console` trait offers portable byte input and output.
Its read operations explicitly reach the core service `StandardInput`. Its
write operations explicitly reach `StandardOutput`. These identify the byte
streams designated as standard input and standard output for this invocation.
A host can connect them to a terminal, pipe, file, or supplied endpoint.
Redirection preserves the declared service. The host supplies the endpoints
separately from the caller's reach ceiling.

Standard error, diagnostic logging, graphical presentation, and routed device
events are outside these two contracts. A host can provide the standard library
interface wherever it can preserve its byte-stream semantics. The
[bounded input contract](../resources/bounded_input.md) defines read outcomes.

An interpreted Build can import `use {omega}/language/core/sandbox/streams.omg as sandbox;`.
`|sandbox|::SandboxOutput::write` appends the supplied bytes to the caller’s
bounded diagnostic sink. The system package’s `InterpreterConsole::write_byte`
supplies a one-byte view and `write_line` supplies its trailing LF. A caller must
supply an input endpoint separately for code that reaches `SandboxInput`.
[Build sandbox bindings](scoped_execution.md#build-sandbox-bindings)

## Network reach

A helper can send a reply on a flow its caller already opened without acquiring
the power to choose who gets that reply. A **flow** is a path for exchanging data
with a peer. A connected socket is one possible implementation. The
`network-service-reach` decision (2026-11-23) applies to both hosted sockets
and protocol stacks that run in the application.

Import `{omega}/language/core/network_authority.omg` to name the memberless
core boundary trait `Network` and its five memberless children. Each child
identifies an operation. Reaching any child also reaches
`Network` under the ordinary [boundary-parent rules](../language/effects.md#boundary-services-with-parameters).

| Service | What the operation permits |
| --- | --- |
| `NetworkResolveName` | Submit a name to a resolver service. The resolver can observe the name even if the caller never contacts the peer. |
| `NetworkInitiateContact` | Open an outbound flow or designate the destination for an outbound datagram. The caller's grant limits which peers it can contact. |
| `NetworkServe` | Make an endpoint reachable for new incoming contact or accept an incoming flow on an existing listener. |
| `NetworkSend` | Supply application bytes for transmission on a flow or to a datagram destination. |
| `NetworkReceive` | Receive application bytes from a flow or datagram endpoint. Receiving a reply on an existing flow requires this reach alone. |

Passing a sender or listener to a helper transfers only the authority the value
carries. A destination address alone grants no authority. Passing any of these
values adds no network reach. Each operation below lists its reach before
boundary parents are added.

| Operation | Required reach |
| --- | --- |
| Send bytes on a sender whose peer is already fixed | `NetworkSend` |
| Open an outbound flow to a chosen peer without supplying application bytes | `NetworkInitiateContact` |
| Send a UDP datagram with a caller-supplied destination | `NetworkInitiateContact + NetworkSend`, even when no separate connect call occurs |
| Reserve a local address or port without opening it to incoming contact | `Network` |
| Enable incoming contact, or take a new connection from an existing listener | `NetworkServe` |

A socket library can declare `SocketBind` as a child of `Network` and
`SocketListen` and `SocketAccept` as children of `NetworkServe`. When a boundary operation
declares such a refinement, it propagates to callers. Other libraries can use
the core identities directly. A bind operation that also makes an endpoint
reachable must include
`NetworkServe`.

Portable and raw platform requirements declare the full union of what each
operation permits. A Windows `ConnectEx` binding that allows an initial payload
reaches both `NetworkInitiateContact` and `NetworkSend`. A binding whose
checked contract excludes that payload can expose just `NetworkInitiateContact`.
Passing an empty buffer to the broader binding still uses its published row.
An in-process transport can send on an already-authorized flow with the same
`NetworkSend` contract. The receiver checks the [physical network authority](permissions.md#physical-network-authority)
of the selected provider separately.

Reading a caller-supplied table to find an address is ordinary computation.
Calling a resolver boundary contributes its declared reach even if its provider
answers from a recording. The [critical audit policy](../packages/review.md#critical-service-audit-policy)
highlights `Network` for both Build and Product code. The [default Build environment](execution.md#restricted-build-requests)
supplies neither live network access nor a subprocess service. The caller can delegate [live networking](scoped_execution.md#live-networking-in-a-build)
and [program launches](program_launch.md#grant-and-delegation) independently.
The [standalone Product](command_line.md#runtime-directory-mappings) also starts
with live networking and program launch disabled and requires its own grants.

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
3. **Target implementations.** Ordinary named implementations
  (`source/library/system/targets/<target>/`) implement the portable service over
  the raw service. Build selects them through explicit target-keyed provider
  rows; neither the folder name nor an OS-prefixed machine declaration supplies
  selection. All translation between the portable shape and the OS shape
  happens here. One implementation may serve several compatible targets.

## Provider-closure transport

Named decision: `provider-body-boundary-authority`, settled 2026-10-14.

An application calls only the exact `PortableService` requirement application.
The selected target implementation's checked provider closure retains every exact raw-family
requirement call and selected raw provider it needs. Installing the portable
slot pins that complete transitive closure to the portable provider era;
changing a raw provider, its contract, or its selected target row changes the closure
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

### Filesystem values and partial results

`read`, `write`, `read_at` and `write_at` may transfer fewer bytes than the
caller supplies. A valid buffer does not become invalid because its length
exceeds a native call's count field: the adapter submits a representable prefix
and returns the actual count. Reads still refuse a count outside the supplied
buffer; positioned operations still refuse a negative offset. `write_all`
continues until all bytes are written or an error occurs, refusing zero progress.
`copy` copies the initialized prefix returned by its bounded read, which may be
short, and reports exactly that prefix's length.

`DirEntry.name` stores up to 765 bytes, and `name_len` counts the returned bytes
without a NUL terminator. This accommodates the longest supported component:
255 UTF-16 code units can require at most 765 UTF-8 bytes. A supplementary
Unicode scalar occupies two UTF-16 code units and four UTF-8 bytes; the adapter
must preserve the scalar, never truncate either encoding. POSIX names retain
their original bytes. Malformed native records or invalid UTF-16 fail rather
than producing a partial name. Enumeration order and a directory snapshot are
not promised.

`set_times` receives signed whole Unix seconds, including times before 1970.
The timestamp is valid independently of an operating system's encoding or the
underlying filesystem's supported range and precision. An adapter that cannot
represent a requested time returns `Unsupported` without applying either update;
it must not clamp the value or report success for a no-op. Windows therefore
returns `Unsupported` for the 1601 epoch (whose zero FILETIME means "leave
unchanged"), earlier times, and values too large for whole-second FILETIME.
Otherwise the host filesystem may still refuse the update or round its stored
precision; successful conversion alone does not promise host acceptance or an
exact subsequent metadata value. Invalid handles and host errors retain their
ordinary error meanings.
