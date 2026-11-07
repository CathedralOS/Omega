# Chapter 23: Testing

Tests are ordinary machines. A project declares a machine requirement for each
test group, registers it in `build.omg`, and the build runs its implementations.
The [testing specification](../spec/build/testing.md) defines registration and execution.

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

    machine failed() {
        crash Trap;
    }

    machine passed() { }
}

machine subtraction_is_correct()
satisfies Tests::arithmetic
{
    transition 5u32 - 3u32 != 2u32 {
        true -> failed()
        _ -> passed()
    }

    machine failed() {
        crash Trap;
    }

    machine passed() { }
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

Both tests are discovered automatically. Adding another implementation of
`Tests::arithmetic` adds it to this package's group. Register each group you
want to run. Generic tests use concrete wrappers to supply their arguments.

## Services and fixtures

A group declares which services its tests may use. The build supplies a
provider for each service, such as an in-memory filesystem. This configuration
fragment assumes a group `Tests::memory` that reaches `Filesystem`:

```omega
let memory: TestGroup = builder.tests.group<Tests::memory>();
memory.service<Filesystem, MemoryFilesystem>();
```

Construct fixture data inside each test and pass it to helpers as ordinary
arguments. Each invocation receives fresh mock-service state.

The root still controls actual authority. A dependency's tests cannot upgrade
a supplied virtual filesystem into host filesystem access. Tests using real
external services must account for their effects; fresh fixture data does not
undo a file write.

## Enablement and results

Build-time tests cannot rely on [root-assumed binding contracts](../spec/build/assumed_binding_contracts.md#runtime-only).
Test an independently checked implementation, or use a separate runtime harness
for an exceptional program that requires those assumptions.

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

## Keep tests separate from the application

The runner builds test roots separately from application roots. Mock providers
and test-only code stay out of the shipped product. Calling a test helper from
application code makes that helper ordinary reachable application code.

For a custom runtime harness, build a separate executable with its own entry
binding and call the helpers you need. Use external containment if the harness
must continue after a native trap.

An interpreted pass tests that execution route; a native pass also exercises
native lowering. Neither makes a simulated device a test of real hardware.
