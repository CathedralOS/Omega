#!/usr/bin/env python3
"""Independent exact-profile model for persisted-Beta OMGRFN15 owners.

The model is a pure source materializer.  It does not invoke the resolver,
lowerer, backend, or CKIR evaluator.  The focused composite separately proves
that producer bytes have not drifted from these reviewed finite tables.
"""

from __future__ import annotations

import importlib.util
import re
import struct
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "bootstrap/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


IR = load("omgrfn15_ir", GATES / "checked_ir_v13_reference.py")


def encode_ckir(tables: dict[str, list[tuple[int, ...]]], values: int,
                places: int) -> bytes:
    counts = {name: len(tables[name]) for name in IR.TABLE_ORDER}
    counts.update(values=values, places=places)
    payload = b"".join(IR.ROWS[name].pack(*row)
                       for name in IR.TABLE_ORDER for row in tables[name])
    return IR.HEADER.pack(
        b"OMGCKIR\0", 13, 0, 1, 1, 0, IR.HEADER.size + len(payload),
        *(counts[name] for name in IR.COUNT_NAMES),
    ) + payload


def source_ckir(profile: int) -> bytes:
    t = {name: [] for name in IR.TABLE_ORDER}
    t["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 0xFFFF_FFFF),
        (3, 2, 1, 0, 0, 0, 0, 0xFFFF_FFFF),
        (4, 1, 0, 0, 0, 0, 0, 255),
    ]
    if profile == 1:
        t["records"] = [(0, 0, 0, 2, 0, 0, 0, 0)]
        t["fields"] = [(0, 0, 0, 3), (1, 0, 1, 3)]
        t["machines"] = [(0, 0, 2, 0, 0, 4, 0, 0, 0, 3, 0)]
        t["blocks"] = [
            (0, 0, 2, 0, 0, 0, 0, 0, 23, 0),
            (1, 0, 2, 0, 0, 0, 0, 23, 1, 1),
            (2, 0, 2, 0, 0, 0, 0, 24, 1, 2),
        ]
        t["operations"] = [
            (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
            (1, 0, 0, 3, 2, 0, 1, 3, 0, 1, 0, 0),
            (2, 0, 0, 1, 1, 0, 0, 3, 1, 0, 0xFFFF_FFFF, 0),
            (3, 0, 0, 6, 0, 0, IR.NO_ID, IR.NO_ID, 1, 2, 0, 0),
            (4, 0, 0, 2, 2, 0, 2, 0, 3, 0, 0, 0),
            (5, 0, 0, 3, 2, 0, 3, 3, 3, 1, 1, 0),
            (6, 0, 0, 1, 1, 0, 1, 3, 4, 0, 0xFFFF_FFFA, 0),
            (7, 0, 0, 6, 0, 0, IR.NO_ID, IR.NO_ID, 4, 2, 0, 0),
            (8, 0, 0, 2, 2, 0, 4, 0, 6, 0, 0, 0),
            (9, 0, 0, 3, 2, 0, 5, 3, 6, 1, 0, 0),
            (10, 0, 0, 2, 2, 0, 6, 0, 7, 0, 0, 0),
            (11, 0, 0, 3, 2, 0, 7, 3, 7, 1, 0, 0),
            (12, 0, 0, 2, 2, 0, 8, 0, 8, 0, 0, 0),
            (13, 0, 0, 3, 2, 0, 9, 3, 8, 1, 1, 0),
            (14, 0, 0, 5, 1, 0, 2, 3, 9, 1, 0, 0),
            (15, 0, 0, 5, 1, 0, 3, 3, 10, 1, 0, 0),
            (16, 0, 0, 26, 1, 0, 4, 3, 11, 2, 0, 0),
            (17, 0, 0, 6, 0, 0, IR.NO_ID, IR.NO_ID, 13, 2, 0, 0),
            (18, 0, 0, 2, 2, 0, 10, 0, 15, 0, 0, 0),
            (19, 0, 0, 3, 2, 0, 11, 3, 15, 1, 0, 0),
            (20, 0, 0, 1, 1, 0, 5, 3, 16, 0, 5, 0),
            (21, 0, 0, 5, 1, 0, 6, 3, 16, 1, 0, 0),
            (22, 0, 0, 18, 1, 0, 7, 1, 17, 2, 0, 0),
            (23, 0, 1, 1, 1, 0, 8, 4, 19, 0, 70, 0),
            (24, 0, 2, 1, 1, 0, 9, 4, 19, 0, 0, 0),
        ]
        t["operands"] = [(0,), (1,), (0,), (2,), (3,), (1,), (4,), (6,),
                         (8,), (9,), (7,), (3,), (2,), (5,), (4,), (10,),
                         (11,), (6,), (5,)]
        t["terminators"] = [
            (0, 0, 0, 2, 0, 0, 7, 1, 19, 0, 2, 19, 0, 0, 0),
            (1, 0, 1, 4, 0, 0, 8, IR.NO_ID, 19, 0, IR.NO_ID, 19, 0, 0, 0),
            (2, 0, 2, 4, 0, 0, 9, IR.NO_ID, 19, 0, IR.NO_ID, 19, 0, 0, 0),
        ]
        return encode_ckir(t, 10, 12)

    t["records"] = [(0, 0, 0, 1, 0, 0, 0, 0)]
    t["fields"] = [(0, 0, 0, 3)]
    t["machines"] = [(0, 0, 2, 0, 0, 4, 0, 0, 0, 1, 0)]
    t["blocks"] = [(0, 0, 2, 0, 0, 0, 0, 0, 13, 0)]
    t["operations"] = [
        (0, 0, 0, 2, 2, 0, 0, 0, 0, 0, 0, 0),
        (1, 0, 0, 3, 2, 0, 1, 3, 0, 1, 0, 0),
        (2, 0, 0, 1, 1, 0, 0, 3, 1, 0, 0, 0),
        (3, 0, 0, 6, 0, 0, IR.NO_ID, IR.NO_ID, 1, 2, 0, 0),
        (4, 0, 0, 2, 2, 0, 2, 0, 3, 0, 0, 0),
        (5, 0, 0, 3, 2, 0, 3, 3, 3, 1, 0, 0),
        (6, 0, 0, 2, 2, 0, 4, 0, 4, 0, 0, 0),
        (7, 0, 0, 3, 2, 0, 5, 3, 4, 1, 0, 0),
        (8, 0, 0, 1, 1, 0, 1, 3, 5, 0, 1, 0),
        (9, 0, 0, 5, 1, 0, 2, 3, 5, 1, 0, 0),
        (10, 0, 0, 26, 1, 0, 3, 3, 6, 2, 0, 0),
        (11, 0, 0, 6, 0, 0, IR.NO_ID, IR.NO_ID, 8, 2, 0, 0),
        (12, 0, 0, 1, 1, 0, 4, 4, 10, 0, 70, 0),
    ]
    t["operands"] = [(0,), (1,), (0,), (2,), (4,), (5,), (2,), (1,), (3,), (3,)]
    t["terminators"] = [(0, 0, 0, 4, 0, 0, 4, IR.NO_ID, 10, 0,
                         IR.NO_ID, 10, 0, 0, 0)]
    return encode_ckir(t, 5, 6)


CKIR = {1: source_ckir(1), 2: source_ckir(2)}

WITNESS = {
    1: bytes.fromhex(
        "4f4d47525357350005000000000054002c020000010000000000000001000000"
        "0200000005000000010000000200000001000000000000000300000000000000"
        "0000000000000000000000000000000000000000000000000000000000000000ffffffff"
        "0000000000000000000000000000000002000000000000000000000002010000"
        "570000000b00000000000000ffffffff00000000010000000000000000000000"
        "050000000b000000000000000100000002000000000000000100000064000000"
        "0300000000000000000000000400000000000000000000000000000000000000"
        "0100000003000000000000000000000000000000010000000200000002000000"
        "000000000000000000000000ffffffff03000000020100000000000000000000"
        "00000000ffffffff0400000001000000000000000000000000000000ff000000"
        "0000000000000000000000000000000002000000000000000000000000000000"
        "0000000003000000170000000600000001000000000000000100000003000000"
        "3400000005000000000000000100000000000000020000000400000000000000"
        "0000000000000000030000000000000000000000000000000000000002000000"
        "7f00000042010000ffffffff0000000000000000000000000100000000000000"
        "01000000020000005c0100005f01000048010000060000000000000000000000"
        "020000000000000002000000020000007f010000810100006b01000006000000"
        "0000000000000000"
    ),
    2: bytes.fromhex(
        "4f4d4752535735000500000000005400c4010000010000000000000001000000"
        "0200000005000000010000000100000001000000000000000100000000000000"
        "0000000000000000000000000000000000000000000000000000000000000000ffffffff"
        "0000000000000000000000000000000002000000000000000000000002010000"
        "3b0000000b00000000000000ffffffff00000000010000000000000000000000"
        "050000000b000000000000000100000002000000000000000100000048000000"
        "0300000000000000000000000400000000000000000000000000000000000000"
        "0100000003000000000000000000000000000000010000000200000002000000"
        "000000000000000000000000ffffffff03000000020100000000000000000000"
        "00000000ffffffff0400000001000000000000000000000000000000ff000000"
        "0000000000000000000000000000000001000000000000000000000000000000"
        "0000000003000000170000000600000000000000010000000000000002000000"
        "0400000000000000000000000000000001000000000000000000000000000000"
        "0000000002000000630000009e000000ffffffff000000000000000000000000"
    ),
}

ELF_HEADER = bytes.fromhex(
    "7f454c4602010100000000000000000002003e00010000000010400000000000"
    "4000000000000000000000000000000000000000400038000200000000000000"
    "0100000005000000000000000000000000004000000000000000400000000000"
    "0020000000000000002000000000000000100000000000000100000006000000"
    "0020000000000000002040000000000000204000000000000000000000000000"
    "00100000000000000010000000000000"
)
ELF_TEXT = {
    1: bytes.fromhex(
        "488d3df90f0000e80e0000000fb6f8b8e70000000f050f0b0f0b554889e54881ec900000004889bdf8ffffff488b85f8ffffff488985c8ffffff488b85c8ffffff480500000000488985c0ffffffb8ffffffff8985f4ffffff488b85c0ffffff4989c28b85f4ffffff3d000000000f82a4ffffff3dffffffff0f8799ffffff418902488b85f8ffffff488985b8ffffff488b85b8ffffff480504000000488985b0ffffffb8faffffff8985f0ffffff488b85b0ffffff4989c28b85f0ffffff3d000000000f824effffff3dffffffff0f8743ffffff418902488b85f8ffffff488985a8ffffff488b85a8ffffff480500000000488985a0ffffff488b85f8ffffff48898598ffffff488b8598ffffff48050000000048898590ffffff488b85f8ffffff48898588ffffff488b8588ffffff48050400000048898580ffffff488b8580ffffff8b008985ecffffff488b8590ffffff8b008985e8ffffff8b85e8ffffff2b85ecffffff0f82aafeffff3d000000000f829ffeffff3dffffffff0f8794feffff8985e4ffffff488b85a0ffffff4989c28b85e4ffffff3d000000000f8273feffff3dffffffff0f8768feffff418902488b85f8ffffff48898578ffffff488b8578ffffff48050000000048898570ffffffb8050000008985e0ffffff488b8570ffffff8b008985dcffffff8b85dcffffff3b85e0ffffff0f94c00fb6c08985d8ffffff8b85d8ffffff85c00f8405000000e905000000e929000000b8460000008985d4ffffff8b85d4ffffff3d000000000f82ddfdffff3dff0000000f87d2fdffffc9c3b8000000008985d0ffffff8b85d0ffffff3d000000000f82b4fdffff3dff0000000f87a9fdffffc9c3"
    ),
    2: bytes.fromhex(
        "488d3df90f0000e80e0000000fb6f8b8e70000000f050f0b0f0b554889e54881ec500000004889bdf8ffffff488b85f8ffffff488985d8ffffff488b85d8ffffff480500000000488985d0ffffffb8000000008985f4ffffff488b85d0ffffff4989c28b85f4ffffff3d000000000f82a4ffffff3dffffffff0f8799ffffff418902488b85f8ffffff488985c8ffffff488b85c8ffffff480500000000488985c0ffffff488b85f8ffffff488985b8ffffff488b85b8ffffff480500000000488985b0ffffffb8010000008985f0ffffff488b85b0ffffff8b008985ecffffff8b85ecffffff2b85f0ffffff0f8226ffffff3d000000000f821bffffff3dffffffff0f8710ffffff8985e8ffffff488b85c0ffffff4989c28b85e8ffffff3d000000000f82effeffff3dffffffff0f87e4feffff418902b8460000008985e4ffffff8b85e4ffffff3d000000000f82c5feffff3dff0000000f87bafeffffc9c3"
    ),
}


def expected_elf(profile: int) -> bytes:
    result = bytearray(8192)
    result[:len(ELF_HEADER)] = ELF_HEADER
    result[4096:4096 + len(ELF_TEXT[profile])] = ELF_TEXT[profile]
    return bytes(result)


ELF = {profile: expected_elf(profile) for profile in (1, 2)}

COMMON = r"""
proc fbyte(index) { return byte[1048576+index] }
proc fu32(at) { return fbyte(at)+fbyte(at+1)*256+fbyte(at+2)*65536+fbyte(at+3)*16777216 }
proc omgbyte(index) { return fbyte(40+index) }
proc witnessbyte(index) { return fbyte(word[700016]+index) }
proc ckirbyte(index) { return fbyte(word[700032]+index) }
proc elfbyte(index) { return fbyte(word[700048]+index) }
proc omgrfn15_read_frame() {
    let n=0 let overflow=0 let c=read_byte() let cursor=0
    state read { to one when(c>=0) to header }
    state one { to over when(n>=4497544) byte[1048576+n]=c n=n+1 c=read_byte() to read }
    state over { overflow=1 c=read_byte() to read }
    state header { to exhausted when(overflow==1) to bad when(n<40) to magic }
    state magic {
        to bad when(fbyte(0)!='O') to bad when(fbyte(1)!='M') to bad when(fbyte(2)!='G')
        to bad when(fbyte(3)!='R') to bad when(fbyte(4)!='F') to bad when(fbyte(5)!='N')
        to bad when(fbyte(6)!='F') to bad when(fbyte(7)!=0) to bad when(fu32(8)!=15)
        word[700000]=fu32(12) word[700008]=fu32(16) word[700024]=fu32(20)
        word[700040]=fu32(24) word[700056]=fu32(28) word[700064]=fu32(32) word[700072]=fu32(36)
        to bad when(word[700000]!=1) to bad when(word[700008]<1) to bad when(word[700024]<1)
        to bad when(word[700040]<1) to bad when(word[700056]<1)
        to exhausted when(word[700008]>267280) to exhausted when(word[700024]>524288)
        to exhausted when(word[700040]>2522192) to exhausted when(word[700056]>1183744)
        word[700016]=40+word[700008] word[700032]=word[700016]+word[700024]
        word[700048]=word[700032]+word[700040] cursor=word[700048]+word[700056]
        to bad when(cursor!=n) to bad when(word[700072]>255)
        to bad when(word[700072]!=word[700064]%256) return 0
    }
    state exhausted { return 252 }
    state bad { return 251 }
}
"""
SOURCE_COMMON = re.sub(
    r"proc witnessbyte\(index\).*?proc elfbyte\(index\) \{[^\n]+\}\n", "", COMMON,
    flags=re.S,
)


def sparse_assignments(contents: bytes, base: int) -> list[str]:
    return [f"byte[{base + i}]={value}" for i, value in enumerate(contents) if value]


def init_procedures(label: str, by_profile: dict[int, bytes], base: int) -> str:
    pieces: list[str] = []
    for profile, contents in by_profile.items():
        assignments = sparse_assignments(contents, base)
        chunks = [assignments[i:i + 48] for i in range(0, len(assignments), 48)]
        for index, chunk in enumerate(chunks):
            tail = f" {label}_p{profile}_{index + 1}()" if index + 1 < len(chunks) else ""
            pieces.append(f"proc {label}_p{profile}_{index}() {{ {' '.join(chunk)}{tail} return 0 }}")
        pieces.append(f"proc {label}_p{profile}() {{ {label}_p{profile}_0() return 0 }}")
    pieces.append(
        f"proc {label}_init(profile) {{ state p1 {{ to p2 when(profile==2) "
        f"{label}_p1() return 0 }} state p2 {{ {label}_p2() return 0 }} }}"
    )
    return "\n".join(pieces) + "\n"


def exact_component(label: str, byte_fn: str, length_word: int,
                    by_profile: dict[int, bytes], base: int) -> str:
    lengths = {profile: len(contents) for profile, contents in by_profile.items()}
    return init_procedures(label, by_profile, base) + f"""
proc {label}_check(profile) {{
    let i=0 let expected=0
    {label}_init(profile)
    state length {{ to p2 when(profile==2) expected={lengths[1]} to compare_length }}
    state p2 {{ expected={lengths[2]} to compare_length }}
    state compare_length {{ to bad when(word[{length_word}]!=expected) to bytes }}
    state bytes {{ to one when(i<expected) return 0 }}
    state one {{ to bad when({byte_fn}(i)!=byte[{base}+i]) i=i+1 to bytes }}
    state bad {{ return 251 }}
}}
"""


def profile_selector() -> str:
    return f"""
proc omgrfn15_profile() {{
    state success {{ to underflow when(word[700040]=={len(CKIR[2])}) to yes when(word[700040]=={len(CKIR[1])}) return 0 }}
    state yes {{ return 1 }}
    state underflow {{ return 2 }}
}}
"""


def source_model() -> dict[int, bytes]:
    return {
        1: (GATES / "fixtures/ckir13-full-u32-subtract/success.omg").read_bytes(),
        2: (GATES / "fixtures/ckir13-full-u32-subtract/underflow.omg").read_bytes(),
    }


def source_profile_code(sources: dict[int, bytes], base: int = 7_000_000) -> str:
    lengths = {profile: len(source) for profile, source in sources.items()}
    return init_procedures("source_expected", sources, base) + f"""
proc source_expected_clear() {{ let i=0 state loop {{ to one when(i<{max(lengths.values())}) return 0 }} state one {{ byte[{base}+i]=0 i=i+1 to loop }} }}
proc source_contains(profile) {{
    let i=0 let j=0 let limit=word[700008] let wanted=0
    source_expected_clear()
    state length {{ to p2 when(profile==2) wanted={lengths[1]} source_expected_init(profile) to starts }}
    state p2 {{ wanted={lengths[2]} source_expected_init(profile) to starts }}
    state starts {{ to start when(i<=limit-wanted) return 0 }}
    state start {{ j=0 to bytes }}
    state bytes {{ to found when(j==wanted) to mismatch when(omgbyte(i+j)!=byte[{base}+j]) j=j+1 to bytes }}
    state mismatch {{ i=i+1 to starts }}
    state found {{ return 1 }}
}}
proc omgrfn15_source_profile() {{
    let success=source_contains(1) let underflow=source_contains(2)
    state exact {{ to p1 when(success==1) to p2 when(underflow==1) return 0 }}
    state p1 {{ to bad when(underflow!=0) return 1 }}
    state p2 {{ return 2 }}
    state bad {{ return 0 }}
}}
"""


def procedures(source: str) -> tuple[int, int]:
    pattern = re.compile(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\(([^)]*)\)\s*\{")
    count = maximum = 0
    for match in pattern.finditer(source):
        depth, cursor = 1, match.end()
        while depth:
            depth += (source[cursor] == "{") - (source[cursor] == "}")
            cursor += 1
        params = sum(bool(item.strip()) for item in match.group(2).split(","))
        maximum = max(maximum, params + len(re.findall(
            r"\blet\s+[A-Za-z_]\w*", source[match.start():cursor])))
        count += 1
    return count, maximum


def write_checked(output: Path, name: str, source: str) -> str:
    count, locals_count = procedures(source)
    if count > 128 or locals_count > 32:
        raise ValueError(f"{name} exceeds Beta shape: {count}/128, {locals_count}/32")
    (output / f"{name}.beta").write_text(source, encoding="ascii")
    return f"{name}\t{count}\t{locals_count}\n"
