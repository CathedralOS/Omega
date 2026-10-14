# Chapter 23: Testing

Tests are ordinary machines. A project declares a machine requirement for each
test group, registers it in `build.omg`, and the build runs its implementations.
There is no test attribute or special `run` name. The
[testing specification](../spec/build/testing.md) defines the contract;
[implementation work](../../TASKS.md#requirement-based-tests) is tracked separately.

## Declare a group, then write tests

```omega
trait Tests {
    machine arithmetic()
        crashes Trap;
}

machine addition_is_correct()
satisfies Tests::arithmetic
{
    transition 2u32 + 3u32 != 5u32 {
        true -> failed()
        _ -> passed()
    }

    state failed() {
        crash Trap;
    }

    state passed() { }
}

machine subtraction_is_correct()
satisfies Tests::arithmetic
{
    transition 5u32 - 3u32 != 2u32 {
        true -> failed()
        _ -> passed()
    }

    state failed() {
        crash Trap;
    }

    state passed() { }
}
```

The trivial arithmetic makes the mechanics visible; real tests call the code
under test and compare its results. `satisfies` checks and inherits the selected
requirement's contract, including its trap allowance. Each machine implements
one requirement, not an entire trait.

In the package's `build.omg`:

```omega
machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_example");
    _ = builder.tests.group<Tests::arithmetic>();
}
```

Both tests are discovered automatically. Adding another satisfier needs no build
edit. Registration names the exact product requirement; it does not import or
execute that product code as a build helper. The runner collects the current
package's implementations, not tests in every imported dependency.

One trait can contain several groups. Register each desired requirement
separately; its name has no special meaning. A wrapper inheriting the same
requirement does not give that requirement a new identity. Open generic tests
need concrete ordinary wrappers rather than guessed type arguments.

Std may provide `testing::Test::run` as a convenient basic contract. Using it
still requires an ordinary dependency and group registration. Your own traits
work without std and can coexist with it; there is no built-in category list.

## Services and fixtures

A group declares permitted behavior; the build selects compatible providers.
For service-bearing tests, declare an ordinary context parameter for provisioning:

```omega
trait Tests<Context> {
    machine memory(context: &mut Context)
        reaches Filesystem;
        crashes Trap;
}

data ScanFixture { }

machine ScanFixture::empty_input(context: &mut ScanFixture)
satisfies Tests<ScanFixture>::memory
{
    // Filesystem operations call the selected static provider slot.
    // Keep scratch buffers local to this invocation.
}
```

Here `Filesystem` and `MemoryFilesystem` below stand for the selected library's
service and compatible in-memory provider; the compiler does not recognize
them by spelling. This is a separate example from the arithmetic-only trait.

```omega
let memory: TestGroup = builder.tests.group<Tests<ScanFixture>::memory>();
memory.service<Filesystem, MemoryFilesystem>();
```

The registered requirement explicitly applies `Tests` to `ScanFixture`;
ordinary trait-argument substitution fixes the context parameter's type. Each invocation receives
a fresh context and mock-service state. The runner
installs the selected group-scoped provider slot before entering the test; it
does not add a service field to the context. Ordinary fields must be zero-valid,
and additional fixture construction is ordinary code. The runner supplies the explicitly declared
context parameter. A test entry takes no other arguments; call
parameterized helpers from its body.

The root controls the actual authority. If it supplies a virtual filesystem,
a dependency cannot obtain the host filesystem by selecting a different provider
for its tests. Descendants may narrow grants, not enlarge them. Live host access
needs both project acceptance and an actual scoped executor grant. Fresh memory
does not undo writes to a real filesystem or make foreign code safe.

## Enablement and results

Build-time tests cannot call a root-assumed binding relationship or rely on its
assumptions, including through helpers, fixtures or providers. Runtime approval
and a genuine host-service grant do not override this exclusion. An independently
checked supplier may still be tested without the assumed relationship; otherwise
use a separately executed runtime harness under its own acceptance. See
[assumed contracts](../spec/build/assumed_binding_contracts.md#runtime-only).

Registered groups are enabled by default. Build configuration may set
`memory.disabled = true;` or `builder.tests.disabled = true;` using ordinary
admitted computation and explicit inputs. Disabled means not run, never passed.
No debug/release convention or ambient environment lookup is implied.

After build configuration and source checking, enabled tests run on every
applicable, permitted engine the host supports, unless group configuration
overrides that choice. Target environment and engine are separate: a Linux test
may run both interpreted and natively. Being able to cross-compile a target
does not mean the host can execute it.

The runner shows the target/engine plan before execution. For example:

| Test environment | Interpreted | Native |
| --- | --- | --- |
| Pure arithmetic | Run | Run |
| Virtual filesystem supported by both engines | Run | Run |
| Binding with only a native realization | Unavailable: no host adapter | Run |

Build group configuration can exclude engines or require particular combinations.
A required unavailable combination prevents success; optional unavailable ones
remain visible, never counted as passes. Each enabled test must run on at least
one engine. A virtual filesystem stays virtual in native tests too: engine
selection cannot change the selected services or increase permissions.

Results name the test, target and engine. Normal completion passes; a trap or
abort fails. Exhaustion of a scheduled invocation is incomplete execution and
prevents build success, not an excuse to skip it. Sponsor limits bound execution
even when a test has no termination promise. Tests do not gain implicit unwinding.

Use mandatory checks for the verdict. An optional assertion whose implementation
can be replaced with a no-op cannot reliably decide whether a test passed.
Testing also cannot validate an admitted guarantee by assuming that same
guarantee; see [admissions and runtime diagnostics](../spec/proofs/contracts.md#admission-and-runtime-diagnostics).
Passing examples are not universal proofs. A bounds-safe scanner can still
return the wrong matches unless its result behavior is proved or tested.

## Application output and native testing

Registration adds runner roots, not application roots. Test-only code and mock
providers therefore stay out of the application Psi and native image without
depending on an optional optimizer. Application code may call a visible test
machine normally; if it does, that reachable code can ship and must obey the
application's ordinary contracts.

Automatic native runs use a separate test executable; interpreted runs execute
the test's Terminal roots. Share compatible compilation work, but keep both
test products separate from application outputs. An interpreted pass says nothing
about native lowering; a native pass against a virtual device says nothing about
real hardware behavior.

For a custom runtime harness, build an ordinary separate executable with its own
entry binding. Its main can call shared helpers or visible test machines without
a test-mode flag. It does
not automatically discover groups or acquire their private visibility, unlike
the build runner operating in the owning package's test scope. A native trap
requires real external containment if execution is to continue.

The interpreter runner shares the ordinary
[embedding lifecycle](chapter_24_embedding.md), rather than a separate test VM.
Fresh instances isolate their guest state, not effects on shared genuine providers.
