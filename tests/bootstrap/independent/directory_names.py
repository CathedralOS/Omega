"""Check package-relative namespaces using an executable Epsilon-written Omega constructor."""
import argparse
from pathlib import Path
import re
import runpy
import struct
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compiled import ROOT, add_invocation_budget, execute, stamp


def cases():
    encode = runpy.run_path(str(ROOT / "source/application/construction/construct.py"))["encode"]
    # Only the selected file can supply answer. A sibling with the same name
    # remains present when the selected declaration is renamed.
    file_caller = b"use two.omg;machine main()->u8{|two|::answer()}"
    sibling_answer = b"public machine answer()->u8{42}"
    for name, declaration_name in [("exact-file-member", b"answer"), ("missing-file-member", b"other")]:
        selected_source = b"public machine " + declaration_name + b"()->u8{7}"
        wire, _, _, rows = encode([("app", 0)], [], [(0, file_caller), (0, sibling_answer), (0, selected_source)],
                                  0, b"main", paths=["main.omg", "one.omg", "two.omg"])
        expected = 7 if declaration_name == b"answer" else struct.pack(
            "<BII", 0, 1, rows[0][1] + file_caller.index(b"|two|"))
        yield name, wire, expected

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

    yield from data_lifetime_cases(packet)
    yield from trait_static_parameter_cases(packet)
    yield from invokes_cases(packet)
    yield from quoted_byte_cases(packet)

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

    # A normal dependency alias starting with "omega" still selects ordinary system.
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
        ("ordinary-system-unmounted", 2, "sub/x.omg", core_import.replace(b"core", b"system"), score, 1, []),
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


    # A machine can return an expression before it declares its helpers.
    # Executable statements cannot resume after the first nested declaration.
    outer = b"machine main()->u8{relay(42) machine unused()->u8{0}"
    relay = b" machine relay(value:u8)->u8{value machine unused()->u8{0}}"
    data, _ = packet([outer + b"}" + relay], ["main.omg"])
    yield "result-before-nested-machine", data, 42
    data, rows = packet([outer + b" 7}" + relay], ["main.omg"])
    yield "statement-after-nested-machine", data, struct.pack("<BII", 0, 1, rows[0][1] + len(outer) + 1)

    yield from construction_unit_cases(encode)
    yield from nominal_record_cases(encode)
    yield from record_bound_cases(encode)
    yield from parameter_order_cases(encode)
    yield from record_reference_cases(encode)
    yield from evaluated_guard_cases(encode)
    yield from nominal_sum_cases(encode)
    yield from sum_equality_cases(encode)
    yield from storage_window_cases(encode)
    yield from nominal_array_cases(encode)
    yield from nested_state_cases(encode)
    yield from nested_parameter_cases(encode)
    yield from predicate_transition_cases(encode)
    yield from private_arrival_cases(encode)
    yield from slice_conjunction_cases(encode)
    yield from slice_successor_cases(encode)


def copy_declaration(definitions, name):
    return declaration(definitions, b"public data " + name + b" [copy] {")


def invokes_cases(packet):
    # A gated package cannot drop an entry ceiling, even on an unused machine.
    for name, source, anchor, status in [
        ("invokes-unchecked-entry", b"machine main()->u8 invokes Entry {42}", b"invokes", 2),
        ("invokes-service-twin", b"boundary trait Entry {} machine main()->u8 invokes Entry {42}", b"invokes", 2),
        ("invokes-unused-entry", b"machine helper() invokes Entry {} machine main()->u8{42}", b"invokes", 2),
        ("invokes-missing-binding", b"machine main()->u8 invokes {42}", b"{42}", 1),
        ("invokes-binding-at-eof", b"machine main()->u8 invokes", b"", 1),
        ("invokes-non-name", b"machine main()->u8 invokes 42 {42}", b"42", 1),
        ("invokes-data-owner", b"data D invokes Bad {} machine main()->u8{42}", b"invokes", 1),
        ("invokes-case-owner", b"data D {case One invokes Bad;} machine main()->u8{42}", b"invokes", 1),
        ("invokes-domain-owner", b"domain u8::D invokes Bad; machine main()->u8{42}", b"invokes", 1),
    ]:
        wire, rows = packet([source], ["main.omg"])
        position = source.index(anchor) if anchor else len(source)
        yield name, wire, struct.pack("<BII", 0, status, rows[0][1] + position)

    # The same spelling can name an ordinary local, and a service can still
    # form a reach ceiling when the program makes no synchronous entry claim.
    for name, source in [
        ("invokes-contextual-name", b"machine main()->u8{let invokes:u8=42; invokes}"),
        ("invokes-reaches-twin", b"boundary trait Entry {} machine main()->u8 reaches Entry {42}"),
    ]:
        yield name, packet([source], ["main.omg"])[0], 42

    # Preserve the original four authored wait names. Supplying their imports
    # as memberless services isolates checking of the Clock requirement.
    clock = (ROOT / "source/library/system/clock.omg").read_bytes()
    services = [("linux_arm64", "LinuxArmClockWait"), ("linux_x86_64", "LinuxClockWait"),
                ("macos_arm64", "DarwinClockWait"), ("windows_x86_64", "WindowsClockWait")]
    sources = [b"machine main()->u8{42}", clock,
               *(f"public boundary trait {entry} {{}}".encode() for _, entry in services)]
    paths = ["main.omg", "clock.omg", *(f"targets/{target}/wait.omg" for target, _ in services)]
    wire, rows = packet(sources, paths)
    yield "invokes-production-clock", wire, struct.pack("<BII", 0, 2, rows[1][1] + clock.index(b"boundary trait Clock"))


def quoted_byte_cases(packet):
    # Read through the descriptor at runtime, including an empty view with no byte to read.
    first = (b"machine first(s:@[u8])->u8{transition s.length>0{true->yes(s),_->0}"
             b"machine yes(s:@[u8])->u8 requires s.length>0{s[0]}}")
    for name, literal, expected in [
        ("ascii", b'"."', 46), ("empty", b'""', 0), ("nul", br'"\0"', 0),
        ("tab", br'"\t"', 9), ("hex", br'"\xFF"', 255), ("utf8", '"é"'.encode(), 195),
        ("escaped-quote", br'"\""', 34), ("backslash", br'"\\"', 92),
        ("carriage-return", br'"\r"', 13), ("newline", br'"\n"', 10),
    ]:
        source = b"machine main()->u8{first(" + literal + b")}" + first
        yield "quoted-image-" + name, packet([source], ["main.omg"])[0], expected

    caller = b'machine main()->u8{read("A")}'
    later = first + b'machine read(s:@[u8])->u8{first("B")}'
    yield "quoted-image-cross-unit", packet([caller, later], ["main.omg", "later.omg"])[0], 66
    index = b"machine read(s:@[u8],i:u64)->u8 requires i<s.length{s[i]}"
    yield "quoted-image-required-length", packet([b'machine main()->u8{read("ab",1)}' + index], ["main.omg"])[0], 98

    # Both complete product helpers use their original contracts and nested machines.
    definitions = (ROOT / "source/application/construction/build_sources.omg").read_bytes()
    helpers = (declaration(definitions, b"machine same_build_path(") + b"\n\n" +
               declaration(definitions, b"machine build_path_valid(") + b"\n")
    for name, path, yes in [("valid-path", b"alpha/build.omg", True),
                            ("traversal", b"alpha/../build.omg", False),
                            ("empty-component", b"alpha//build.omg", False)]:
        arms = b"true -> 42, _ -> 7" if yes else b"true -> 7, _ -> 42"
        main = b'machine main() -> u8 crashes Trap { transition build_path_valid("' + path + b'") { ' + arms + b" } }"
        yield "quoted-image-original-" + name, packet([helpers + main], ["main.omg"])[0], 42

    for name, source, at, status in [
        ("short", b'machine main()->u8{read("ab",2)}' + index, b"machine main", 2),
        ("invalid-escape", br'machine main()->u8{first("\q")}' + first, br"\q", 1),
        ("immutable", b'data Cell{value:u8;}machine main()->u8{write("a")}machine write(x:@mutable Cell)->u8{x.value=7;42}', b'"a"', 2),
    ]:
        wire, rows = packet([source], ["main.omg"])
        yield "quoted-image-" + name, wire, struct.pack("<BII", 0, status, rows[0][1] + source.index(at))


def declaration(definitions, header):
    start = definitions.index(header)
    return definitions[start:definitions.index(b"\n}", start) + 2]


def construction_unit_cases(encode):
    # The first file calls a later dependency. Its import belongs only to that
    # file, and an unreachable machine in another file must still type-check.
    caller = b"use lib\\answer; machine main() -> u8 { answer() }"
    provider = b"public machine answer() -> u8 { 42 }"
    hidden_call = b"machine unused() -> u8 { answer() }"
    good_unused = b"machine unused() -> u8 { 7 }"
    packages = [("app", 0), ("library", 0)]
    dependency = [(0, 1, "lib")]
    paths = ["main.omg", "helper.omg", "answer.omg"]

    def forward(helper, declaration, edges):
        data, _, _, rows = encode(packages, edges,
                                  [(0, caller), (0, helper), (1, declaration)],
                                  0, b"main", paths=paths)
        return data, rows

    # The callee can use the same local row number as a live caller.
    # The return to main must still see its original value.
    live_caller = (b"use lib\\answer; machine main() -> u8 { let kept: u8 = 42; "
                   b"let got: u8 = answer(); transition got == 7 { true -> kept, _ -> got } }")
    stores_local = b"public machine answer() -> u8 { let temp: u8 = 7; temp }"
    data, _, _, _ = encode(packages, dependency, [(0, live_caller), (1, stores_local)],
                           0, b"main", paths=["main.omg", "answer.omg"])
    yield "caller-local-survives-forward-call", data, 42

    data, _ = forward(good_unused, provider, dependency)
    yield "forward-dependency", data, 42
    data, rows = forward(good_unused, provider, [])
    yield "forward-missing-edge", data, struct.pack("<BII", 0, 1, rows[0][1])
    data, rows = forward(good_unused, provider.removeprefix(b"public "), dependency)
    yield "forward-private", data, struct.pack("<BII", 0, 1, rows[0][1])
    data, rows = forward(hidden_call, provider, dependency)
    yield "unused-file-cannot-borrow-import", data, struct.pack(
        "<BII", 0, 1, rows[1][1] + hidden_call.index(b"answer"))
    # Every unit must parse before an unresolved import can be diagnosed.
    data, rows = forward(b"\0", provider, [])
    yield "parse-before-missing-edge", data, struct.pack("<BII", 0, 1, rows[1][1])

    typed_caller = (b"use lib\\answer; machine main() -> u8 { relay(42) } "
                    b"machine relay(kept: u8) -> u8 { let got: u8 = answer(7); "
                    b"transition got == 7 { true -> kept, _ -> got } }")
    typed_provider = b"public machine answer(value: u8) -> u8 { value }"
    data, _, _, _ = encode(packages, dependency, [(0, typed_caller), (1, typed_provider)],
                           0, b"main", paths=["main.omg", "answer.omg"])
    yield "caller-parameter-survives-forward-call", data, 42
    bad_caller = typed_caller.replace(b"answer(7)", b"answer(false)")
    data, _, _, rows = encode(packages, dependency, [(0, bad_caller), (1, typed_provider)],
                              0, b"main", paths=["main.omg", "answer.omg"])
    yield "forward-mistyped-argument", data, struct.pack(
        "<BII", 0, 1, rows[0][1] + bad_caller.index(b"false"))

    # Each source file needs 8,320 authored import components. Together
    # they exceed the 16,384-row parser arena. Repeated imports of the same
    # declaration are legal and do not change the expected executable.
    components = ["x"] * 126
    import_path = "\\".join(["lib", *components, "answer"]).encode("ascii")
    import_statement = b"use " + import_path + b";\n"
    imports = import_statement * 65
    sources = [(0, imports + b"machine main() -> u8 { answer() }"),
               (0, imports + b"machine unused() -> u8 { answer() }"),
               (1, provider)]
    deep_paths = ["main.omg", "helper.omg", "/".join([*components, "answer.omg"])]
    data, _, _, _ = encode(packages, dependency, sources, 0, b"main", paths=deep_paths)
    yield "imports-fit-one-unit-at-a-time", data, 42

    # Moving both import blocks into one file exhausts its own arena. The
    # first component ("lib") of import 129 requires path member 16,385.
    sources = [(0, imports * 2 + b"machine main() -> u8 { answer() }"),
               (0, good_unused), (1, provider)]
    data, _, _, rows = encode(packages, dependency, sources, 0, b"main", paths=deep_paths)
    first_overflow = rows[0][1] + 128 * len(import_statement) + len(b"use ")
    yield "imports-exceed-one-unit", data, struct.pack("<BII", 0, 3, first_overflow)


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
    let mutable pair:Buffers;
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


    # SourceSet carries five independent field bounds, including its eight-MiB
    # arena. A valid full byte arena cannot establish capacity in the path arena.
    files = [declaration(source, b"public data SourceSet\n")]
    files.extend(copy_declaration(source, name) for name in [b"DependencyRow", b"PackageRow", b"SourceRange", b"DependencyScope"])
    reader = b"""use inputs\\SourceSet;
machine main()->u8{
 let mutable sources:SourceSet;
 sources.unit_count=512;
 sources.byte_count=8388608;
 sources.path_byte_count=16384;
 sources.package_count=64;
 sources.dependency_count=256;
 sources.bytes[8388607]=42;
 transition 512-sources.unit_count==0 && 8388608-sources.byte_count==0 &&
   16384-sources.path_byte_count==0 && 64-sources.package_count==0 && 256-sources.dependency_count==0 {
   true->sources.bytes[8388607],_->1
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


def record_reference_cases(encode):
    # The supplier mutates the caller's actual SourceUnit and later borrows
    # it shared. Both signatures precede the imported record's declaration.
    text = (ROOT / "source/subsystems/compiler/source_to_psi/representations/source_files/source.omg").read_bytes()
    unit = declaration(text, b"public data SourceUnit\n")
    identifier = declaration(text, b"public data SourceId ")
    clear = declaration(text, b"machine source_unit_clear(")
    wrapper = b"""public machine clear_unit(unit:@mutable SourceUnit,id:SourceId){source_unit_clear(unit,id);}
public machine retained(unit:@SourceUnit)->bool{unit.last_retained}
"""
    caller = b"""use store\\SourceUnit;use store\\SourceId;use store\\clear_unit;use store\\retained;
machine main()->u8{
 let mutable unit:SourceUnit;
 unit.bytes[7]=42;
 unit.length=11;
 let id:SourceId=SourceId{value:17 as +Trapping};
 clear_unit(@mutable unit,id);
 transition retained(@unit) && unit.length==0 && unit.id.value==17 {true->unit.bytes[7],_->0}
}"""

    def packet(main=caller, api=wrapper, record=unit):
        return encode([("app", 0), ("store", 0)], [(0, 1, "store")], [
            (0, main), (1, api + b"\n" + clear), (1, record), (1, identifier)
        ], 0, b"main", paths=["main.omg", "api.omg", "unit.omg", "id.omg"])

    wire, _, _, _ = packet()
    yield "product-source-unit-forward-reference", wire, 42
    shared = caller.replace(b"@mutable unit,id", b"@unit,id")
    wire, _, _, rows = packet(main=shared)
    yield "source-unit-readonly-actual", wire, struct.pack("<BII", 0, 2, rows[0][1] + shared.index(b"@unit,id"))
    missing = wrapper.replace(b"unit:@SourceUnit", b"unit:@Missing")
    wire, _, _, rows = packet(api=missing)
    yield "source-unit-missing-referent", wire, struct.pack("<BII", 0, 2, rows[1][1] + missing.index(b"unit:@Missing"))
    wire, _, _, rows = packet(record=unit.replace(b"public data", b"data", 1))
    yield "source-unit-hidden-referent", wire, struct.pack("<BII", 0, 1, rows[0][1])


def evaluated_guard_cases(encode):
    array = (b"machine main()->u8{let bytes:[u8;1]=[42];choose(@bytes[..],0)}"
             b"machine inspect(bytes:@[u8])->bool{bytes.length>0}")
    signature = b"machine choose(bytes:@[u8],i:u64)->u8 "
    transfer = b"{transition inspect(bytes){true->read(bytes,i),_ ->0}"
    read = b"machine read(bytes:@[u8],i:u64)->u8 requires i<bytes.length {bytes[i]}}"
    cases = [
        ("guard-call-preserved-slice", signature + b"requires i<bytes.length " + transfer + read + array, 42),
        ("guard-call-without-bound", signature + transfer + read + array, (2, b"machine choose")),
    ]
    header = (b"data Cell{value:u8;}"
              b"machine dirty(c:@mutable Cell)->bool{c.value=2;true}"
              b"machine drive(stable:@Cell,other:@mutable Cell)->u8 {"
              b"transition stable.value<2 {true->choose(stable,other),_ ->7}"
              b"machine choose(stable:@Cell,other:@mutable Cell)->u8 requires stable.value<2 {")
    use = (b"transition dirty(other){true->use(stable),_ ->7}}"
           b"machine use(stable:@Cell)->u8 requires stable.value<2 {42}}"
           b"machine main()->u8{let mutable left:Cell;let mutable right:Cell;drive(@left,@mutable right)}")
    cases.append(("guard-call-disjoint-write", header + use, 42))
    overlap = (b"data Cell{value:u8;}machine dirty(c:@mutable Cell)->bool{c.value=2;true}"
               b"machine drive(c:@mutable Cell)->u8{transition c.value<2{true->choose(c),_ ->7}"
               b"machine choose(c:@mutable Cell)->u8 requires c.value<2 {"
               b"transition dirty(c){true->use(c),_ ->7}}"
               b"machine use(c:@mutable Cell)->u8 requires c.value<2 {42}}"
               b"machine main()->u8{let mutable cell:Cell;drive(@mutable cell)}")
    cases.append(("guard-call-overlapping-write", overlap, (2, b"machine drive")))
    # A left conjunct was true before dirty wrote its referent. The target
    # must not recover it from the Boolean value of the whole condition.
    old = overlap.replace(b" requires c.value<2 {transition dirty(c)",
                          b" {transition c.value<2 && dirty(c)")
    cases.append(("guard-call-stale-conjunct", old, (2, b"machine drive")))
    for name, source, expected in cases:
        packet, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
        if isinstance(expected, tuple):
            status, anchor = expected
            expected = struct.pack("<BII", 0, status, rows[0][1] + source.index(anchor))
        yield name, packet, expected


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



SYNTAX_FILE = "representations/syntax_trees/syntax_trees.omg"
NUMERIC_FILE = "representations/tokens/numeric.omg"
SOURCE_FILE = "representations/source_files/source.omg"


def syntax_units(definitions, declarations, *, source_spans=False):
    """Keep product types and functions in the files that declare them."""
    product = ROOT / "source/subsystems/compiler/source_to_psi"
    numeric = (product / NUMERIC_FILE).read_bytes()
    units = [(NUMERIC_FILE, copy_declaration(numeric, b"IntegerSuffix"))]
    if source_spans:
        source = (product / SOURCE_FILE).read_bytes()
        units.append((SOURCE_FILE, b"\n".join(copy_declaration(source, name)
                      for name in (b"SourceId", b"Span", b"SourceSpan"))))
    imports = [f"use {path};".encode() for path, _ in units]
    for statement in imports:
        assert statement in definitions.splitlines(), statement
    return [(SYNTAX_FILE, b"\n".join([*imports, *declarations])), *units]


def full_syntax_data(definitions):
    header = rb"(?m)^public data [A-Za-z_][A-Za-z_0-9]*(?: \[copy\])?\s*(?:\{|\nrequires)"
    return [declaration(definitions, match.group()) for match in re.finditer(header, definitions)]


def syntax_request(encode, main, units):
    sources = [("main.omg", main), *units]
    request, _, _, _ = encode([("app", 0)], [], [(0, source) for _, source in sources],
                              0, b"main", paths=[path for path, _ in sources])
    return request


def sum_equality_cases(encode):
    """Intrinsic nominal equality, including the product's type-form fields."""
    def packet(source):
        data, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main",
                                  paths=["main.omg"])
        return data, rows[0][1]

    carrier = b"data Mode [copy]{case First;case Second;}"
    finish = b"{true->42,_->7}"
    for name, source in [
        ("sum-equality-distinct-storage", carrier +
         b"machine main()->u8{let a:Mode=Mode::First;let b:Mode=Mode::First;"
         b"transition a==b && a!=Mode::Second && !(a!=b)" + finish + b"}"),
        ("sum-equality-reused-call-result", carrier +
         b"machine echo(value:Mode)->Mode{value}machine main()->u8{"
         b"transition echo(Mode::First)!=echo(Mode::Second) && "
         b"echo(Mode::Second)==echo(Mode::Second)" + finish + b"}"),
        ("sum-equality-left-observed-before-mutation", carrier +
         b"data Holder{mode:Mode;}machine change(holder:@mutable Holder)->Mode{"
         b"holder.mode=Mode::Second;holder.mode}machine main()->u8{let mutable holder:Holder;"
         b"holder.mode=Mode::First;let different:bool=holder.mode!=change(@mutable holder);"
         b"transition different && holder.mode==Mode::Second" + finish + b"}"),
        ("sum-equality-evaluation-order", carrier +
         b"data Counter{value:u8;}machine first(counter:@mutable Counter)->Mode{"
         b"counter.value=1;Mode::First}machine second(counter:@mutable Counter)->Mode{"
         b"transition counter.value==1{true->Mode::Second,_->Mode::First}}"
         b"machine main()->u8{let mutable counter:Counter;"
         b"transition first(@mutable counter)!=second(@mutable counter)" + finish + b"}"),
        ("sum-equality-authorized-effect", carrier +
         b"machine risky()->Mode crashes Trap{Mode::First}"
         b"machine compare()->bool crashes Trap{Mode::First==risky()}"
         b"machine main()->u8 crashes Trap{transition compare()" + finish + b"}"),
        ("sum-equality-loan-independent-place", carrier + b"data Holder{mode:Mode;}"
         b"machine blocked(holder:@mutable Holder)->bool{transition{_->next(holder,Mode::First==Mode::First)}"
         b"machine next(holder:@mutable Holder,answer:bool)->bool{answer}}"
         b"machine main()->u8{let mutable holder:Holder;transition blocked(@mutable holder)" + finish + b"}"),
    ]:
        yield name, packet(source)[0], 42

    # Preserve the actual product's declarations. The projection guard is the
    # front of syntax_trees::same_type_form; unrelated fields are omitted here.
    definitions = (ROOT / "source/subsystems/compiler/source_to_psi/representations/syntax_trees/syntax_trees.omg").read_bytes()
    units = syntax_units(definitions, [copy_declaration(definitions, b"IntegerLiteral"),
                                     declaration(definitions, b"public machine static_integer_is_valid(")])
    for name, exact, empty, suffix, negate in [
        ("valid", b"true", b"false", b"None", b""),
        ("inexact", b"false", b"false", b"None", b"!"),
        ("empty", b"true", b"true", b"None", b"!"),
        ("suffixed", b"true", b"false", b"U8", b"!"),
    ]:
        main = (f"use {SYNTAX_FILE} as syntax;\nuse {NUMERIC_FILE};\n".encode() +
                b"machine main()->u8{let value:|syntax|::IntegerLiteral=|syntax|::IntegerLiteral{"
                b"magnitude:42 as u64 +Wrapping,exact:" + exact +
                b",empty_digits:" + empty + b",suffix:|numeric|::IntegerSuffix::" + suffix +
                b"};transition " + negate + b"|syntax|::static_integer_is_valid(value)" + finish + b"}")
        yield "sum-equality-product-static-integer-" + name, syntax_request(encode, main, units), 42

    kinds = copy_declaration(definitions, b"TypeReferenceKind")
    access = copy_declaration(definitions, b"ReferenceAccess")
    source = (kinds + access +
              b"data TypeReference [copy]{kind:TypeReferenceKind;access:ReferenceAccess;argument_count:u8;}"
              b"machine same_type_form(left:TypeReference,right:TypeReference)->bool{"
              b"transition left.kind!=right.kind || left.access!=right.access || "
              b"left.argument_count!=right.argument_count{true->false,_->true}}"
              b"machine main()->u8{let a:TypeReference=TypeReference{kind:TypeReferenceKind::Reference,"
              b"access:ReferenceAccess::Shared,argument_count:0};"
              b"let b:TypeReference=TypeReference{kind:TypeReferenceKind::Reference,"
              b"access:ReferenceAccess::Mutable,argument_count:0};"
              b"let c:TypeReference=TypeReference{kind:TypeReferenceKind::Array,"
              b"access:ReferenceAccess::Shared,argument_count:0};"
              b"transition same_type_form(a,a) && !same_type_form(a,b) && !same_type_form(a,c)" + finish + b"}")
    yield "sum-equality-product-type-form", packet(source)[0], 42

    for name, source, marker, status in [
        ("sum-equality-different-nominals", carrier + b"data Other [copy]{case First;case Second;}"
         b"machine main()->u8{transition Mode::First==Other::First" + finish + b"}", b"==", 1),
        ("sum-equality-payload-bearing", b"data Mode [copy]{case First;case Second(value:u8);}"
         b"machine main()->u8{transition Mode::First==Mode::First" + finish + b"}", b"==", 2),
        ("sum-equality-record-conformance-required", b"data Record [copy]{value:u8;}"
         b"machine main()->u8{let a:Record=Record{value:1};transition a==a" + finish + b"}", b"==", 2),
        ("sum-equality-no-ordering", carrier + b"machine main()->u8{transition Mode::First<Mode::Second" + finish + b"}", b"<", 1),
        ("sum-equality-no-scalar-coercion", carrier + b"machine main()->u8{transition Mode::First==0" + finish + b"}", b"==", 1),
        ("sum-equality-retains-effects", carrier +
         b"machine risky()->Mode crashes Trap{Mode::First}"
         b"machine compare()->bool crashes none{Mode::First==risky()}"
         b"machine main()->u8{42}", b"machine compare", 2),
        ("sum-equality-retains-loans", carrier + b"data Holder{mode:Mode;}"
         b"machine blocked(holder:@mutable Holder)->bool{transition{_->next(holder,holder.mode==Mode::First)}"
         b"machine next(holder:@mutable Holder,answer:bool)->bool{answer}}"
         b"machine main()->u8{let mutable holder:Holder;transition blocked(@mutable holder)" + finish + b"}", b"machine blocked", 1),
        ("sum-equality-retains-copy-check", carrier.replace(b"[copy]", b"") +
         b"machine main()->u8{let a:Mode=Mode::First;transition a==a" + finish + b"}", b"machine main", 2),
    ]:
        wire, start = packet(source)
        yield name, wire, struct.pack("<BII", 0, status, start + source.index(marker))


def parameter_order_cases(encode):
    # The original build-path scanner passes both relations together. The
    # strict byte reader needs fresh evidence before it may index at an endpoint.
    signature = b"machine scan(path:@[u8],start:u64,index:u64)->u8 "
    bounded = b"requires start<=index && index<=path.length "
    reader = b"machine byte(path:@[u8],start:u64,index:u64)->u8 requires start<=index && index<path.length {path[index]}"
    dispatcher = signature + bounded + b"{transition index<path.length {true->byte(path,start,index),_ ->7}" + reader + b"}"
    entry = b"machine main()->u8{let path:[u8;1]=[42];scan(@path[..],0,0)}"
    samples = [
        ("order-slice-first-byte", dispatcher + entry, 42),
        ("order-slice-endpoint", dispatcher + entry.replace(b",0,0)", b",1,1)"), 7),
        ("order-slice-empty", dispatcher + entry.replace(b"@path[..]", b"@path[0..0]"), 7),
        ("order-slice-inverted", dispatcher + entry.replace(b",0,0)", b",1,0)"), (2, b"machine main")),
        ("order-slice-past-end", dispatcher + entry.replace(b",0,0)", b",0,2)"), (2, b"machine main")),
        ("order-slice-end-is-not-byte", signature + bounded + b"{path[index]}" + entry, (2, b"path[index]")),
    ]
    receiver = b"machine take(a:u64,b:u64)->u8 requires a<=b {42}"
    caller = b"machine pass(x:u64,y:u64)->u8 "
    main = b"machine main()->u8{pass(0,1)}"
    for name, requirements, arguments, expected in [
        ("order-scalar-forward", b"requires x<=y ", b"x,y", 42),
        ("order-scalar-reverse-spelling", b"requires y>=x ", b"x,y", 42),
        ("order-scalar-strict-implies-inclusive", b"requires x<y ", b"x,y", 42),
        ("order-scalar-reflexive", b"", b"x,x", 42),
        ("order-scalar-reversed-actuals", b"requires x<=y ", b"y,x", (2, b"machine pass")),
        ("order-scalar-unproved", b"", b"x,y", (2, b"machine pass")),
    ]:
        samples.append((name, caller + requirements + b"{take(" + arguments + b")}" + receiver + main, expected))
    guard = caller + b"{transition x>y {false->take(x,y),_ ->7}" + receiver + b"}"
    samples.append(("order-scalar-negative-guard", guard + main, 42))
    samples.append(("order-scalar-wrong-direction", guard.replace(b"x>y", b"x<y") + main, (2, b"machine pass")))
    samples.append(("order-scalar-inclusive-is-not-strict", caller + b"requires x<=y {take(x,y)}" +
                    receiver.replace(b"a<=b", b"a<b") + main, (2, b"machine pass")))
    samples.append(("order-scalar-effectful-argument", caller + b"requires x<=y {take(x,copy(y))}" + receiver +
                    b"machine copy(n:u64)->u64{n}" + main, (2, b"machine pass")))
    two = b"machine choose(left:@[u8],right:@[u8],n:u64)->u8 requires n<=left.length {"
    two_entry = (b"machine main()->u8{let a:[u8;1]=[42];let b:[u8;1]=[7];"
                 b"choose(@a[..],@b[..],1)}")
    end = b"machine end(path:@[u8],n:u64)->u8 requires n<=path.length {42}"
    samples.append(("order-slice-forward", two + b"end(left,n)}" + end + two_entry, 42))
    samples.append(("order-slice-wrong-length", two + b"end(right,n)}" + end + two_entry,
                    (2, b"machine choose")))
    # The construction application's has_build_file passes a u32 unit and
    # limit. Its nested file machine requires unit < limit on that arrival.
    for carrier in (b"u32", b"u8"):
        sig = b"machine search(unit:" + carrier + b",limit:" + carrier + b")->u8 "
        visitor = b"machine file(unit:" + carrier + b",limit:" + carrier + b")->u8 requires unit<limit {42}"
        body = b"{transition unit<limit {true->file(unit,limit),_ ->7}" + visitor + b"}"
        invoke = b"machine main()->u8{search(0,1)}"
        stem = "order-" + carrier.decode()
        samples.append((stem + "-unit-limit", sig + body + invoke, 42))
        samples.append((stem + "-empty", sig + body + invoke.replace(b"(0,1)", b"(1,1)"), 7))
        samples.append((stem + "-wrong-pair", sig + body.replace(b"file(unit,limit)", b"file(limit,unit)") +
                        invoke, (2, b"machine search")))
        step = (b"machine step(unit:" + carrier + b",limit:" + carrier + b")->" + carrier +
                b" requires unit<limit {unit+1}")
        use = b"machine main()->u8{let next:" + carrier + b"=step(254,255);transition next==255{true->42,_ ->7}}"
        samples.append((stem + "-successor", step + use, 42))
        samples.append((stem + "-successor-inclusive", step.replace(b"unit<limit", b"unit<=limit") + use,
                        (2, b"+1")))
        samples.append((stem + "-successor-wrong-base", step.replace(b"{unit+1}", b"{limit+1}") + use,
                        (2, b"+1")))
        samples.append((stem + "-successor-two", step.replace(b"{unit+1}", b"{unit+2}") + use,
                        (2, b"+2")))
    # Exercise arithmetic above the bootstrap proof ledger's signed 32-bit
    # interval range. The runtime guard supplies the precondition.
    top = (b"machine pass(unit:u32,limit:u32)->u32{transition unit<limit{true->step(unit,limit),_ ->0}"
           b"machine step(unit:u32,limit:u32)->u32 requires unit<limit{unit+1}}"
           b"machine main()->u8{let next:u32=pass(4294967294,4294967295);"
           b"transition next==4294967295{true->42,_ ->7}}")
    samples.append(("order-u32-last-successor", top, 42))
    mixed = b"machine wrong(unit:u8,limit:u32)->u8 requires unit<limit {42}machine main()->u8{wrong(0,1)}"
    samples.append(("order-unsigned-carrier-mismatch", mixed, (2, b"machine wrong")))
    narrow = b"machine wrong(path:@[u8],index:u32)->u8 requires index<path.length {42}" + entry.replace(b"scan(@path[..],0,0)", b"wrong(@path[..],0)")
    samples.append(("order-narrow-index-slice-length", narrow, (2, b"machine wrong")))
    for name, source, expected in samples:
        packet, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
        if isinstance(expected, tuple):
            status, anchor = expected
            expected = struct.pack("<BII", 0, status, rows[0][1] + source.index(anchor))
        yield name, packet, expected

    # The callee's argument positions belong to its own file. Callers can
    # spell and order their parameters differently.
    provider = b"public machine take(high:u64,low:u64)->u8 requires low<=high {42}"
    for name, actuals, accepted in [("order-cross-file", b"y,x", True),
                                    ("order-cross-file-swapped", b"x,y", False)]:
        client = caller + b"requires x<=y {take(" + actuals + b")}" + main
        packet, _, _, rows = encode([("app", 0)], [], [(0, client), (0, provider)], 0,
                                    b"main", paths=["main.omg", "limits.omg"])
        expected = 42 if accepted else struct.pack("<BII", 0, 2, rows[0][1] + client.index(b"machine pass"))
        yield name, packet, expected


def storage_window_cases(encode):
    """Checked backing is disjoint from both the Alpha tape and return stack."""
    def packet(source):
        data, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main",
                                  paths=["main.omg"])
        return data, rows[0][1]

    # These declarations reserve/check large regions, but main never calls
    # reserve or initializes a giant value. Seed execution remains tiny.
    for name, source in [
        ("storage-array-last-valid", b"data Region{bytes:[u8;1073741824];}machine main()->u8{42}"),
        ("storage-record-last-valid", b"data Region{left:[u8;536870912];right:[u8;536870912];}machine main()->u8{42}"),
        ("storage-payload-last-valid", b"data Region [copy]{case Value(bytes:[u8;1073741816]);}machine main()->u8{42}"),
        ("storage-arena-last-valid", b"data Reserve [copy]{bytes:[u8;1073741808];}machine reserve(bytes:Reserve)->u8{42}machine main()->u8{42}"),
        ("storage-byte-view-relocated", b"data B{bytes:[u8;3];}machine second(bytes:@[u8])->u8 requires bytes.length>=2 {bytes[1]}machine main()->u8{let mutable b:B;b.bytes[1]=42;second(@b.bytes[0..2])}"),
    ]:
        yield name, packet(source)[0], 42

    for name, source, marker in [
        ("storage-array-first-invalid", b"data Region{bytes:[u8;1073741825];}machine main()->u8{42}", b"data Region"),
        ("storage-record-first-invalid", b"data Region{left:[u8;536870912];right:[u8;536870912];tail:u8;}machine main()->u8{42}", b"data Region"),
        ("storage-payload-first-invalid", b"data Region [copy]{case Value(bytes:[u8;1073741817]);}machine main()->u8{42}", b"bytes:"),
        ("storage-arena-first-invalid", b"data Reserve [copy]{bytes:[u8;1073741809];}machine reserve(bytes:Reserve)->u8{42}machine main()->u8{42}", b"machine main"),
        ("storage-array-product-overflow", b"data Region{bytes:[u64;536870913];}machine main()->u8{42}", b"data Region"),
        ("storage-two-regions-exhausted", b"data Reserve{bytes:[u8;536870912];}machine main()->u8{let a:Reserve;let b:Reserve;42}", b"machine main"),
    ]:
        wire, start = packet(source)
        yield name, wire, struct.pack("<BII", 0, 2, start + source.index(marker))

    definitions = (ROOT / "source/subsystems/compiler/source_to_psi" / SYNTAX_FILE).read_bytes()
    units = syntax_units(definitions, full_syntax_data(definitions), source_spans=True)
    # Both complete 45,156,664-byte owners are live at once. Calls touch the
    # final word and an earlier constrained field independently; aliasing or
    # result/parameter backing reuse changes the observable result.
    main = (f"use {SYNTAX_FILE} as syntax;\n".encode() +
            b"machine mark(tree:@mutable |syntax|::SyntaxTrees){tree.accepted=true;tree.root_count=42;}"
            b"machine poison(tree:@mutable |syntax|::SyntaxTrees){tree.accepted=false;tree.root_count=7;}"
            b"machine inspect(a:@|syntax|::SyntaxTrees,b:@|syntax|::SyntaxTrees)->bool{"
            b"a.accepted && !b.accepted && a.root_count==42 && b.root_count==7}"
            b"machine main()->u8{let mutable a:|syntax|::SyntaxTrees;let mutable b:|syntax|::SyntaxTrees;"
            b"mark(@mutable a);poison(@mutable b);transition inspect(@a,@b){true->42,_->7}}")
    yield "storage-product-two-complete-syntax-trees", syntax_request(encode, main, units), 42


def nominal_array_cases(encode):
    def packet(sources, paths, library=None):
        packages = [("app", 0)]
        dependencies = []
        if library:
            packages.append((library, 0))
            dependencies.append((0, 1, library))
        return encode(packages, dependencies, sources, 0, b"main", paths=paths)

    # Compile the unedited declaration and reach both ends of its eight-MiB field.
    definitions = (ROOT / "source/application/construction/input.omg").read_bytes()
    storage = declaration(definitions, b"data ConstructionInput {")
    caller = b"""machine main()->u8{
    let mutable a:ConstructionInput;
    a.unit_count=305;
    a.bytes[8388607]=255;
    a.bytes[8388606]=41;
    a.bytes[0]=42;
    transition a.unit_count==305 && a.bytes[0]==42 && a.bytes[8388606]==41 && a.bytes[8388607]==255 {true->42,_->7}
}"""
    data, _, _, _ = packet([(0, caller), (0, storage)], ["caller.omg", "input.omg"])
    yield "forward-product-construction-array", data, 42

    definitions = (ROOT / "source/subsystems/compiler/alpha/emit.omg").read_bytes()
    emitter = declaration(definitions, b"public data AlphaEmitter {")
    caller = b"""use alpha\\AlphaEmitter;machine main()->u8{
    let mutable a:AlphaEmitter;
    a.bytes[2097151]=42;
    a.labels[10239]=73;
    a.marks[65535]=99;
    transition a.labels[10239]==73 && a.marks[65535]==99 {true->a.bytes[2097151],_->7}
}"""
    data, _, _, _ = packet([(0, caller), (1, emitter)], ["caller.omg", "emit.omg"], "alpha")
    yield "forward-product-emitter-arrays", data, 42

    storage = b"public data Packet{bytes:[u8;9];tail:u64;}"
    length = b"public machine length(bytes:@[u8])->u64{bytes.length}"
    for name, end in [("borrow-retained-byte-array", b"9"), ("borrow-past-retained-byte-array", b"10")]:
        main = (b"use wire\\Packet;use wire\\length;machine main()->u8{let mutable p:Packet;p.bytes[8]=42;p.tail=77;"
                b"let n:u64=length(@p.bytes[7.." + end + b"]);"
                b"transition n==2&&p.tail==77{true->p.bytes[8],_->7}}")
        data, _, _, rows = packet([(0, main), (1, length), (1, storage)], ["main.omg", "length.omg", "packet.omg"], "wire")
        expected = 42 if end == b"9" else struct.pack("<BII", 0, 1, rows[0][1] + main.index(b".."))
        yield name, data, expected

    # Two directories expose the same spelling and layout. The field's owner
    # file chooses the element identity and declares the indexable extent.
    caller = b"""use types\\Shelf;use left\\Part;machine main()->u8{
    let mutable s:Shelf;
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

    def packet(units):
        data, _, _, rows = encode([('app', 0)], [], [(0, unit) for unit in units],
                                  0, b'main', paths=paths)
        return data, rows

    data, _ = packet(sources)
    yield 'nested-parameters-across-files', data, 42

    missing = sources.copy()
    prefix, body = missing[-1].rsplit(b'{p0}', 1)
    missing[-1] = prefix + b'{other_slot}' + body
    data, rows = packet(missing)
    position = rows[-1][1] + missing[-1].index(b'other_slot')
    yield 'nested-parameters-last-reference', data, struct.pack('<BII', 0, 1, position)

    mistyped = sources.copy()
    mistyped[1] = callee.replace(b'local(input)', b'local(false)')
    data, rows = packet(mistyped)
    position = rows[1][1] + mistyped[1].index(b'false')
    yield 'nested-parameters-mistyped', data, struct.pack('<BII', 0, 1, position)


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


def predicate_transition_sources():
    """Ordered subjectless predicates, with the product's projected-guard shape."""
    form = (b"data Form [copy] {kind:u8;access:u8;argument_count:u8;} "
            b"machine same_type_form(left:Form,right:Form)->bool {transition {"
            b"left.kind != right.kind || left.access != right.access || left.argument_count != right.argument_count -> different(),"
            b"_ -> equal()} machine different()->bool{false} machine equal()->bool{true}} ")
    for name, right, expected in [
        ("same", b"Form{kind:1,access:2,argument_count:3}", 42),
        ("changed-kind", b"Form{kind:0,access:2,argument_count:3}", 43),
        ("changed-access", b"Form{kind:1,access:0,argument_count:3}", 43),
        ("changed-count", b"Form{kind:1,access:2,argument_count:0}", 43),
    ]:
        source = (form + b"machine main()->u8 {let left:Form=Form{kind:1,access:2,argument_count:3};"
                  b"let right:Form=" + right + b";transition same_type_form(left,right) {true->42,_ ->43}}")
        yield "predicate-product-shape-" + name, source, expected

    choose = (b"machine choose(x:u8)->u8 {transition {x==0->42,x==1->43,_ ->44}} "
              b"machine main()->u8{choose(")
    for value, expected in [(0, 42), (1, 43), (2, 44)]:
        yield f"predicate-choice-{value}", choose + str(value).encode() + b")}", expected

    yield "predicate-true-coverage", b"machine main()->u8{transition{true->42}}", 42
    yield "predicate-first-wildcard", b"machine main()->u8{transition{_ ->42,true->7}}", 42
    yield "predicate-first-true", b"machine main()->u8{transition{true->42,true->7,_ ->1}}", 42
    yield "predicate-effects-ordered", (
        b"data Counter {value:u8;} "
        b"machine mark(counter:@mutable Counter, expected:u8, next:u8)->bool {"
        b"let answer:bool=counter.value==expected;counter.value=next;answer} "
        b"machine main()->u8{let mutable counter:Counter;transition{"
        b"mark(@mutable counter,9,1)->99,mark(@mutable counter,1,2)->counter.value,"
        b"mark(@mutable counter,2,3)->98,_ ->97}}"), 2
    yield "predicate-evaluated-once", (
        b"data Counter {flipped:bool;} machine flip(counter:@mutable Counter)->bool {"
        b"let old:bool=counter.flipped;counter.flipped=!old;!old} "
        b"machine main()->u8{let mutable counter:Counter;transition{flip(@mutable counter)->selected(counter.flipped),_ ->1}"
        b"machine selected(flipped:bool)->u8{transition flipped{true->42,_ ->7}}}"), 42
    yield "predicate-short-circuit", (
        b"data Counter {value:u8;} machine touch(counter:@mutable Counter)->bool{counter.value=9;true} "
        b"machine main()->u8{let mutable counter:Counter;transition{"
        b"false && touch(@mutable counter)->98,true || touch(@mutable counter)->counter.value,_ ->97}}"), 0

    # A refusal tuple is (guest status, exact source-coordinate spelling).
    yield "predicate-nonboolean", b"machine main()->u8{transition{1->42,_ ->0}}", (1, b"1->")
    yield "predicate-unselected-result-type", b"machine main()->u8{transition{true->42,_ ->false}}", (1, b"false")
    yield "predicate-unselected-guard-type", b"machine main()->u8{transition{_ ->42,1->0}}", (1, b"1->")
    yield "predicate-unselected-target", b"machine main()->u8{transition{false->missing(),_ ->42}}", (1, b"missing")
    yield "predicate-missing-comma", b"machine main()->u8{transition{true->42 _ ->0}}", (1, b"_ ->")
    # More general coverage proofs remain outside this bootstrap provision.
    yield "predicate-unproved-coverage", b"machine main()->u8{transition{false->42}}", (2, b"->42")

    yield "predicate-checked-arrival", (
        b"data B {bytes:[u8;2];} machine choose(index:u64)->u8{transition{index<2->read(index),_ ->0}"
        b"machine read(index:u64)->u8 requires index<2 {let mutable b:B;b.bytes[1]=42;b.bytes[index]}}"
        b"machine main()->u8{choose(1)}"), 42
    yield "predicate-failed-guard-kills-entry-fact", (
        b"data Cell requires length<=2,{bytes:[u8;2];length:u64;} "
        b"machine mutate(cell:@mutable Cell)->bool{cell.length=2;false} "
        b"machine choose(cell:@mutable Cell)->u8 requires cell.length<2 {transition{mutate(cell)->0,_ ->cell.bytes[cell.length]}} "
        b"machine main()->u8{0}"), (2, b"machine choose")


def predicate_transition_cases(encode):
    for name, source, expected in predicate_transition_sources():
        request, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
        if isinstance(expected, tuple):
            status, spelling = expected
            expected = struct.pack("<BII", 0, status, rows[0][1] + source.index(spelling))
        yield name, request, expected


def trait_static_parameter_cases(packet):
    for name, declarations, status, anchor in [
        ("typed-static-trait-parameter", b"trait Calling<const Policy: CallingPolicy> {} trait CallingPolicy {}", 2, b"trait Calling<"),
        ("retired-trait-evidence-binder", b"trait Calling<Policy satisfies CallingPolicy> {}", 1, b"satisfies"),
        ("duplicate-static-trait-parameter", b"trait Calling<const Policy: A, const Policy: B> {}", 1, b"Policy: B"),
        ("missing-static-trait-parameter-type", b"trait Calling<const Policy: > {}", 1, b">"),
    ]:
        source = declarations + b" machine main()->u8{42}"
        request, rows = packet([source], ["main.omg"])
        yield name, request, struct.pack("<BII", 0, status, rows[0][1] + source.index(anchor))


def data_lifetime_cases(packet):
    # The field names may be forwarded, repeated, or permuted. Each must bind
    # in the declaring item, and the application must match the target's arity.
    pair = b"public data Pair<'left,'right> { x: @'left u8; y: @'right u8; }"
    consumer = b"data Result<'a,'b> { case Stored(value: [Pair<'b,'a>; 2]); }"
    main = b"machine main()->u8{42}"
    for name, source, status, anchor in [
        ("valid-field-lifetime-application", consumer, 2, b"a,'b"),
        ("repeated-field-lifetime-argument", consumer.replace(b"'b,'a>;", b"'a,'a>;"), 2, b"a,'b"),
        ("undeclared-payload-lifetime", consumer.replace(b"'a>;", b"'unbound>;"), 1, b"unbound"),
        ("too-few-field-lifetimes", consumer.replace(b"'b,'a>;", b"'b>;"), 1, b"Pair<"),
        ("too-many-field-lifetimes", consumer.replace(b"'b,'a>;", b"'b,'a,'a>;"), 1, b"Pair<"),
        ("omitted-field-lifetimes", consumer.replace(b"<'b,'a>;", b";"), 1, b"Pair;"),
        ("scalar-with-field-lifetimes", b"data Result<'a> { value: bool<'a>; }", 1, b"bool<"),
        ("undeclared-reference-lifetime", b"data Result<'a> { value: @'missing [u8]; }", 1, b"missing"),
        ("duplicate-data-lifetime", b"data Result<'a,'a> {}", 1, b"a>"),
    ]:
        sources = [main, source, pair]
        request, rows = packet(sources, ["main.omg", "result.omg", "pair.omg"])
        yield name, request, struct.pack("<BII", 0, status, rows[1][1] + source.index(anchor))


def slice_conjunction_sources():
    signature = b"machine choose(left:@[u8],right:@[u8],index:u64)->u8{transition "
    read = (b"machine read(left:@[u8],right:@[u8],index:u64)->u8 "
            b"requires index<left.length && index<right.length {left[index]}} ")
    main = (b"machine main()->u8{let left:[u8;1]=[42];let right:[u8;1]=[9];"
            b"let answer:u8=choose(@left[..],@right[..],0);answer}")
    guards = [
        ("slice-guard-conjunction", b"index<left.length && index<right.length", 42),
        ("slice-guard-reversed-conjunction", b"index<right.length && index<left.length", 42),
        ("slice-guard-nested-conjunction", b"true && (index<left.length && index<right.length)", 42),
        ("slice-guard-disjunction", b"index<left.length || index<right.length", (2, b"machine choose")),
        ("slice-guard-wrong-slice", b"index<left.length && index<left.length", (2, b"machine choose")),
    ]
    for name, guard, expected in guards:
        source = signature + guard + b"{true->read(left,right,index),_ ->0}" + read + main
        yield name, source, expected
    source = (signature + b"index<left.length && index<right.length{true->0,_ ->read(left,right,index)}" + read + main)
    yield "slice-guard-false-arm", source, (2, b"machine choose")
    source = (signature + b"index<left.length && index<right.length{true->read(left,right,index),_ ->0}" + read + main)
    yield "slice-guard-empty-right", source.replace(b"@right[..]", b"@right[0..0]"), 0
    signature_other = b"machine choose(left:@[u8],right:@[u8],index:u64,other:u64)->u8{transition "
    source = (signature_other + b"index<left.length && index<right.length{true->read(left,right,other),_ ->0}" + read + main.replace(b"@right[..],0)", b"@right[..],0,0)"))
    yield "slice-guard-wrong-index", source, (2, b"machine choose")


def slice_conjunction_cases(encode):
    for name, source, expected in slice_conjunction_sources():
        request, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
        if isinstance(expected, tuple):
            status, spelling = expected
            expected = struct.pack("<BII", 0, status, rows[0][1] + source.index(spelling))
        yield name, request, expected


def slice_successor_sources():
    # Keep the complete construction helper unchanged; only its caller varies.
    product = declaration((ROOT / "source/application/construction/build_sources.omg").read_bytes(),
                          b"machine same_build_path(")
    for name, left_view, right, right_view, expected in [
        ("equal", b"[..]", b"[65,66]", b"[..]", 42),
        ("changed-byte", b"[..]", b"[65,67]", b"[..]", 7),
        ("different-length", b"[..]", b"[65,66]", b"[0..1]", 7),
        ("empty", b"[0..0]", b"[65,66]", b"[0..0]", 42),
    ]:
        source = (product + b" machine main()->u8{let left:[u8;2]=[65,66];let right:[u8;2]=" + right +
                  b";let equal:bool=same_build_path(@left" + left_view + b",@right" + right_view +
                  b");transition equal{true->42,_ ->7}}")
        yield "slice-successor-product-path-" + name, source, expected

    prefix = b"machine next_index(items:@[u8],index:u64)->u64 requires index<items.length {"
    call = (b"machine main()->u8{let bytes:[u8;2]=[41,42];"
            b"let result:u64=next_index(@bytes[..],0);transition result==1{true->42,_ ->7}}")
    yield "slice-successor-first", prefix + b"index+1}" + call, 42
    yield "slice-successor-last", (prefix + b"index+1}" + call).replace(b"@bytes[..],0)", b"@bytes[..],1)").replace(b"result==1", b"result==2"), 42
    yield "slice-successor-two", prefix + b"index+2}" + call, (2, b"+2")
    yield "slice-successor-unbounded", (prefix + b"index+1}" + call).replace(b"requires index<items.length ",b""), (2, b"+1")
    wrong = (prefix + b"other+1}" + call).replace(b"index:u64)",b"index:u64,other:u64)").replace(b"@bytes[..],0)",b"@bytes[..],0,0)")
    yield "slice-successor-wrong-index", wrong, (2, b"+1")
    inclusive = (prefix + b"index+1}" + call).replace(b"index<items.length",b"index<=items.length")
    yield "slice-successor-inclusive", inclusive, (2, b"+1")
    read = (b"machine read(items:@[u8],index:u64)->u8 requires index<items.length {items[index+1]} "
            b"machine main()->u8{let bytes:[u8;1]=[42];let result:u8=read(@bytes[..],0);result}")
    yield "slice-successor-is-not-an-index-proof", read, (2, b"items[index")


def successor_order_sources():
    prefix = (b"machine step(path:@[u8],start:u64,index:u64)->u8 "
              b"requires start<=index && index<path.length {transition {_->finish(path,")
    callee = b"machine finish(path:@[u8],a:u64,b:u64)->u8 requires "
    entry = b"machine main()->u8{let path:[u8;1]=[42];step(@path[..],0,0)}"
    for name, args, condition, expected in [
        ("slice-successor-endpoint", b"start,index+1", b"a<b && b<=path.length", 42),
        ("slice-successor-equal-endpoints", b"index+1,index+1", b"a<=b && b<=path.length", 42),
        ("slice-successor-equal-is-not-strict", b"index+1,index+1", b"a<b", (2, b"machine step")),
        ("slice-successor-cannot-read-end", b"start,index+1", b"b<path.length", (2, b"machine step")),
        ("slice-successor-order-reversed", b"index+1,start", b"a<=b", (2, b"machine step")),
    ]:
        yield name, prefix + args + b")}" + callee + condition + b"{42}}" + entry, expected


def slice_successor_cases(encode):
    for name, source, expected in (*slice_successor_sources(), *successor_order_sources()):
        request, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
        if isinstance(expected, tuple):
            status, spelling = expected
            expected = struct.pack("<BII", 0, status, rows[0][1] + source.index(spelling))
        yield name, request, expected


def private_arrival_sources():
    body = b"machine read(index:u64)->u8{let bytes:[u8;2]=[41,42];bytes[index]}"
    main = b"machine main()->u8{choose(1)}"
    yield 'terminal-guard', b"machine choose(index:u64)->u8{transition index<2{true->read(index),_ ->0}"+body+b"}"+main, 42
    yield 'ordinary-literal', b"machine main()->u8{let result:u8=read(1);result "+body+b"}", 42
    yield 'ordinary-entry-contract', b"machine choose(index:u64)->u8 requires index<2 {let result:u8=read(index);result "+body+b"}"+main, 42
    yield 'two-good-arrivals', b"machine choose(index:u64)->u8{transition{index<1->read(index),index==1->read(index),_ ->0}"+body+b"}"+main, 42
    yield 'one-bad-arrival', b"machine choose(index:u64)->u8{transition{index<1->read(index),_ ->read(index)}"+body+b"}"+main, (2,66)
    yield 'bad-ordinary', b"machine main()->u8{let result:u8=read(2);result "+body+b"}", (2,33)
    yield 'unused', b"machine main()->u8{42 "+body+b"}", (2,30)
    yield 'free-entry', b"machine read(index:u64)->u8{let bytes:[u8;2]=[41,42];bytes[index]} machine main()->u8{42}", (2,0)
    yield 'explicit-true', b"machine choose(index:u64)->u8{transition index<2{true->read(index),_ ->0}"+body.replace(b'->u8{',b'->u8 requires true {')+b"}"+main, (2,0)
    yield 'explicit-too-weak', b"machine choose(index:u64)->u8{transition index<2{true->read(index),_ ->0}"+body.replace(b'->u8{',b'->u8 requires index<=2 {')+b"}"+main, (2,0)
    cyclic=b"machine read(index:u64)->u8{let bytes:[u8;2]=[41,42];transition index<2{true->read(index),_ ->bytes[index]}}"
    yield 'terminal-cycle', b"machine choose(index:u64)->u8{transition index<2{true->read(index),_ ->0}"+cyclic+b"}"+main, (2,151)
    field=(b"data B requires count<=2,{bytes:[u8;2];count:u64;} machine choose(box:@B,index:u64)->u8{transition index<box.count{true->read(box,index),_ ->0}machine read(box:@B,index:u64)->u8{box.bytes[index]}}machine main()->u8{let mutable box:B;box.count=2;box.bytes[1]=42;choose(@box,1)}")
    yield 'field-upper-bound', field, 42
    yield 'field-upper-bound-too-wide', field.replace(b'count<=2',b'count<=3'), (2,121)
    yield 'field-upper-bound-inclusive', field.replace(b'index<box.count',b'index<=box.count'), (2,122)
    trapped=(b"machine choose(index:u64 in Trapping)->u8{transition index<2{true->read(index),_ ->0}machine read(index:u64 in Trapping)->u8{let bytes:[u8;2]=[41,42];bytes[index as -Trapping]}}machine main()->u8{choose(1u64 as +Trapping)}")
    yield 'carrier-preserving-removal', trapped, 42
    tighten=(b"machine choose(index:u64)->u8{transition index<1{true->read(index),_ ->0}machine read(index:u64)->u8{let a:[u8;2]=[41,42];let b:[u8;1]=[43];let first:u8=a[index];b[index]}}machine main()->u8{choose(0)}")
    yield 'tightened-index', tighten, 43
    yield 'tightened-index-bad-arrival', tighten.replace(b'index<1',b'index<2').replace(b'choose(0)',b'choose(1)'), (2,55)


def private_arrival_full_cases(encode):
    syntax = (ROOT / "source/subsystems/compiler/source_to_psi" / SYNTAX_FILE).read_bytes()
    operations = [declaration(syntax, b"public machine " + name + b"(") for name in
                  (b"static_integer_is_valid", b"same_type_form", b"syntax_trees_integer_literal")]
    units = syntax_units(syntax, full_syntax_data(syntax) + operations, source_spans=True)
    yield "private-arrival-full-product-compile", syntax_request(encode, b"machine main()->u8{42}", units), 42
    def caller(second_magnitude):
        return (f"use {SYNTAX_FILE} as syntax;\nuse {NUMERIC_FILE};\n".encode() +
                b"machine main()->u8 crashes Trap {let mutable store:|syntax|::SyntaxTrees;"
                b"store.integer_literal_count=2;"
                b"store.integer_literals[0]=|syntax|::IntegerLiteral{magnitude:4u64 as +Wrapping,"
                b"exact:true,empty_digits:false,suffix:|numeric|::IntegerSuffix::None};"
                b"store.integer_literals[1]=|syntax|::IntegerLiteral{magnitude:" + second_magnitude +
                b"u64 as +Wrapping,exact:true,empty_digits:false,suffix:|numeric|::IntegerSuffix::None};"
                b"let left:|syntax|::TypeReference=|syntax|::TypeReference{kind:|syntax|::TypeReferenceKind::Literal,"
                b"literal:0u64 as +Trapping,..};"
                b"let right:|syntax|::TypeReference=|syntax|::TypeReference{kind:|syntax|::TypeReferenceKind::Literal,"
                b"literal:1u64 as +Trapping,..};"
                b"let first:|syntax|::IntegerLiteral=|syntax|::syntax_trees_integer_literal(@store,0 as +Trapping);"
                b"transition first.magnitude==4u64 as +Wrapping && |syntax|::same_type_form(@store,left,right)"
                b"{true->42,_ ->7}}")

    yield "private-arrival-full-product-equal", syntax_request(encode, caller(b"4"), units), 42
    yield "private-arrival-full-product-changed", syntax_request(encode, caller(b"5"), units), 7


def private_arrival_cases(encode):
    for name, source, expected in private_arrival_sources():
        request, _, _, rows = encode([("app", 0)], [], [(0, source)], 0, b"main", paths=["main.omg"])
        if isinstance(expected, tuple):
            status, coordinate = expected
            expected = struct.pack("<BII", 0, status, rows[0][1] + coordinate)
        yield "private-arrival-" + name, request, expected
    yield from private_arrival_full_cases(encode)


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
