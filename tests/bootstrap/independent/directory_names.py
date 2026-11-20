"""Check package-relative namespaces using an executable Epsilon-written Omega constructor."""
import argparse
from pathlib import Path
import runpy
import struct
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compiled import ROOT, add_invocation_budget, execute, stamp


def cases():
    encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]
    select = b"use first\\score; machine main() -> u8 { score() }"
    sibling = b"machine main() -> u8 { score() }"
    constant = b"machine main() -> u8 { 42 }"
    score = b"public machine score() -> u8 { 42 }"
    other = b"public machine score() -> u8 { 7 }"
    array_caller = b"use later\\read; machine main()->u8{let x:[u8;2]=[1,42];read(@x)}"
    array_reader = b"public machine read(x:@[u8;2])->u8{x[1]}"
    cases_source = (ROOT / "tests/bootstrap/conformance/omega/payload_bindings.omg").read_bytes()

    def packet(sources, paths=None):
        data, _, _, rows = encode([("app", 0)], [], [(0, src) for src in sources],
                                  0, b"main", paths=paths)
        return data, rows

    # A callee in another file owns its parameter's element type and count.
    paths = ["main.omg", "later/api.omg"]
    wire, _ = packet([array_caller, array_reader], paths)
    yield "shared-array-forward", wire, 42
    wire, rows = packet([array_caller, array_reader.replace(b"[u8;2]", b"[u8;3]")], paths)
    borrow = rows[0][1] + array_caller.index(b"@x")
    yield "shared-array-wrong-extent", wire, struct.pack("<BII", 0, 1, borrow)

    for name, sources, paths in [
        ("directory-import", [select, score, other], ["main.omg", "first/x.omg", "second/x.omg"]),
        ("renamed-and-reordered", [select, other, score], ["new.omg", "second/a.omg", "first/b.omg"]),
        ("same-directory", [sibling, other, score], ["first/m.omg", "second/s.omg", "first/s.omg"]),
        ("qualified-call", [b"machine main() -> u8 { first\\score() }", score],
         ["main.omg", "first/x.omg"]),
        ("nested-directory", [select.replace(b"first", b"outer\\first"), score],
         ["main.omg", "outer/first/x.omg"]),
        ("static-case", [cases_source], ["example/cases.omg"]),
        ("legacy-root", [sibling, score], None),
    ]:
        yield name, packet(sources, paths)[0], 42

    data, _, _, _ = encode([("app", 0), ("dep", 0)], [(0, 1, "library")],
                           [(0, select.replace(b"first", b"library\\first")), (1, score), (1, other)],
                           0, b"main", paths=["main.omg", "first/x.omg", "second/x.omg"])
    yield "dependency-directory", data, 42

    # The installed core is identified by its role, independent of its package
    # display name. Exercise the product declaration without a dependency edge.
    ordering = (ROOT / "source/library/core/fence_ordering.omg").read_bytes()
    order_reader = (
        b"use omega\\language\\core\\FenceOrdering; machine main() -> u8 { "
        b"let order: FenceOrdering = FenceOrdering::ReceivePublish; "
        b"transition order { FenceOrdering::ReceivePublish -> pass(), _ -> fail() } "
        b"machine pass() -> u8 { 42 } machine fail() -> u8 { 7 } }"
    )
    data, _, _, _ = encode([("app", 0), ("installed", 1)], [],
                           [(0, order_reader), (1, ordering)], 0, b"main",
                           paths=["main.omg", "fence_ordering.omg"])
    yield "core-product-declaration", data, 42

    core_import = b"use omega\\language\\core\\sub\\score; machine main() -> u8 { score() }"
    core_call = b"machine main() -> u8 { omega\\language\\core\\sub\\score() }"
    packages = [("app", 0), ("installed", 1), ("impostor", 0)]
    paths = ["main.omg", "sub/x.omg", "language/core/sub/x.omg"]
    for name, caller in [("core-import", core_import), ("core-qualified-call", core_call)]:
        data, _, _, _ = encode(packages, [(0, 2, "omega")], [(0, caller), (1, score), (2, other)],
                               0, b"main", paths=paths)
        yield name, data, 42

    local_alias = b"use alternate\\omega; " + core_call
    data, _, _, _ = encode([("app", 0), ("installed", 1)], [],
                           [(0, local_alias), (1, score), (0, other), (0, b"machine anchor() {}")],
                           0, b"main", paths=["main.omg", "sub/x.omg",
                                              "alternate/omega/language/core/sub/x.omg",
                                              "alternate/omega/marker.omg"])
    yield "core-before-local-namespace", data, 42

    # A normal dependency alias starting with "omega" still selects ordinary std.
    ordinary = b"use omega\\sub\\score; machine main() -> u8 { score() }"
    data, _, _, _ = encode([("app", 0), ("library", 2)], [(0, 1, "omega")],
                           [(0, ordinary), (1, score)], 0, b"main",
                           paths=["main.omg", "sub/x.omg"])
    yield "ordinary-omega-alias", data, 42

    for name, role, path, caller, declaration, owner, edges in [
        ("private-core-member", 1, "sub/x.omg", core_import, b"machine score() -> u8 { 42 }", 1, []),
        ("core-name-without-role", 0, "sub/x.omg", core_import, score, 1, []),
        ("local-core-impostor", 0, "omega/language/core/sub/x.omg", core_import, score, 0, []),
        ("aliased-core-impostor", 0, "language/core/sub/x.omg", core_import, score, 1, [(0, 1, "omega")]),
        ("ordinary-std-unmounted", 2, "sub/x.omg", core_import.replace(b"core", b"std"), score, 1, []),
    ]:
        data, _, _, rows = encode([("app", 0), ("omega_language_core", role)], edges,
                                  [(0, caller), (owner, declaration)], 0, b"main",
                                  paths=["main.omg", path])
        yield name, data, struct.pack("<BII", 0, 1, rows[0][1])

    data, rows = packet([select, score], ["main.omg", "moved/x.omg"])
    yield "moved-declaration", data, struct.pack("<BII", 0, 1, rows[0][1])
    # The bootstrap compiler implements imports of free declarations. A static
    # member selector must not select a same-spelled free machine in a directory.
    data, rows = packet([select.replace(b"first\\score", b"first::score"), score],
                        ["main.omg", "first/x.omg"])
    yield "static-import-outside-subset", data, struct.pack("<BII", 0, 2, rows[0][1] + 4)
    data, rows = packet([b"module first; " + constant], ["m.omg"])
    yield "retired-header", data, struct.pack("<BII", 0, 1, rows[0][1])
    for name, path, offset in [
        ("bad-directory", "ok/2wrong/x.omg", 3),
        ("reserved-directory", "u3072/x.omg", 0),
        ("empty-component", "ok//x.omg", 3),
        ("missing-basename", "ok/", 3),
        ("parent-basename", "ok/..", 3),
        ("nul-in-name", "ok/a\0b", 4),
        ("backslash-in-path", "ok\\x.omg", 2),
    ]:
        data, rows = packet([constant], [path])
        yield name, data, struct.pack("<BII", 0, 1, rows[0][3] + offset)
    data, _ = packet([constant], ["a" * 256 + "/x.omg"])
    yield "last-directory-byte", data, 42
    data, _ = packet([constant], ["a" * 257 + "/x.omg"])
    yield "directory-too-long", data, struct.pack("<BI", 0, 3)
    data, _ = packet([constant], ["a" * 300 + ".omg"])
    yield "long-basename", data, 42
    yield "truncated-frame", data[:-1], struct.pack("<BI", 0, 3)
    yield "unknown-version", bytes(4) + struct.pack("<I", 2), struct.pack("<BI", 0, 3)


    yield from nominal_record_cases(encode)
    yield from record_bound_cases(encode)
    yield from nominal_sum_cases(encode)
    yield from nominal_array_cases(encode)
    yield from nested_state_cases(encode)
    yield from nested_parameter_cases(encode)


def copy_declaration(definitions, name):
    return declaration(definitions, b"public data " + name + b" [copy] {")


def declaration(definitions, header):
    start = definitions.index(header)
    return definitions[start:definitions.index(b"\n}", start) + 2]


def nominal_record_cases(encode):
    # Keep the actual product declarations, including their integer domains.
    definitions = (ROOT / "source/subsystems/compiler/source_to_psi/representations/source_files/source.omg").read_bytes()

    caller = b"""use packets\\SourceSpan; use packets\\SourceId; use packets\\Span; use packets\\pass;
machine main()->u8 {
    let id:SourceId=SourceId{value:42 as +Trapping};
    let start:u64 in Trapping=7 as u64 +Trapping;
    let range:Span=Span{start:start,end:start};
    let combined:SourceSpan=SourceSpan{source:id,span:range};
    let y:SourceSpan=pass(combined);
    let out:SourceId=y.source;
    let r:Span=y.span;
    transition ((out.value as -Trapping)==42) && ((r.start as -Trapping)==7) {true->42,_->7}
}"""
    relay = b"public machine pass(x:SourceSpan)->SourceSpan{x}"
    units = [(0, caller), (1, relay), (1, copy_declaration(definitions, b"SourceSpan")),
             (1, copy_declaration(definitions, b"Span")), (1, copy_declaration(definitions, b"SourceId"))]
    names = ["main.omg", "pass.omg", "combined.omg", "span.omg", "source_id.omg"]
    packet, _, _, _ = encode([("app", 0), ("packets", 0)], [(0, 1, "packets")],
                             units, 0, b"main", paths=names)
    yield "forward-product-records", packet, 42

    # The caller can import the public function. Its return type is private.
    caller = b"use packets\\pass; machine main()->u8 {42}"
    relay = b"public machine pass(x:Packet)->Packet{x}"
    packet, _, _, rows = encode(
        [("app", 0), ("packets", 0)], [(0, 1, "packets")],
        [(0, caller), (1, relay), (1, b"data Packet [copy] {value:u8;}")], 0, b"main",
        paths=["main.omg", "pass.omg", "declaration.omg"],
    )
    yield "private-type-in-foreign-signature", packet, struct.pack("<BII", 0, 1, rows[1][1])

    first = b"machine main()->u8 {42}data A [copy]{b:B;}"
    second = b"data B [copy] {a:A;}"
    packet, _, _, rows = encode([("app", 0)], [], [(0, first), (0, second)], 0, b"main",
                                paths=["a.omg", "b.omg"])
    coordinate = rows[0][1] + first.index(b"data A")
    yield "cross-file-record-cycle", packet, struct.pack("<BII", 0, 2, coordinate)


    # Both Part declarations have the same fields. Only the owner selected
    # by Outer's own import can initialize its field.
    caller = (b"use types\\Outer;use types\\pass;use left\\Part;machine main()->u8{"
              b"let p:Part=Part{value:42};let x:Outer=Outer{part:p};let y:Outer=pass(x);"
              b"let saved:Part=y.part;saved.value}")
    relay = b"public machine pass(x:Outer)->Outer{x}"
    part = b"public data Part [copy]{value:u8;}"
    paths = ["main.omg", "types/pass.omg", "types/outer.omg", "left/part.omg", "right/part.omg"]
    for module in (b"left", b"right"):
        owner = b"use " + module + b"\\Part;public data Outer [copy]{part:Part;}"
        units = [(0, source) for source in (caller, relay, owner, part, part)]
        packet, _, _, rows = encode([("app", 0)], [], units, 0, b"main", paths=paths)
        if module == b"left":
            yield "field-declaring-scope", packet, 42
        else:
            coordinate = rows[0][1] + caller.index(b"machine main")
            yield "different-nominal-field-owner", packet, struct.pack("<BII", 0, 1, coordinate)


def record_bound_cases(encode):
    # Each length belongs to a different declaration. The enclosing record's
    # own bound must survive laying out both of its forward-declared fields.
    source = (ROOT / "source/subsystems/compiler/source_to_psi/representations/source_files/source.omg").read_bytes()
    source_unit = declaration(source, b"public data SourceUnit\n")
    module_path = declaration(source, b"public data ModulePath\n")
    source_id = declaration(source, b"public data SourceId ")
    buffers = b"""public data Buffers
requires marker <= 7,
{
    unit:SourceUnit;
    path:ModulePath;
    marker:u64;
}"""
    caller = b"""use types\\Buffers;
machine main()->u8{
    let mut pair:Buffers;
    pair.unit.length=131072;
    pair.path.length=256;
    pair.marker=7;
    let remaining:u64=131072-pair.unit.length;
    let free_path:u64=256-pair.path.length;
    let room:u64=7-pair.marker;
    transition remaining==0 && free_path==0 && room==0 {true->42,_->1}
}"""

    def packet(main=caller, record=buffers, unit=source_unit):
        files = [(0, main), (1, record), (1, module_path), (1, unit), (1, source_id)]
        return encode([("app", 0), ("types", 0)], [(0, 1, "types")], files, 0, b"main",
                      paths=["main.omg", "buffers.omg", "path.omg", "unit.omg", "id.omg"])

    wire, _, _, _ = packet()
    yield "product-record-bound-owners", wire, 42
    for label, store, excess in [
        ("source-unit", b"length=131072", b"length=131073"),
        ("module-path", b"length=256", b"length=257"),
        ("enclosing-record", b"marker=7", b"marker=8"),
    ]:
        wire, _, _, rows = packet(main=caller.replace(store, excess))
        yield f"{label}-bound-exceeded", wire, struct.pack("<BII", 0, 2, rows[0][1] + caller.index(b"machine main"))

    wire, _, _, rows = packet(record=buffers.replace(b"marker <= 7", b"length <= 7"))
    yield "field-bound-names-another-record", wire, struct.pack("<BII", 0, 2, rows[1][1])
    wire, _, _, rows = packet(unit=source_unit.replace(b"length <= 131072", b"length >= 131072"))
    yield "unsupported-later-record-predicate", wire, struct.pack("<BII", 0, 2, rows[3][1])


    # SourceSet carries five independent field bounds, including its four-MiB
    # arena. A valid full byte arena cannot establish capacity in the path arena.
    files = [declaration(source, b"public data SourceSet\n")]
    files.extend(copy_declaration(source, name) for name in [b"DependencyRow", b"PackageRow", b"SourceRange", b"DependencyScope"])
    reader = b"""use inputs\\SourceSet;
machine main()->u8{
 let mut sources:SourceSet;
 sources.unit_count=512;
 sources.byte_count=4194304;
 sources.path_byte_count=16384;
 sources.package_count=64;
 sources.dependency_count=256;
 sources.bytes[4194303]=42;
 transition 512-sources.unit_count==0 && 4194304-sources.byte_count==0 &&
   16384-sources.path_byte_count==0 && 64-sources.package_count==0 && 256-sources.dependency_count==0 {
   true->sources.bytes[4194303],_->1
 }}"""
    for label, program in [("source-set-field-bounds", reader),
                           ("source-set-path-bound-exceeded", reader.replace(b"path_byte_count=16384", b"path_byte_count=16385"))]:
        wire, _, _, rows = encode([("app", 0), ("inputs", 0)], [(0, 1, "inputs")],
                                  [(0, program)] + [(1, data) for data in files], 0, b"main",
                                  paths=["main.omg", "sources.omg", "dep.omg", "pack.omg", "range.omg", "scope.omg"])
        if program == reader:
            yield label, wire, 42
        else:
            yield label, wire, struct.pack("<BII", 0, 2, rows[0][1] + reader.index(b"machine main"))


def nominal_sum_cases(encode):
    def packet(sources, paths, library="numeric"):
        units = [(0, sources[0])] + [(1, source) for source in sources[1:]]
        result, _, _, rows = encode([("app", 0), (library, 0)], [(0, 1, library)],
                                    units, 0, b"main", paths=paths)
        return result, rows

    numeric = (ROOT / "source/subsystems/compiler/source_to_psi/representations/tokens/numeric.omg").read_bytes()
    main = (b"use numeric\\NumericBase;use numeric\\IntegerSuffix;use numeric\\classify;"
            b"machine main()->u8{let b:NumericBase=NumericBase::Hexadecimal;"
            b"let s:IntegerSuffix=IntegerSuffix::U8;classify(b,s)}")
    classify = (b"public machine classify(base:NumericBase,suffix:IntegerSuffix)->u8{"
                b"transition base{NumericBase::Hexadecimal->hex(suffix),_->0}"
                b"machine hex(value:IntegerSuffix)->u8{transition value{IntegerSuffix::U8->42,_->0}}}")
    data, _ = packet([main, classify, numeric], ["main.omg", "classify.omg", "numeric.omg"])
    yield "forward-product-numeric-cases", data, 42

    # Exercise the exact return alternatives that blocked the original
    # resolver, retaining all of ValueBinding's other cases and payload types.
    definitions = (ROOT / "source/subsystems/compiler/source_to_psi/representations/symbol_resolved_trees/symbol_resolved_trees.omg").read_bytes()

    main = (b"use psi\\ValueBinding;use psi\\select;use psi\\observe;machine main()->u8{"
            b"let h:u64 in Trapping=42 as u64 +Trapping;let yes:ValueBinding=select(true,h);"
            b"let no:ValueBinding=select(false,h);transition observe(yes)==42&&observe(no)==0{true->42,_->7}}")
    select = (b"public machine select(flag:bool,handle:u64 in Trapping)->ValueBinding{transition flag{"
              b"false->ValueBinding::Unresolved,_->ValueBinding::Machine{handle:handle}}}")
    observe = (b"public machine observe(value:ValueBinding)->u8{transition value{"
               b"ValueBinding::Machine{handle as h}->check(h),_->0}machine check(h:u64 in Trapping)->u8{"
               b"transition (h as -Trapping)==42{true->42,_->7}}}")
    names = (b"ValueBinding", b"VisibleOverloads", b"BytePredicate", b"LayoutQuery")
    sources = [main, select, observe] + [copy_declaration(definitions, name) for name in names]
    paths = ["main.omg", "select.omg", "observe.omg"] + [name.decode() + ".omg" for name in names]
    data, _ = packet(sources, paths, "psi")
    yield "forward-product-value-binding", data, 42

    main = (b"use numeric\\Packet;use numeric\\Hit;use numeric\\score;machine main()->u8{"
            b"let p:Packet=Packet{value:42};let h:Hit=Hit::One{item:p};score(h)}")
    score = b"public machine score(h:Hit)->u8{transition h{Hit::One{item as answer}->answer.value,_->0}}"
    hit = b"public data Hit [copy]{case Miss;case One(item:Packet);}"
    record = b"public data Packet [copy]{value:u8;}"
    paths = ["main.omg", "score.omg", "hit.omg", "packet.omg"]
    data, _ = packet([main, score, hit, record], paths)
    yield "forward-nominal-case-payload", data, 42

    data, rows = packet([b"use numeric\\score;machine main()->u8{42}", score, hit,
                         record.replace(b"public data", b"data")], paths)
    coordinate = rows[2][1] + hit.index(b"item:Packet")
    yield "private-type-in-case-payload", data, struct.pack("<BII", 0, 1, coordinate)



def nominal_array_cases(encode):
    def packet(sources, paths, library=None):
        packages = [("app", 0)]
        dependencies = []
        if library:
            packages.append((library, 0))
            dependencies.append((0, 1, library))
        return encode(packages, dependencies, sources, 0, b"main", paths=paths)

    # Compile the unedited declaration and reach both ends of its four-MiB field.
    definitions = (ROOT / "source/application/construction/main.omg").read_bytes()
    storage = declaration(definitions, b"data ConstructionInput {")
    caller = b"""machine main()->u8{
    let mut a:ConstructionInput;
    a.unit_count=305;
    a.bytes[4194303]=255;
    a.bytes[4194302]=41;
    a.bytes[0]=42;
    transition a.unit_count==305 && a.bytes[0]==42 && a.bytes[4194302]==41 && a.bytes[4194303]==255 {true->42,_->7}
}"""
    data, _, _, _ = packet([(0, caller), (0, storage)], ["caller.omg", "input.omg"])
    yield "forward-product-construction-array", data, 42

    definitions = (ROOT / "source/subsystems/compiler/alpha/emit.omg").read_bytes()
    emitter = declaration(definitions, b"public data AlphaEmitter {")
    caller = b"""use alpha\\AlphaEmitter;machine main()->u8{
    let mut a:AlphaEmitter;
    a.bytes[2097151]=42;
    a.labels[10239]=73;
    a.marks[65535]=99;
    transition a.labels[10239]==73 && a.marks[65535]==99 {true->a.bytes[2097151],_->7}
}"""
    data, _, _, _ = packet([(0, caller), (1, emitter)], ["caller.omg", "emit.omg"], "alpha")
    yield "forward-product-emitter-arrays", data, 42

    storage = b"public data Packet{bytes:[u8;9];tail:u64;}"
    length = b"public machine length(bytes:@[u8])->u64{bytes.len}"
    for name, end in [("borrow-retained-byte-array", b"9"), ("borrow-past-retained-byte-array", b"10")]:
        main = (b"use wire\\Packet;use wire\\length;machine main()->u8{let mut p:Packet;p.bytes[8]=42;p.tail=77;"
                b"let n:u64=length(@p.bytes[7.." + end + b"]);"
                b"transition n==2&&p.tail==77{true->p.bytes[8],_->7}}")
        data, _, _, rows = packet([(0, main), (1, length), (1, storage)], ["main.omg", "length.omg", "packet.omg"], "wire")
        expected = 42 if end == b"9" else struct.pack("<BII", 0, 1, rows[0][1] + main.index(b".."))
        yield name, data, expected

    # Two directories expose the same spelling and layout. The field's owner
    # file chooses the element identity and declares the indexable extent.
    caller = b"""use types\\Shelf;use left\\Part;machine main()->u8{
    let mut s:Shelf;
    let p:Part=Part{byte:42};
    s.parts[2]=p;
    let saved:Part=s.parts[2];
    saved.byte
}"""
    paths = ["main.omg", "types/shelf.omg", "left/part.omg", "right/part.omg"]
    part = b"public data Part [copy]{byte:u8;}"
    for name, module, length in [
        ("array-field-declaring-scope", b"left", b"3"),
        ("different-nominal-array-owner", b"right", b"3"),
        ("wrong-array-extent", b"left", b"2"),
    ]:
        shelf = b"use " + module + b"\\Part;public data Shelf{parts:[Part;" + length + b"];}"
        data, _, _, rows = packet([(0, caller), (0, shelf), (0, part), (0, part)], paths)
        if name == "array-field-declaring-scope":
            yield name, data, 42
        else:
            yield name, data, struct.pack("<BII", 0, 1, rows[0][1] + caller.index(b"machine main"))

    caller = b"use types\\Shelf;machine main()->u8{42}"
    shelf = b"use left\\Part;public data Shelf{parts:[Part;3];}"
    data, _, _, rows = packet([(0, caller), (0, shelf), (0, part.replace(b"public ", b"")), (0, part)], paths)
    yield "private-array-element", data, struct.pack("<BII", 0, 1, rows[1][1] + shelf.index(b"parts"))




def nested_parameter_cases(encode):
    # The caller's signed -1 must satisfy its one-sided bound and survive
    # the callee's 7. Other files reuse the same slot for unsigned parameters.
    caller = (b'machine main()->u8{transition{_->relay(-1)}'
              b'\n    machine relay(kept:i32)->u8 requires kept<=0 {let observed:u8=other(7);'
              b'transition kept==-1{true->42,_->observed}}}')
    callee = b'machine other(input:u8)->u8{transition{_->local(input)}machine local(value:u8)->u8{value}}'
    sources = [caller, callee]
    parameters = ','.join(f'p{i}:u8' for i in range(16))
    for unit in range(64):
        machines = [f'machine state{i}({parameters})->u8{{p0}}' for i in range(16)]
        sources.append(('\n'.join([f'machine group{unit}()->u8{{0', *machines, '}'])).encode())
    paths = [f'file{i}.omg' for i in range(len(sources))]
    for bad in (False, True):
        units = sources.copy()
        if bad:
            prefix, body = units[-1].rsplit(b'{p0}', 1)
            units[-1] = prefix + b'{other_slot}' + body
        packet, _, _, rows = encode([('app', 0)], [], [(0, unit) for unit in units],
                                    0, b'main', paths=paths)
        if bad:
            position = rows[-1][1] + units[-1].index(b'other_slot')
            yield 'nested-parameters-last-reference', packet, struct.pack('<BII', 0, 1, position)
        else:
            yield 'nested-parameters-across-files', packet, 42


def nested_state_cases(encode):
    # Every nested state is entered on the success path. A file has only its
    # own 63 or 64 nested names; the 64 root names are visible between files.
    groups = 64
    steps = 63
    for name in ("over-state-directory", "nested-state-wrong-owner", "broken-last-state"):
        sources = []
        for group in range(groups):
            root = "main" if group == 0 else f"hop{group}"
            lines = [f"public machine {root}()->u8{{transition{{_->s0()}}"]
            for step in range(steps):
                if step + 1 < steps:
                    result = f"transition{{_->s{step + 1}()}}"
                elif group + 1 == groups:
                    result = "transition{_->only_final()}"
                elif name == "nested-state-wrong-owner" and group + 2 == groups:
                    result = "only_final()"
                else:
                    result = f"hop{group + 1}()"
                lines.append(f"machine s{step}()->u8{{{result}}}")
            if group + 1 == groups:
                value = "unknown_late()" if name == "broken-last-state" else "42"
                lines.append(f"machine only_final()->u8{{{value}}}")
            lines.append("}")
            sources.append("\n".join(lines).encode())
        paths = [f"rung{group}.omg" for group in range(groups)]
        data, _, _, rows = encode([("app", 0)], [], [(0, s) for s in sources], 0, b"main", paths=paths)
        if name == "over-state-directory":
            yield name, data, 42
        else:
            group, missing = (groups - 2, b"only_final()") if name == "nested-state-wrong-owner" else (groups - 1, b"unknown_late()")
            coordinate = rows[group][1] + sources[group].index(missing)
            yield name, data, struct.pack("<BII", 0, 1, coordinate)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("constructor", type=Path)
    parser.add_argument("--case", action="append", choices=[name for name, _, _ in cases()])
    add_invocation_budget(parser)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        for name, request, expected in cases():
            if args.case and name not in args.case:
                continue
            status, output = execute(args.constructor.resolve(), request, args.seconds)
            if status:
                raise SystemExit(f"{name}: constructor exited {status}")
            if isinstance(expected, int):
                if output[:5] != bytes(5) or len(output) <= 5:
                    raise SystemExit(f"{name}: expected Alpha tape, got {output[:40].hex()}")
                tape, program = work / (name + ".tape"), work / name
                tape.write_bytes(output[5:])
                stamp(tape, program)
                if execute(program, timeout=args.seconds) != (expected, b""):
                    raise SystemExit(f"{name}: Alpha program did not exit {expected} with empty output")
            elif output != expected:
                raise SystemExit(f"{name}: got {output.hex()}, expected {expected.hex()}")
            print(f"{name} passed", flush=True)


if __name__ == "__main__":
    main()
