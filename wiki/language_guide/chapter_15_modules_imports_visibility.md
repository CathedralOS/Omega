# Chapter 15: Modules, imports, and visibility

A package has a `build.omg`. Use another source file by importing its logical
filename, then select its declarations through the alias bound in this file.

## A package

```omega
machine build(plan: @mutable BuildPlan) {
    plan.product = BuildProduct::Package { name: "arithmetic_kernels" };
}
```

An executable returns `BuildProduct::Application`. A workspace returns
`BuildProduct::Workspace` with its member directories. Commit `omega.lock`: it pins dependency sources
and records what you reviewed. [Using source packages](packages.md) covers
adding and updating dependencies.

## Files and imports

Suppose the current package has a file `math/vector.omg`, and a dependency
registered as `geometry` also provides `math/vector.omg`. These imports select
one file from each root. The filename supplies the default alias `vector`.
An `as` clause lets you name the second file independently.

```omega
use math/vector.omg;
use {geometry}/math/vector.omg as geom;
use {omega}/language/core/process_authority.omg as process;
```

Write `|geom|::Point` for the type from the dependency and `|vector|::length(v)`
for a machine from the local file. Import even a sibling by its path from the
package root. A hyphenated filename needs an alias, for example
`use math/fast-vector.omg as fast;`. Importing two files as `vector` is an
error. Import order cannot change the chosen file.

A generator can write `parser.omg` into the source tree the Build plan hands
to the next compilation. A handwritten Product file can then say `use parser.omg;`
and call `|parser|::parse` like any other imported machine. Writing the file
does not put `parse` in scope or make it callable by the running generator.
[Generated files and the Product source view](../spec/build/execution.md#generated-source-boundary)

## One spelling for each owner

An imported file and a parameter can both be called `math`. In this example
both offer the same operation with the same signature. You can see which one
is selected even when taking the callable for later use.

```omega
use {geometry}/scalar/math.omg;

data Clamp = machine(x: u32, low: u32, high: u32) -> u32 reaches none crashes none;
data Policy { clamp: Clamp; }

machine agrees(math: Policy, x: u32, low: u32, high: u32) -> bool
requires low <= high
{
    let standard: u32 = |math|::clamp(x, low, high);
    let supplied: u32 = math.clamp(x, low, high);
    standard == supplied
}
```

A data type named `math` could likewise supply a case named `math::Fast`.
The import prefix applies in patterns as well:
`|conversion|::Result::Err { error }` selects a case and binds its payload.
To combine a value with an imported mask, write `flags | |wire|::MASK`.

A public machine may delegate to a helper in another file. Callers keep the
public machine’s import when you rearrange its implementation. Moving the
public declaration itself to a different file changes its import address.
The [file import sample](../../samples/cli/probes/direct_file_imports/README.md)
uses a private helper, a hyphenated filename and a callable field.

## Public and private

A declaration is private unless marked `public`:

```omega
public data Player {
    health: i32;
}

public machine take_damage(player: @mutable Player, amount: i32)
requires amount >= 0
requires player.health >= amount
{
    player.health = player.health - amount;
}
```

Fields, cases and nested `data` declarations are as visible as the type that
contains them. A nested machine is only ever visible inside its parent.

## There are no private fields

A public type's fields are public. To control who creates, reads or writes a
value, use a domain with an access policy
([Chapter 8](chapter_8_domains.md#read-and-write-authority)): it names the
machines allowed to do each, and everyone else, inside the package or out,
goes through them.

Code that only calls your operations keeps compiling when you rearrange the
fields. Code that names a field has chosen to depend on it.

## Types from a dependency's dependency

```omega
use {storage}/filesystem.omg;

|filesystem|::close(|filesystem|::open(path));
```

If `filesystem` returns a type owned by a package you do not depend on, you
can pass it straight back as shown. A generic helper can also receive it as
`T` and declare a local of type `T`. To write the owner's type name or select
its public fields or cases yourself, declare the direct dependency.

## Build helpers

For a local helper, write `use tables.omg;`. For a helper in a package that
the caller supplies as `generator`, write `use {generator}/tables.omg;`.
Either way, `|tables|::generate_tables(builder)` is an ordinary call. The
compiler resolves the import before the Build starts. The invoker supplies
the root's external libraries, and the parent supplies each child's. Product
source sees the libraries selected for the product.

A shared file can declare `use tables.omg;` before that file exists. If only
the successor calls `|tables|::generate_tables`, the first stage can write
`tables.omg` for it. If no compiled stage uses the import, the build reports
one unused-import warning at the end.
[Checked Build stages](../spec/build/scoped_execution.md#checked-build-stages)

A build helper runs on the machine doing the build, whatever the target is,
and gets no network or credential access from the build.

## Where next

[Chapter 16](chapter_16_errors_traps_failure.md) covers failure. The complete
rules are in [modules](../spec/language/modules.md),
[package boundaries](../spec/packages/boundaries.md) and
[build execution](../spec/build/execution.md).
