# Who supplies arguments to an Omega program?

Consider the command `editor "release notes.txt" --line 12`. A shell can supply filenames and option text. An IDE that already owns an open document may want to lend the file and pass typed options. A firmware entry receives a live system table. These callers make different promises about the values, memory and authority they hand over.

This note asks reviewers to trace each argument from its actual supplier through the [installed entry contract](../../spec/build/entry_roots.md#entry-shape-and-arrival-bridge). Delete it when an authored hosted example and a typed embedding example exercise that contract without adding privileges to a parameter.

## One binding, three programs

An executable's Build selects a machine for each target it supports. The target's calling policy describes where the CPU receives its inputs and returns a result. The installed entry contract describes what the caller supplies, including which memory and operations it may delegate. Build can bind the same machine for two targets, or bind different startup machines that call shared application code. The destination is known at build time. [Entry binding](../../spec/build/entry_roots.md#source-selection-is-not-invocation) · [Calling policies](../../spec/build/calling_plans.md#authorship-and-evaluation)

### A program that ignores startup arguments

These bindings use the shape in the [existing hosted sample](../../../samples/cli/probes/direct_file_imports/build.omg). Both targets can enter a parameterless machine. A library with no executable entry omits the binding.

```omega
// heartbeat/build.omg
use main.omg as app;

machine build(plan: @mutable Build) {
    Build::application(plan, "heartbeat");
    Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);
    Build::bind(plan, windows_x86_64::ProgramEntry, |app|::main);
}

// heartbeat/main.omg
public machine main() {}
```

### A CLI with two startup machines

This proposal uses the same binding with two illustrative target profiles, each with one required `ProgramEntry`. The profile and argument API names below are proposals. The `system` library would supply the platform adapters as an ordinary dependency. A project can supply its own. [Library responsibilities](../../spec/build/declarations.md#library-responsibilities-and-names)

```omega
// tool/build.omg
use startup.omg as startup;

machine build(plan: @mutable Build) {
    Build::application(plan, "tool");
    Build::bind(plan, linux_cli::ProgramEntry, |startup|::linux);
    Build::bind(plan, windows_cli::ProgramEntry, |startup|::windows);
}

// tool/startup.omg
use main.omg as app;

public machine linux(args: @LinuxProcessArguments) {
    let help = LinuxProcessArguments::contains_user_argument(args, "--help");
    |app|::main(help);
}

public machine windows() {
    let line: @[u16] = WindowsProcess::command_line();
    let help = WindowsCommandLine::contains_user_argument(line, utf16"--help");
    |app|::main(help);
}

// tool/main.omg
use {omega}/language/core/process_exit.omg as process;

public machine main(help: bool) {
    // An observable result for the example. The hosted profile supplies ProcessExit.
    transition help {
        true -> |process|::ProcessExit::exit_process(0),
        false -> |process|::ProcessExit::exit_process(1)
    }
}
```

On Linux x86-64 the kernel places `argc` and the argument pointers on the initial stack. The proposed adapter lends a read-only view of that table. The lookup skips the executable name and can compare the zero-terminated bytes in place. A helper returning an indexed byte slice would scan that argument for its terminator. A helper collecting all slices would also need backing for the descriptors. The parameterless entry needs neither operation. [Linux physical entry](../../../source/library/std/targets/linux_x86_64/entry.omg)

On Windows x86-64 the OS starts the executable without a command-line parameter. The proposed `command_line` calls `GetCommandLineW`, scans its system-owned UTF-16 buffer to the first zero code unit and lends a read-only slice of that length. The matcher uses a documented quoting rule (Microsoft C argument quoting in this example). A portable reader can expose successive arguments while preserving boundaries and empty arguments. Any conversion to Unicode text needs a policy for invalid input. A validated filename still conveys no directory or file authority. [Windows physical entry](../../../source/library/std/targets/windows_x86_64/entry.omg) · [Windows command-line buffer](https://learn.microsoft.com/en-us/windows/win32/api/processenv/nf-processenv-getcommandlinew) · [Microsoft argument quoting](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments)

| Invocation | Expected exit | What the matcher observes |
| --- | --- | --- |
| `tool --help` | 0 | One user argument equals `--help`. |
| `tool "--help me"` | 1 | One argument contains a space. |
| `tool "" --help` | 0 | The first argument is empty and the second matches. |

### A Cathedral entry that receives a live firmware table

The existing UEFI x64 physical entry receives the image handle in `RCX` and the system-table pointer in `RDX`. Its current application continuation receives two storage grants. This *alternative Cathedral profile* would let its application receive the live table under an explicit domain. The profile, the qualification `LiveFirmwareTable` and this projection into application code are proposed. [UEFI physical declaration and calling plan](../../../source/library/std/uefi_x86_64_entry.omg) · [Current UEFI continuation](../../spec/build/uefi_entry.md#physical-arrival-and-semantic-roots)

```omega
// cathedral/build.omg
use boot.omg as boot;

machine build(plan: @mutable Build) {
    Build::application(plan, "cathedral");
    Build::bind(plan, cathedral_uefi::ProgramEntry, |boot|::start);
}

// cathedral/boot.omg
use kernel.omg as kernel;

public machine start(
    image: EfiImageHandle,
    table: @EfiSystemTable in LiveFirmwareTable
) -> EfiStatus {
    |kernel|::boot(image, table)
}
```

Firmware supplies this image handle and this table on this invocation. The installed physical contract admits their provenance and lifetime. The domain's access plan names the permitted issuer with `Issuance::Only`. The checked adapter validates what it can inspect and lends that same table to `start`. The implementation of `boot` must use qualified handles for the operations it delegates. Firmware errors return under the selected calling policy. Successful OS handoff consumes Boot Services and ends access through boot-scoped handles. [Firmware authority and grants](../../spec/build/uefi_entry.md#firmware-api-authority-and-storage-grants) · [OS handoff](../../spec/build/uefi_entry.md#returning-application-versus-os-handoff)

| Input to the same `start` | Expected result |
| --- | --- |
| Qualified borrow established at the installed firmware entry | The adapter can forward it to `boot`. |
| An ordinary Omega caller already holds that qualification | It can forward its borrow within the established lifetime and firmware phase. |
| Writable scratch containing a byte-for-byte copy of the genuine table | Refuse the call. A layout check can validate fields. It cannot establish that firmware supplied this copy or delegated its operations. |

## What the compiler and UEFI show

Build already selects a product entry with `Build::bind(plan, Target::Slot, Product::entry)`. A declaration reference tells the compiler which product code to check, emit into Psi and lower. The Build interpreter never has to execute that code or manufacture its future arguments.

At inspected revision `e500217a3254`, [the source entry checker](../../../source-rust/subsystems/compiler/source-to-psi/driver/compile_product/entry_shapes.rs) distinguishes no visible parameters from image plus initial storage. The [native signature checker](../../../source-rust/subsystems/compiler/psi-to-native/representations/program-entry-contracts/entry_signature.rs) also depends on this distinction. The generalization is to derive visible parameters from the selected entry declaration and establish each at launch. Preserve the existing parameterless hosted entry. A different function signature alone cannot change the platform contract.

UEFI shows the trust boundary. Firmware supplies an image handle and a system-table borrow to the target's physical entry. The adapter can delegate only the qualified firmware operations its contract establishes for that phase. Core's [semantic entry](../../../source/library/core/extent.omg) receives granted image and initial-storage extents. Those grants require the actual image mapping, separate storage, disjointness and exclusion of the live stack. Parameter layout and matching addresses establish none of that. The [UEFI contract](../../spec/build/uefi_entry.md#firmware-api-authority-and-storage-grants) and [implementation board](../../../TASKS.md) distinguish the required handoff from the remaining adapter and custody work.

## A launcher that already has a serialized message

An IDE or launcher that owns a framed message can lend a single byte slice directly. The editor owns its decoder. Packing shell arguments into such a message would require an explicit framing step that preserves empty arguments and boundaries.

```omega
// editor/startup.omg
use options.omg as options;

machine start(payload: @[u8]) {
    |options|::open_from_message(payload);
}
```

## Follow a typed launch

**`editor/document.omg`**, a proposed shared entry shape. Both parties use the
same declarations. The IDE supplies the borrow and chooses its lifetime.

```omega
public data EditorOptions {
    first_line: u32;
    read_only: bool;
}

public boundary trait OpenDocument {
    machine open(document: @ReadableFile, options: EditorOptions)
        reaches FilesystemContentRead crashes none;
}
```

Here `ReadableFile` stands for the actual agreed file capability. A flag such
as `read_only` expresses the caller's options. Only the supplied capability and
its access rules establish which operations the receiver can perform.

An embedded IDE can select an exported entry whose shared interface takes `EditorOptions` and a document borrow. The IDE owns the file and can lend it for the checked lifetime. The receiving app gets the borrowed file and a typed line number. Across a process boundary the platform must establish a usable local handle or mediated equivalent. Copying the sender's integer or serializing a pointer cannot transfer custody. [Embedding imports and entry contracts](../../spec/build/embedding.md#imports-exports-and-provider-slots)

| Caller intent | Suitable input | What must hold |
| --- | --- | --- |
| Read a message | `@[u8]` | Preserve backing and lifetime. A typed view needs valid geometry and representation. Decoding can produce an owned editable copy. |
| Edit the caller's scratch buffer | `@mutable [u8]` | The caller owns or may lend the exact writable range exclusively. Writes affect that range and must satisfy handback obligations. |
| Delegate operations on a protected live object | Qualified value or borrow under its domain | Respect the authorized issuers, field or whole-subject writer rules, and lifetime. A raw writable view over a restricted field must reject. |

Review the [recast](../../spec/layouts/recasts.md#shared-and-mutable-views), [access-plan](../../spec/resources/access.md#access-rule-subjects) and [initial authority](../../spec/resources/authority.md#initial-call-authority) rules together. A successful decode of data cannot mint a grant.

## Feedback requested

Show one hosted entry with a caller that supplies every parameter. Where are bounds, lifetimes and mutable handback established? Show how the same project rejects a missing capability before app code executes. For the [Console example](component_hot_swap_authorship.md), identify whether the launcher owns a slot to delegate or whether the app bootstraps a fresh one. Only the former needs a slot authority argument. A new entry name or a reflected field alone answers neither question.
