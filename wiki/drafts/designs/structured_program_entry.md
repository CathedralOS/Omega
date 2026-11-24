# Who supplies arguments to an Omega program?

Consider the command `editor "release notes.txt" --line 12`. A shell can supply filenames and option text. An IDE that already owns an open document may want to lend the file and pass typed options. A firmware entry receives a live system table. These callers make different promises about the values, memory and authority they hand over.

This note asks reviewers to trace each argument from its actual supplier through the [installed entry contract](../../spec/build/entry_roots.md#entry-shape-and-arrival-bridge). Delete it when an authored hosted example and a typed embedding example exercise that contract without adding privileges to a parameter.

## What the compiler and UEFI show

Build already selects a product entry with `Build::bind(plan, Target::Slot, Product::entry)`. A declaration reference tells the compiler which product code to check, emit into Psi and lower. The Build interpreter never has to execute that code or manufacture its future arguments.

At inspected revision `e500217a3254`, [the source entry checker](../../../source-rust/subsystems/compiler/source-to-psi/driver/compile_product/entry_shapes.rs) distinguishes no visible parameters from image plus initial storage. The [native signature checker](../../../source-rust/subsystems/compiler/psi-to-native/representations/program-entry-contracts/entry_signature.rs) also depends on this distinction. The generalization is to derive visible parameters from the selected entry declaration and establish each at launch. Preserve the existing parameterless hosted entry. A different function signature alone cannot change the platform contract.

UEFI shows the trust boundary. Firmware supplies an image handle and a system-table borrow to the target's physical entry. The adapter can delegate only the qualified firmware operations its contract establishes for that phase. Core's [semantic entry](../../../source/library/core/extent.omg) receives granted image and initial-storage extents. Those grants require the actual image mapping, separate storage, disjointness and exclusion of the live stack. Parameter layout and matching addresses establish none of that. The [UEFI contract](../../spec/build/uefi_entry.md#firmware-api-authority-and-storage-grants) and [implementation board](../../../TASKS.md) distinguish the required handoff from the remaining adapter and custody work.

## Follow a shell launch

1. Linux supplies argument addresses and a count on the initial process stack. The [physical entry declaration](../../../source/library/std/targets/linux_x86_64/entry.omg) names that arrival. Other platforms have their own adapters.
2. A proposed byte-oriented desktop entry can expose checked argument slices. If the adapter packs them into one slice it must specify framing and preserve empty arguments and argument boundaries. `editor "a b"` and `editor a b` are distinct. A launcher with an already serialized payload can supply that single bounded slice directly. The portable encoding and exact hosted source signature remain open.
3. The application parses the option text and checks the requested line number. No component loader, Build callback or universal options struct is involved.

**`editor/startup.omg`**, illustrative single-payload entry. For this particular profile the launcher already supplies a framed message. Parsing and errors belong to the editor.

```omega
use options.omg as options;

machine start(payload: @[u8]) {
    |options|::open_from_message(payload);
}
```

The shell example supplies no open directory argument. An app may open the filename only using filesystem authority actually available to it. Adding `project: @ReadDirectory` to this signature would require a different caller or installed contract capable of supplying that particular borrow.

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
