# Which inputs belong to a Build?

A release program, its interpreted Build, the product that Build describes, and each dependency's own Build can run different code and hold different authority. This note follows a firmware package through those contexts so reviewers can distinguish a compiler migration from the remaining host-service decision. Delete it when the migration is documented at its implementation and the owner resolves the [live-service question](../../../OWNER_QUESTIONS.md#build-network-and-subprocess-grants).

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

If a child simply needs one firmware blob, the release program can acquire it before the build and put it in the supplied filesystem. If the child owns a conditional fetch or validation algorithm, a candidate design would let the parent supply a service for one resource. The child calls it, checks returned bytes against its expected hash, and writes using its own filesystem grant. A recorded provider can return the same bytes offline. A live provider would need a limited destination and operation, a redirect rule, bounds, independently accepted policy, and an actual grant for that invocation. Accepting the bytes' hash answers an integrity question. It does not authorize the fetch.

Core network reach describes resolving a name, initiating contact, serving, sending and receiving. Passing a sender adds no network reach. Writing on a fixed-peer sender reaches `NetworkSend`. Choosing a UDP destination and sending reaches both `NetworkInitiateContact` and `NetworkSend`. Audits separately report host network stack access and possible off-machine transmission. The spec classifies these operations already. Whether Build may receive a live network provider is still open. Package retrieval uses the resolver's own authority. [Network reach](../../spec/build/platform_services.md#network-reach) · [Build restrictions](../../spec/build/execution.md#restricted-build-requests)

## Independent products and tools

A release program can run the compiler as a permitted subprocess, or import the compiler library and request builds itself. It can build the kernel and bootloader separately, give their published artifacts to a vendor packer and distribute the resulting disk image. Each producer describes its own product. Selecting a tool by filename and passing arguments cannot by itself confine which files the tool opens or whether it contacts a server. The alternative under discussion is a host service with enforceable limits on executable, inputs and outputs. The simpler release-program route already locates that authority outside dependency Build code. [Release orchestration](../../spec/build/declarations.md#releases-with-independent-products)

The same separation matters for updates. A component can be built and published without authorizing its installation anywhere. An editor's updater may later download and install it under its own runtime authority. A build selecting a runtime provider should never silently donate that provider's capabilities to its own Build code. [Hot-swap authorship](component_hot_swap_authorship.md)

## Feedback requested

Show a real dependency for which captured inputs do not suffice. Who supplies each service, what does the project approve, and where can the executor enforce the limits? Also show what the root invoker or parent actually writes to supply external Build libraries. Preserve the ordinary local-import case and avoid either self-registration or an implicit fetch during `use`.
