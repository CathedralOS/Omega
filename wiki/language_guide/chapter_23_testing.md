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
    builder.package("arithmetic-example");
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
For service-bearing tests, use ordinary receiver provisioning:

```omega
trait Tests {
    machine memory(&mut self)
        reaches Filesystem;
        crashes Trap;
}

data ScanFixture {
    filesystem: Binding<Filesystem>;
}

machine ScanFixture::empty_input(&mut self)
satisfies Tests::memory
{
    // Use self.filesystem and ordinary scanning/checking helpers.
}
```

Here `Filesystem` and `MemoryFilesystem` below stand for the selected library's
service and compatible in-memory provider; the compiler does not recognize
them by spelling. This is a separate example from the arithmetic-only trait.

```omega
let memory = builder.tests.group<Tests::memory>();
memory.service<Filesystem, MemoryFilesystem>();
```

Each invocation receives a fresh receiver and mock-service state. The runner
establishes the `Binding` field before entering the test; zeroed bytes do not
make a service valid. Other fields must be zero-valid, and additional fixture
construction is ordinary code. A test entry takes no other arguments; call
parameterized helpers from its body.

The root controls the actual authority. If it supplies a virtual filesystem,
a dependency cannot obtain the host filesystem by selecting a different provider
for its tests. Descendants may narrow grants, not enlarge them. Live host access
needs both project acceptance and an actual scoped executor grant. Fresh memory
does not undo writes to a real filesystem or make foreign code safe.

## Enablement and results

Registered groups are enabled by default. Build configuration may set
`memory.disabled = true;` or `builder.tests.disabled = true;` using ordinary
admitted computation and explicit inputs. Disabled means not run, never passed.
No debug/release convention or ambient environment lookup is implied.

After build configuration and source checking, enabled tests run through the
Terminal Psi interpreter before the build can succeed. Normal completion passes;
a trap or abort fails. Exhaustion and unsupported execution are reported
separately and also prevent build success. Sponsor limits bound execution even
when a test has no termination promise. Tests do not gain implicit unwinding.

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

Automatic tests do not exercise native lowering or a real deployment. For that,
build an ordinary separate executable with its own entry binding. Its main can
call shared helpers or visible test machines without a test-mode flag. It does
not automatically discover groups or acquire their private visibility, and a
native trap requires real external containment if execution is to continue.

The interpreter runner shares the ordinary
[embedding lifecycle](chapter_24_embedding.md), rather than a separate test VM.
Fresh instances isolate their guest state, not effects on shared genuine providers.
