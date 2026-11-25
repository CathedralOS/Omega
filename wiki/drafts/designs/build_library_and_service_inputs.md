# Which inputs belong to a Build?

A release program, its interpreted Build, its product and each dependency Build have separate authority. The [program launch contract](../../spec/build/program_launch.md) settles host tool access. The [staged package input draft](../../spec/packages/staged_inputs.md) gives the proposed library handoff and identifies the remaining decisions.

## A library for whom?

The root invoker supplies the libraries its Build source can use. A parent supplies the corresponding bindings for a child before that child starts. Local `use tables.omg;` selects an existing local file. External `use {generator}/tables.omg;` selects a file through a previously supplied package alias. Neither import fetches code. The parent can also register a product dependency while it runs. That registration makes the library available to the later product compilation. A child package having its own `build.omg` says nothing about whether its parent consumes it as a Build library or a product library.

`Build::add_library` was suggested as a clearer spelling for the product operation currently called `Build::dependency`. A proposed symmetric `add_build_library` implied that running code could extend its own imports. The [fixed-import ruling](../../spec/build/scoped_execution.md#checked-build-stages) retires that behavior. Source authors can choose between two already imported helpers based on runtime data. Missing Build imports fail before the first effect. Suspending preserves the program, its storage and borrows, and its exactly-once initializers.

A generator writes ordinary files into a filesystem it holds. Stage A can write `parser.omg`. After the driver freezes the selected source view, stage B can import it using an ordinary `use`. Stage A cannot acquire new callable names by writing or registering the file. [Generated-source handoff](../../spec/build/execution.md#generated-source-boundary)

## One firmware dependency

For example, the following **`parent/build.omg`** excerpt shows the intended
service calls. Creation and read-only-view spellings are illustrative. The
child gets the view passed at registration.

```omega
let bytes = |filesystem|::read(parent_files, "fixtures/expected.sha256");
let mutable child_files = |memory_filesystem|::create();
|filesystem|::write(@mutable child_files, "expected.sha256", bytes);
let view = |filesystem|::read_only_view(child_files);
let firmware = |dependency|::from_source(Source::Path { location: "deps/firmware" });
Build::dependency(plan, |dependency|::with_filesystem(firmware, view));
```

Give a child a filesystem by configuring its dependency description before registration. For a confined input the parent reads the file it already has, creates an empty memory filesystem, writes the bytes at the name the child expects, then supplies that view. Those are distinct operations. A single helper called “private tree with input” hid which file was read, which store owned the result, and who could change it. Editing a Build plan cannot widen a child's host access. Sibling grants stay separate. [Filesystem delegation](../../spec/build/scoped_execution.md#supplying-a-filesystem-to-a-child-build)

For a pinned firmware blob, an authorized earlier stage can capture the bytes
and give the child an offline filesystem view. A child that must fetch live
instead needs an admitted provider and its parent's explicit delegation.
The child can verify the expected digest in either case.
[Build network grants](../../spec/build/scoped_execution.md#live-networking-in-a-build)

## Independent products and tools

A release program can run the compiler as a permitted subprocess, or import the compiler library and request builds itself. It can build the kernel and bootloader separately, give their published artifacts to a vendor packer and distribute the resulting disk image. Each producer describes its own product. Selecting a tool by filename and passing arguments cannot by itself confine which files the tool opens or whether it contacts a server. The caller can instead bind the executable to the Build under an enforceable [program grant](../../spec/build/program_launch.md). The simpler release-program route already locates that authority outside dependency Build code. [Release orchestration](../../spec/build/declarations.md#releases-with-independent-products)

The same separation matters for updates. A component can be built and published without authorizing its installation anywhere. An editor's updater may later download and install it under its own runtime authority. A build selecting a runtime provider should never silently donate that provider's capabilities to its own Build code. [Hot-swap authorship](component_hot_swap_authorship.md)

## Feedback requested

Show a real dependency for which captured inputs do not suffice. Who supplies each service, what does the project approve, and where can the executor enforce the limits? Also show what the root invoker or parent actually writes to supply external Build libraries. Preserve the ordinary local-import case and avoid either self-registration or an implicit fetch during `use`.
