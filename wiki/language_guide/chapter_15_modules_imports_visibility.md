# Chapter 15: Modules, imports, and visibility

A package is a directory with a `build.omg`. Modules organize the
declarations inside it, and `use` brings names into a file.

## A package

```omega
machine build(builder: &mut Build) {
    Build::package(builder, "arithmetic_kernels");
}
```

Use `Build::application` instead for a program, and `Build::member` to list
the members of a workspace. Commit `omega.lock`: it pins dependency sources
and records what you reviewed. [Using source packages](packages.md) covers
adding and updating dependencies.

## Modules

```omega
module dungeon::combat;
```

A module declaration goes at the top of a file. Files that name the same
module share it. `::` separates names and `.` selects fields:

```omega
let room: dungeon::rooms::Room = dungeon::rooms::create();
dungeon::rooms::describe(&room);
```

## Imports

```omega
use dungeon::combat::CombatSystem;
use dungeon::rooms::Room;
```

An import affects only the file it is in. Two imports that supply the same
name are an error; import order never picks a winner. A package you import
from must be a dependency you declared directly.

## Public and private

A declaration is private unless marked `public`:

```omega
public data Player {
    health: i32;
}

public machine take_damage(player: &mut Player, amount: i32)
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
let handle = filesystem::open(path);
filesystem::read(&handle);
filesystem::close(handle);
```

If `filesystem` returns a type owned by a package you do not depend on, you
can hold it, move it and pass it back. To name its fields or cases you need to
depend on its owner directly.

## Build helpers

A build can run code from another package, for example a table generator:

```omega
use generator;

machine build(builder: &mut Build)
suspends
{
    Build::application(builder, "generated_tables");
    let mut request = dependency::from_source(Source::Path {
        location: "../table-generator"
    });
    dependency::with_alias(&mut request, "generator");
    let registered = Build::build_dependency(builder, request);

    suspend Build::prepare_dependency(builder, registered);
    generator::generate_tables(builder);
}
```

A build helper runs on the machine doing the build, whatever the target is,
and gets no network or credential access from the build.

## Where next

[Chapter 16](chapter_16_errors_traps_failure.md) covers failure. The complete
rules are in [modules](../spec/language/modules.md),
[package boundaries](../spec/packages/boundaries.md) and
[build execution](../spec/build/execution.md).
