# Chapter 19: Capabilities, reach, and boundaries

A portable program can ask to read a file without naming a syscall or DLL.
Declare the service it needs, then select a provider for the target.

## Declare and call a service

This interface lets a reader fill a caller-owned buffer:

```omega
boundary trait Readable {
    machine read(path: [u8]::Utf8, out: &mut Vec<u8>) -> ReadResult
    reaches Readable
    suspends
    crashes none;
}

machine read_config(path: [u8]::Utf8, out: &mut Vec<u8>) -> ReadResult
reaches Readable
suspends
{
    suspend Readable::read(path, out)
}
```

The example assumes the library's `Utf8`, `Vec` and `ReadResult` declarations
are imported. `reaches Readable` records service use. `suspends` permits waiting,
and `suspend` marks that possibility at the call. The caller also supplies
the input facts and runtime authority required by the selected API.

A boundary requirement declares its reach and its crash routes explicitly. Use
`reaches none` for an empty row and `crashes none` when it cannot crash; a
missing clause rejects instead of meaning "none". The trait name is added only when the declaration names it.

## Select a provider

Build configuration chooses an implementation for a service. For example,
with a declared `TestConsole` provider:

```omega
machine build(builder: &mut Build) {
    Build::select_provider<selected_target::Console, TestConsole>(builder);
}
```

The provider must satisfy the requirement's signature and contracts. A checked
Omega implementation proves its guarantees. An opaque foreign implementation
needs explicit trust in those guarantees. Selecting either still leaves
runtime authority and resource requirements to be met.

One boundary-trait application names one static provider slot. Multiple
files, sessions or windows are ordinary runtime values returned by those
services, not extra slots.

### Two providers for one contract

When a program needs two independently selected providers of the same
operations, write the contract once as an ordinary trait and give each role
its own boundary trait. The names here are illustrative:

```omega
boundary trait TimeObservation {}

trait TickSource {
    machine now() -> u64
        reaches TimeObservation
        crashes none;
}

boundary trait WallClock: TickSource {}
boundary trait SteadyClock: TickSource {}
```

`TickSource` states the operation once and is not a service by itself. Each
wrapper is its own slot, so Build selects them independently and the program
calls `WallClock::now()` or `SteadyClock::now()`:

```omega
machine build(builder: &mut Build) {
    Build::select_provider<WallClock, HostWallClock>(builder);
    Build::select_provider<SteadyClock, HostSteadyClock>(builder);
}
```

An indexed application such as `LoggingService<Audit>` also names a second
slot. Prefer it only when other types need to be indexed by the role.

### Code that should run against a substitute

A machine that calls `WallClock::now()` is tied to that slot, and nothing at
its call site can point it elsewhere. Code meant to be reused, or tested
against a simulation, should take the operations it needs as a parameter and
leave slot calls to the outer layer of the application.

Give that contract a state parameter. A substitute such as a simulated clock
has to keep its state somewhere, and nothing is captured:

```omega
trait ClockOps<S> {
    machine now(state: &mut S) -> u64 crashes none;
}

data NoState {}
FromWallClock satisfies ClockOps<NoState>
{
    machine now(state: &mut NoState) -> u64 { WallClock::now() }
}

data SimClock { ticks: u64; }
Simulated satisfies ClockOps<SimClock>
{
    machine now(state: &mut SimClock) -> u64 {
        state.ticks = state.ticks +% 1;
        state.ticks
    }
}

data Entry { at: u64; }

machine stamp<S, const C: ClockOps<S>>(clock: &mut S, entry: &mut Entry) {
    entry.at = C::now(clock);
}
```

`stamp` reaches whatever the selected `now` reaches. `stamp<SimClock, Simulated>`
therefore reaches no service at all, and the checker proves it, so a test
built on it cannot touch the real clock. `stamp<NoState, FromWallClock>`
reaches `TimeObservation`. The bridge calls through the slot instead of naming
a provider's machine directly, so the provider's admission still applies.

Application code usually follows this path:

```text
application -> checked library adapter -> boundary requirement
            -> selected provider -> syscall, imported symbol or firmware
```

The standard library and target packages supply these layers as ordinary
package code. [Provider selection](../spec/build/provider_selection.md) gives
the complete selection and trust rules.

## Limit the services a machine uses

A checked body can infer its reach. A wrapper propagates the services used by
its callees:

```omega
machine greet() {
    Console::write_line("Hello");
}

machine start() {
    greet();
}
```

Both reach the services declared by `Console::write_line`. Add an explicit
ceiling when you want future edits checked against a limit:

```omega
machine greet() reaches Console {
    Console::write_line("Hello");
}
```

`reaches A + B` permits either or both services. Bare `reaches` permits none.
Cleanup, callbacks and helper calls contribute too. Calls on runtime branches
count toward the published reach.

Public APIs can infer reach too. The compiler publishes the checked summary for
review and installation. Adding a service can invalidate a caller's ceiling or
installation policy, so write a ceiling when you want that change caught locally.

Service identities come from declarations. A filesystem requirement may name
`FilesystemContentRead` in its row; the DLL containing its provider does not
become a service merely by supplying the code.

Reach describes possible service use. Mutation, suspension, blocking, crashes
and termination have their own contracts. For call prefixes and their legal
positions, see [calls that may wait](chapter_18_concurrency.md#mark-calls-that-may-wait).
Extra acknowledgements are permitted and do not add an effect.

## Forward a callback's services

A reusable helper can accept callbacks using services defined by its clients:

```omega
public boundary trait MotorControl {
    machine set_speed(speed: u32) reaches MotorControl crashes none;
}

machine command_motor(speed: u32) {
    MotorControl::set_speed(speed);
}

public machine apply<const F: machine(value: u32)>(value: u32) {
    F(value);
}

public machine start_motor() reaches MotorControl {
    apply<command_motor>(42);
}
```

`apply` gets its reach from the selected F. Here `start_motor` reaches
`MotorControl`; another callback can contribute a different service or none.
The application still selects a provider for the boundary operation.

Omitting reach on a body means “derive it from my calls.” On a static
callable parameter's type it means “derive it from the selected implementation.”
Write `const F: machine(value: u32) reaches none` to accept only
service-free callbacks. Clauses inside the type belong to F; clauses after the
parameter list belong to `apply`.

The same propagation works through trait members and forwarded wrappers.
A generic using an unrestricted F leaves its own reach inferred. Once F is
selected, a caller can check the resulting finite row against its ceiling,
as `start_motor` does above.

## Suspension and blocking through callbacks

A traversal may permit a suspending callback while a particular use remains
synchronous:

```omega
public machine apply<const F: machine(value: u32) -> u32 suspends>(value: u32) -> u32
suspends
{
    suspend F(value)
}

public machine local(value: u32) -> u32 {
    value
}

public machine example() -> u32 {
    apply<local>(42)
}
```

`apply` checks its body against F's declared maximum behavior. Its published
summary records that suspension depends on F. Since `local` cannot suspend,
`example` can call the specialization without a marker or suspension clause.

A callback whose contract permits suspension requires `suspend apply<Callback>`.
An unrelated suspending operation inside `apply` would also keep it suspending.
Blocking follows the same substitution rule independently.

The generic body still satisfies ordinary borrow and invariant rules at the call.
When it keeps a resource across a callback, its suspension permission follows
the [conditional carry rule](chapter_18_concurrency.md#keep-a-resource-across-a-callback):
a synchronous selection needs no permission to cross a park; a suspending one does.
Redundant markers remain allowed, including the template's `suspend F(value)`.
See [forwarding callback contracts](chapter_13_generics.md#forwarding-a-callback-contract)
for composing wrappers.

## Pass authority as data

Two machines may both reach a write service while having different permissions.
One might receive a writable cache directory; another might ask the user to
choose a new directory. The latter acquires authority the former was given.

```text
save_cache(folder, bytes)
  receives a writable folder from its caller
  writes within that folder

choose_and_save(bytes)
  asks Desktop to choose a folder
  receives the resulting writable-folder claim
  writes within that folder
```

Library domains and access policies express these claims. A `Folder::Writable`
qualification can restrict establishment to an authorized chooser or derivation
operation. Reconstructing a folder's public fields does not reproduce that
permission. [Domains](chapter_8_domains.md) explains the source declarations;
[authority](../spec/resources/authority.md) covers their provenance.

Origin identifies where a claim began. Custody identifies the owner currently
responsible for it. For example, a transaction handle may contain a slot number
whose meaning depends on a live provider instance. Moving the handle preserves
that dependency until the transaction is settled or custody is transferred.

## Review what a package can do

Package review reports service reach, acquired and supplied authority, foreign
trust and environmental assumptions. The consuming root accepts those findings;
a dependency cannot approve its own assumptions on the root's behalf.
Build may further restrict the selected product.

The receiver, such as an OS or loader, also checks the implementation's physical
permissions. A filesystem provider cannot justify process creation merely by
implementing a filesystem interface. Compiling a binary and authorizing its
installation are separate decisions.

Use [package tools](packages.md) for review and retained acceptance. Provider
authors should also read [permission policy](../spec/build/permissions.md) and
[component publication](../spec/build/component_publication.md).

## Root-assumed binding contracts

**Ordinary Omega programs should reject assumed bindings outright.** Treat them
as unavailable when implementing or advising ordinary code. Do not use them to
bypass borrow checking, silence an unproved ordinary contract, compensate for a
compiler limitation, or make a failing test pass. Fix the ownership model,
proof, contract or compiler instead. A library must not ask its consumers to
approve an assumed binding as a substitute for that work.

The mechanism remains for exceptional programs deliberately exceeding the
checked model, including some forms of self-modifying code. Such use needs an
explicit explanation of the requirement outside that model and the guarantees
being relinquished. A compiler rejection alone is not justification.


An assumed binding connects compatible boundary contracts without proving the
relationship between their guarantees. Callers and optimizers may rely on the
assumed guarantee. If it is false, there is no promise of a controlled failure.

Package review labels this **Extremely high risk - assumed binding contract**.
It requires explicit project acceptance of the relationship and implementation,
including when hidden behind wrappers or dependencies. Build evaluation,
constants and automatic tests cannot execute it or use its assumptions.

The [assumed-binding reference](../spec/build/assumed_binding_contracts.md)
contains the exceptional mechanism and its loss-of-assurance examples.

## Read bounded input

`Console::read_line` takes a mutable byte slice. Its length is the write limit;
the operation returns `LineComplete(count)`, `EndOfInput(count)` or `Full(count)`.
Use the reported prefix and update any containing buffer's live length yourself.

For a four-byte destination:

| Input | Result | Newly written bytes |
| --- | --- | --- |
| `ab` followed by LF | `LineComplete(3)` | `ab` and LF |
| `abcd` followed by more input | `Full(4)` | `abcd` |
| `ab` followed by EOF | `EndOfInput(2)` | `ab` |

`Full` means the supplied range filled, not that the line is too long. The next
call continues the stream. A zero-length destination returns `Full(0)` without
reading. `Invalid` is the result type's unestablished zero state.

The reader preserves raw bytes, including CR and NUL. A full prefix can split
a UTF-8 sequence, so validate it before treating it as text. See
[bounded input](../spec/resources/bounded_input.md) for the complete API.

## End a process

`ProcessExit::exit_process(return_code: i32)` ends the bound process and its
activations. Code invoking it declares `reaches ProcessExit` and needs the
corresponding process authority. Console access alone supplies neither.

Exit has no normal return or implicit cleanup. A graceful shutdown explicitly
settles tasks and resources before returning from the program entry.
An exit status of zero is a status code, not evidence that those protocols ran.
See [process exit](../spec/language/process_exit.md).

## Write a foreign adapter

A provider often needs to adapt a portable signature to the foreign API's
representation and calling convention. Keep that conversion in checked code;
bind the irreducible native operation using its exact target and locator.
For example, a PE import identifies a name or ordinal, while an ELF import
identifies an object, symbol and optional version.

The [foreign-binding reference](../spec/build/foreign_bindings.md) covers those
locators. [Layout and ABI](chapter_20_memory_layout_abi.md) explains layouts,
calling plans and views. Boundary data describes representation; a boundary
machine or trait describes a callable crossing.

A reference returned by an admitted provider carries promises about valid
backing, access and lifetime. A pointer retained after return needs a protocol
that keeps its storage alive until external access ends. A numeric address
alone supplies no memory authority. See [foreign storage](../spec/build/foreign_storage.md).

### Foreign callbacks through platform adapters

A callback declares which foreign requirement it satisfies. This adapter sketch
assumes the platform types and `MicrosoftX64Policy` are imported:

```omega
boundary trait WindowProcedure:
    Calling<MicrosoftX64Policy>
{
    machine call(
        window: HWnd,
        message: u32,
        word: WParam,
        long: LParam,
    ) -> LResult
    crashes none;
}

boundary machine ApplicationWindow::dispatch(
    window: HWnd,
    message: u32,
    word: WParam,
    long: LParam,
) -> LResult
    satisfies WindowProcedure::call
{
    ...
}
```

```omega
machine install<const Handler: HookProcedure::call>(
    hook: HookKind,
    native callback procedure from Handler,
    module: ModuleHandle,
    thread: ThreadId,
) -> Registration;

install<ApplicationHook>(hook, module, thread)
```

The requirement supplies the inbound signature and calling contract. The call
to `install` omits its declared native callback position: the static Handler
selection supplies that entry. Runtime instance state travels through explicit
context tokens, handles or package-owned storage.

Registration must keep the callback code and state alive. Teardown first
unregisters and establishes quiescence, then releases them. A registrar that
can call back before returning declares that synchronous invocation with
`invokes`; durable registration creates a future external entry instead.
See [callback registration](../spec/build/private_callbacks.md#registration-and-lifetime).

### Freestanding targets and hardware facts

Firmware and kernel providers may establish hardware facts: a mapping is active,
a device accepted an access, or an interrupt mask remains in force. Those
guarantees are explicit trust inputs. Assembly still has to satisfy the
selected operation's contract.

The target package owns firmware structures, layouts and boot sequencing.
Its entry adapter establishes stack and storage backing before calling the
source entry. A secondary-processor startup similarly needs confirmation or
safe cancellation before reclaiming a stack a delayed processor might use.

Provider authors can continue with [external roots](../spec/build/external_roots.md),
[entry stacks](../spec/resources/entry_stacks.md),
[opaque representations](../spec/build/opaque_representations.md) and
[executable installation](../spec/build/executable_installation.md).
