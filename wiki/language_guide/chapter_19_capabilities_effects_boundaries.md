# Chapter 19: Capabilities, reach, and boundaries

Omega code cannot touch the outside world by accident. Files, the console,
the clock and hardware are reached through boundary traits. A program
declares which ones it uses and the build decides what is behind each.

## Declare and call a service

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

A boundary trait is called by path, like `Console::write_line`. There is no
handle to pass around for the service itself. `reaches Readable` on
`read_config` records that it uses the service.

## Select a provider

The build says what implements each boundary trait:

```omega
machine build(builder: &mut Build) {
    Build::select_provider<selected_target::Console, TestConsole>(builder);
}
```

The standard library and target packages supply providers for ordinary
platforms, so most programs never write this line.

```text
application -> library -> boundary trait -> selected provider -> syscall or firmware
```

## Limit the services a machine uses

A machine's reach is worked out from what it calls:

```omega
machine greet() {
    Console::write_line("Hello");
}

machine start() {
    greet();                 // reaches Console, through greet
}
```

Write the clause to set a limit that later edits are checked against:

```omega
machine greet() reaches Console {
    Console::write_line("Hello");
}

machine pure_math(x: u32) -> u32 reaches none { ... }
```

`reaches A + B` allows either. `reaches none` allows no service at all.

## Two providers for one contract

One boundary trait is one slot. For two independently chosen providers of the
same operations, write the operations once and wrap them twice:

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

```omega
machine build(builder: &mut Build) {
    Build::select_provider<WallClock, HostWallClock>(builder);
    Build::select_provider<SteadyClock, HostSteadyClock>(builder);
}
```

## Code that should run against a substitute

A machine that calls `WallClock::now()` is tied to that slot. Code you want to
test against a fake takes its operations as a parameter instead:

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

`stamp<NoState, FromWallClock>` reaches the real clock.
`stamp<SimClock, Simulated>` reaches nothing, and the compiler proves it, so a
test built on it cannot touch the real clock. The state parameter is there
because a fake has to keep its state somewhere and nothing is captured.

## Forward a callback's services

A generic helper reaches whatever its callback reaches:

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

`apply` knows nothing about motors. `apply<command_motor>` reaches
`MotorControl`. To accept only callbacks that reach nothing, type the
parameter `const F: machine(value: u32) reaches none`.

## Suspension and blocking through callbacks

Waiting follows the callback the same way:

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
    apply<local>(42)          // local cannot suspend, so no marker is needed
}
```

## Pass authority as data

Reaching a service is not the same as being allowed to use it on a particular
thing. Permission to write one folder is a value, and a machine can only use
what it was handed:

```text
save_cache(folder, bytes)        receives a writable folder, writes inside it
choose_and_save(bytes)           asks the user for a folder, then writes inside it
```

A domain such as `Folder::Writable` with an access policy
([Chapter 8](chapter_8_domains.md#read-and-write-authority)) makes the
permission impossible to forge: only the chooser can create one. See
[authority](../spec/resources/authority.md).

## Review what a package can do

Package review lists, for each dependency, the services it reaches, the
authority it acquires and any foreign code it trusts. The project that builds
the final program accepts those findings; a dependency cannot approve itself.
See [package tools](packages.md).

## Foreign code

A provider that calls a C API or a syscall keeps the conversion in checked
Omega and binds only the native call itself. A callback handed to foreign code
declares the requirement it satisfies:

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

See [foreign bindings](../spec/build/foreign_bindings.md),
[callback registration](../spec/build/private_callbacks.md#registration-and-lifetime)
and [Chapter 20](chapter_20_memory_layout_abi.md).

## Root-assumed binding contracts

**Ordinary programs should not use assumed bindings.** An assumed binding
tells the compiler to trust a relationship it cannot check. If the claim is
false, nothing promises a controlled failure. Do not use one to get past a
borrow error, an unproved contract or a compiler limitation; fix the program,
the contract or the proof. Package review labels every use "Extremely high
risk". The [reference](../spec/build/assumed_binding_contracts.md) covers the
rare programs that need one.

## Small things worth knowing

- **Reading input.** `Console::read_line` fills a byte slice you supply and
  reports `LineComplete(count)`, `EndOfInput(count)` or `Full(count)`. `Full`
  means your buffer filled, not that the line ended. See
  [bounded input](../spec/resources/bounded_input.md).
- **Exiting.** `ProcessExit::exit_process(code)` ends the process without
  cleanup and needs `reaches ProcessExit`. Returning from the entry machine is
  the graceful way out. See [process exit](../spec/language/process_exit.md).

## Where next

[Chapter 20](chapter_20_memory_layout_abi.md) covers layout and foreign data.
The complete rules are in [effects](../spec/language/effects.md) and
[provider selection](../spec/build/provider_selection.md).
