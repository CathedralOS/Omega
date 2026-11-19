# Chapter 23: Testing

A test is an ordinary machine. You declare a group, write machines that
belong to it, and register the group in `build.omg`.

## A group and two tests

```omega
trait Tests {
    machine arithmetic()
        crashes Trap;
}

machine addition_is_correct()
satisfies Tests::arithmetic
{
    transition 2u32 + 3u32 != 5u32 {
        true -> failed(),
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
        true -> failed(),
        _ -> passed()
    }

    machine failed() {
        crash Trap;
    }

    machine passed() { }
}
```

```omega
machine build(builder: @mut Build) {
    Build::package(builder, "arithmetic_example");
    _ = builder.tests.group::<Tests::arithmetic>();
}
```

A group is a trait requirement. Every machine that `satisfies` it is a test in
that group and is found automatically. A test passes by returning and fails by
crashing.

## Services and fixtures

Register a filesystem test group the same way as an arithmetic group.

```omega
let memory: TestGroup = builder.tests.group::<Tests::memory>();
```

For the test product, select a provider for the same `FilesystemService`
that the application calls. For example, a harness that defines a conforming
provider named `PrivateFilesystem` would make this selection in the test
product's configuration:

```omega
Build::select_provider::<FilesystemService, PrivateFilesystem>(builder, linux_x86_64);
```

The harness supplies a fresh, empty backing for that provider each time a test
runs. Inside a test, an ordinary call such as
`block FilesystemService::create_new("note.txt")` creates the file in that
test's filesystem. A helper can open it again. The next test can create
`note.txt` anew, even when both tests run in parallel.

A dependency given that view can pass it along or narrow it. Asking for a host
filesystem cannot give it access to the developer's files. The application's
own provider selection is independent. See
[test configuration and authority](../spec/build/testing.md#test-runner-filesystem-slot).

## What runs where

Tests run during the build, on every engine the host supports:

| Test uses | Interpreted | Native |
| --- | --- | --- |
| Pure computation | Runs | Runs |
| A service both engines can provide | Runs | Runs |
| A binding that exists only natively | Unavailable | Runs |

Results name the test, the target and the engine. A disabled or unavailable
test is reported as such and never counted as a pass:

```omega
memory.disabled = true;
```

## Tests stay out of the product

Test code and mock providers are built separately from the application and are
not shipped. A passing test shows that one execution behaved; it is not a
proof, and a simulated device is not a test of real hardware.

## Where next

[Chapter 24](chapter_24_embedding.md) covers embedding. The complete rules are
in [testing](../spec/build/testing.md).
