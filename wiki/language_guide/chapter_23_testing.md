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

```omega
machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_example");
    _ = builder.tests.group<Tests::arithmetic>();
}
```

A group is a trait requirement. Every machine that `satisfies` it is a test in
that group and is found automatically. A test passes by returning and fails by
crashing.

## Services and fixtures

A test that uses a service names its slot, and the test runner supplies it.
For std's `FilesystemService` the runner supplies a private in-memory
filesystem, so ordinary filesystem calls in the test reach it the way they
reach a real one:

```omega
let memory: TestGroup = builder.tests.group<Tests::memory>();
builder.tests.service<Tests::memory, FilesystemService>();
```

The runner supplies an empty filesystem for every test invocation. Build fixture data inside the
test and pass it to helpers as arguments. A dependency's tests cannot turn the
in-memory filesystem you gave them into access to the real one.

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
