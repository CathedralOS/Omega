#!/usr/bin/env python3
"""Focused OMGRFN14 checker-source model.

This is a pure materializer, not an acceptance oracle.  It spells the frozen
CKIR12 fixture and conservative ELF template into small persisted-Beta owners;
the generated executables still consume and reject the untrusted frame.
"""

from __future__ import annotations

import importlib.util
import re
import sys
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
GATES = REPO / "source/on-ramp/omega-bootstrap/gates"
sys.path.insert(0, str(GATES))


def load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


FIXTURE = load("omgrfn14_ckir_fixture", GATES / "delta-checked-ir-v12-fixture.py")


def produced_ckir(byte_values: tuple[int, ...]) -> bytes:
    """Construct the frozen producer-backed CKIR12 rows without the producer."""
    tables = {name: [] for name in FIXTURE.ir12.TABLE_ORDER}
    tables["types"] = [
        (0, 4, 0, 0, 0, 0, 0, 0),
        (1, 3, 0, 0, 0, 0, 0, 1),
        (2, 2, 0, 0, 0, 0, 0, 0x7FFF_FFFF),
        (3, 1, 0, 0, 0, 0, 0, 255),
        (4, 7, 0, 0, 3, 0, 0, 0),
    ]
    tables["records"] = [(0, 0, 0, 0, 0, 0, 0, 0)]
    tables["machines"] = [(0, 0, 2, 0, 0, 3, 0, 0, 0, 5, 0)]
    tables["blocks"] = [
        (0, 0, 2, 0, 0, 0, 0, 0, 1, 0),
        (1, 0, 2, 0, 0, 0, 1, 1, 1, 1),
        (2, 0, 2, 0, 0, 1, 2, 2, 0, 2),
        (3, 0, 2, 0, 0, 3, 0, 2, 1, 3),
        (4, 0, 2, 1, 0, 3, 1, 3, 2, 4),
    ]
    tables["block_params"] = [
        (0, 1, 0, 4, 0), (1, 2, 0, 3, 1),
        (2, 2, 1, 4, 2), (3, 4, 0, 4, 3),
    ]
    if byte_values:
        tables["constants"] = [(0, 3, 0, 0, byte_values[0], 0), (1, 4, 0, 1, 0, 0)]
        tables["constant_children"] = [(0,)]
        root = 1
    else:
        tables["constants"] = [(0, 4, 0, 0, 0, 0)]
        root = 0
    tables["operations"] = [
        (0, 0, 0, 22, 1, 0, 4, 4, 0, 0, root, 0),
        (1, 0, 1, 23, 1, 0, 5, 1, 0, 1, 0, 0),
        (2, 0, 3, 1, 1, 0, 6, 3, 1, 0, 70, 0),
        (3, 0, 4, 24, 1, 0, 7, 3, 1, 1, 0, 0),
        (4, 0, 4, 25, 1, 0, 8, 4, 2, 1, 0, 0),
    ]
    tables["operands"] = [(0,), (3,), (3,), (4,), (0,), (7,), (8,)]
    no_id = FIXTURE.NO_ID
    tables["terminators"] = [
        (0, 0, 0, 1, 0, 0, no_id, 1, 3, 1, no_id, 4, 0, 0, 0),
        (1, 0, 1, 2, 0, 0, 5, 4, 4, 1, 3, 5, 0, 0, 0),
        (2, 0, 2, 4, 0, 0, 1, no_id, 5, 0, no_id, 5, 0, 0, 0),
        (3, 0, 3, 4, 0, 0, 6, no_id, 5, 0, no_id, 5, 0, 0, 0),
        (4, 0, 4, 1, 0, 0, no_id, 2, 5, 2, no_id, 7, 0, 0, 0),
    ]
    return FIXTURE.encode(tables, values=9)


CKIR = {1: produced_ckir((70,)), 2: produced_ckir(())}

ELF_HEADER = bytes.fromhex(
    "7f454c4602010100000000000000000002003e00010000000010400000000000"
    "4000000000000000000000000000000000000000400038000300000000000000"
    "0100000005000000000000000000000000004000000000000000400000000000"
    "0020000000000000002000000000000000100000000000000100000004000000"
    "0020000000000000002040000000000000204000000000000010000000000000"
    "0010000000000000001000000000000001000000060000000030000000000000"
    "0030400000000000003040000000000000000000000000000010000000000000"
    "0010000000000000000000000000000000000000000000000000000000000000"
)
ELF_TEXT_ONE = bytes.fromhex(
    "488d3df91f0000e80e0000000fb6f8b8e70000000f050f0b0f0b554889e54881"
    "ec800000004889bdf8ffffff4c8d9da8ffffff488d05c60f0000498903b8010000"
    "00498943084c899dd0ffffff488b85d0ffffff48898590ffffff488b8590ffffff"
    "488985f0ffffffe9000000004c8b9df0ffffff49837b08000f95c00fb6c08985cc"
    "ffffff8b85ccffffff85c00f8421000000488b85f0ffffff48898590ffffff488b"
    "8590ffffff488985d8ffffffe94c000000e91e0000008b85ecffffff3d00000000"
    "0f824dffffff3dff0000000f8742ffffffc9c3b8460000008985c8ffffff8b85c8"
    "ffffff3d000000000f8224ffffff3dff0000000f8719ffffffc9c34c8b9dd8ffff"
    "ff49837b08000f8405ffffff498b030fb6008985c4ffffff4c8b9dd8ffffff4c8d"
    "9598ffffff49837b08000f84e0feffff498b034883c001498902498b43084883e8"
    "01498942084c8995b8ffffff8b85c4ffffff898590ffffff488b85b8ffffff4889"
    "8588ffffff8b8590ffffff3d000000000f8298feffff3dff0000000f878dfeffff"
    "8985ecffffff488b8588ffffff488985e0ffffffe916ffffff"
)


def expected_elf(profile: int) -> bytes:
    result = bytearray(12_288)
    result[:len(ELF_HEADER)] = ELF_HEADER
    text = bytearray(ELF_TEXT_ONE)
    if profile == 2:
        text[62] = 0
    result[4096:4096 + len(text)] = text
    if profile == 1:
        result[8192] = 70
    return bytes(result)


ELF = {profile: expected_elf(profile) for profile in (1, 2)}


COMMON = r"""
proc fbyte(index) { return byte[1048576+index] }
proc fu32(at) { return fbyte(at)+fbyte(at+1)*256+fbyte(at+2)*65536+fbyte(at+3)*16777216 }
proc omgbyte(index) { return fbyte(40+index) }
proc witnessbyte(index) { return fbyte(word[700016]+index) }
proc witnessu32(index) { return witnessbyte(index)+witnessbyte(index+1)*256+witnessbyte(index+2)*65536+witnessbyte(index+3)*16777216 }
proc ckirbyte(index) { return fbyte(word[700032]+index) }
proc ckiri32(index) { return ckirbyte(index)+ckirbyte(index+1)*256+ckirbyte(index+2)*65536+ckirbyte(index+3)*16777216 }
proc elfbyte(index) { return fbyte(word[700048]+index) }

proc omgrfn14_read_frame() {
    let n=0 let overflow=0 let c=read_byte() let cursor=0
    state read { to one when(c>=0) to header }
    state one { to over when(n>=4497544) byte[1048576+n]=c n=n+1 c=read_byte() to read }
    state over { overflow=1 c=read_byte() to read }
    state header { to exhausted when(overflow==1) to bad when(n<40) to magic }
    state magic {
        to bad when(fbyte(0)!='O') to bad when(fbyte(1)!='M') to bad when(fbyte(2)!='G')
        to bad when(fbyte(3)!='R') to bad when(fbyte(4)!='F') to bad when(fbyte(5)!='N')
        to bad when(fbyte(6)!='E') to bad when(fbyte(7)!=0) to bad when(fu32(8)!=14)
        word[700000]=fu32(12) word[700008]=fu32(16) word[700024]=fu32(20)
        word[700040]=fu32(24) word[700056]=fu32(28) word[700064]=fu32(32) word[700072]=fu32(36)
        to bad when(word[700000]!=1) to bad when(word[700008]<1) to bad when(word[700024]<1)
        to bad when(word[700040]<1) to bad when(word[700056]<1)
        to exhausted when(word[700008]>267280) to exhausted when(word[700024]>524288)
        to exhausted when(word[700040]>2522192) to exhausted when(word[700056]>1183744)
        word[700016]=40+word[700008] word[700032]=word[700016]+word[700024]
        word[700048]=word[700032]+word[700040] cursor=word[700048]+word[700056]
        to bad when(cursor!=n) to bad when(word[700064]>4294967295)
        to bad when(word[700072]>255) to bad when(word[700072]!=word[700064]%256)
        return 0
    }
    state exhausted { return 252 }
    state bad { return 251 }
}
"""

SOURCE_COMMON = re.sub(
    r"proc ckirbyte\(index\).*?proc elfbyte\(index\) \{[^\n]+\}\n",
    "",
    COMMON,
    flags=re.S,
)


def sparse_assignments(contents: bytes, base: int) -> list[str]:
    return [f"byte[{base + index}]={value}" for index, value in enumerate(contents) if value]


def init_procedures(label: str, by_profile: dict[int, bytes], base: int) -> str:
    pieces: list[str] = []
    for profile, contents in by_profile.items():
        assignments = sparse_assignments(contents, base)
        chunks = [assignments[index:index + 48] for index in range(0, len(assignments), 48)]
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
proc omgrfn14_profile() {{
    state one {{ to empty when(word[700040]=={len(CKIR[2])}) to yes when(word[700040]=={len(CKIR[1])}) return 0 }}
    state yes {{ return 1 }}
    state empty {{ return 2 }}
}}
"""


WITNESS_CHECK = r"""
proc omgrfn14_witness_check() {
    let types=0 let records=0 let fields=0 let sums=0 let cases=0 let payloads=0
    let machines=0 let mparams=0 let blocks=0 let bparams=0 let type_at=0
    let machine_at=0 let mparam_at=0 let block_at=0 let bparam_at=0
    let i=0 let slice=4294967295 let element=0 let slice_params=0
    state header {
        to bad when(word[700024]<84)
        to bad when(witnessbyte(0)!='O') to bad when(witnessbyte(1)!='M')
        to bad when(witnessbyte(2)!='G') to bad when(witnessbyte(3)!='R')
        to bad when(witnessbyte(4)!='S') to bad when(witnessbyte(5)!='W')
        to bad when(witnessbyte(6)!='4') to bad when(witnessbyte(7)!=0)
        to bad when(witnessbyte(8)!=4) to bad when(witnessbyte(9)!=0)
        to bad when(witnessbyte(10)!=0) to bad when(witnessbyte(11)!=0)
        to bad when(witnessbyte(12)!=0) to bad when(witnessbyte(13)!=0)
        to bad when(witnessbyte(14)!=84) to bad when(witnessbyte(15)!=0)
        to bad when(witnessu32(16)!=word[700024]) to bad when(witnessu32(80)!=0)
        types=witnessu32(36) records=witnessu32(40) fields=witnessu32(44)
        machines=witnessu32(48) mparams=witnessu32(52) blocks=witnessu32(56)
        bparams=witnessu32(60) sums=witnessu32(64) cases=witnessu32(68) payloads=witnessu32(72)
        to bad when(witnessu32(20)>16) to bad when(witnessu32(24)>32)
        to bad when(witnessu32(28)>256) to bad when(witnessu32(32)>256)
        to bad when(types>8192) to bad when(records>128) to bad when(fields>8192)
        to bad when(sums>128) to bad when(cases>4096) to bad when(payloads>4096)
        to bad when(machines>128) to bad when(mparams>896)
        to bad when(blocks>2048) to bad when(bparams>4096)
        type_at=84+witnessu32(20)*36+witnessu32(24)*48+witnessu32(28)*28+witnessu32(32)*28
        machine_at=type_at+types*24+records*24+fields*24+sums*24+cases*28+payloads*24
        mparam_at=machine_at+machines*40 block_at=mparam_at+mparams*24
        bparam_at=block_at+blocks*40 to extent
    }
    state extent { to bad when(bparam_at+bparams*24!=word[700024]) to types_loop }
    state types_loop { to type_one when(i<types) i=0 to require_slice }
    state type_one { to slice_row when(witnessbyte(type_at+i*24+4)==7) i=i+1 to types_loop }
    state slice_row {
        to bad when(slice!=4294967295) slice=i
        to bad when(witnessbyte(type_at+i*24+5)!=0)
        to bad when(witnessbyte(type_at+i*24+6)!=0) to bad when(witnessbyte(type_at+i*24+7)!=0)
        element=witnessu32(type_at+i*24+8)
        to bad when(witnessu32(type_at+i*24+12)!=0)
        to bad when(witnessu32(type_at+i*24+16)!=0)
        to bad when(witnessu32(type_at+i*24+20)!=0)
        i=i+1 to types_loop
    }
    state require_slice { to bad when(slice==4294967295) to bad when(element>=types) to element }
    state element {
        to bad when(witnessbyte(type_at+element*24+4)!=1)
        to bad when(witnessbyte(type_at+element*24+5)!=0)
        to bad when(witnessu32(type_at+element*24+8)!=0)
        to bad when(witnessu32(type_at+element*24+12)!=0)
        to bad when(witnessu32(type_at+element*24+16)!=0)
        to bad when(witnessu32(type_at+element*24+20)!=255)
        i=0 to mparams_loop
    }
    state mparams_loop { to mparam_one when(i<mparams) i=0 to bparams_loop }
    state mparam_one { to mparam_slice when(witnessu32(mparam_at+i*24+12)==slice) i=i+1 to mparams_loop }
    state mparam_slice { slice_params=slice_params+1 i=i+1 to mparams_loop }
    state bparams_loop { to bparam_one when(i<bparams) to done }
    state bparam_one { to bparam_slice when(witnessu32(bparam_at+i*24+12)==slice) i=i+1 to bparams_loop }
    state bparam_slice { slice_params=slice_params+1 i=i+1 to bparams_loop }
    state done { to bad when(slice_params<1) return 0 }
    state bad { return 251 }
}
"""


def source_model() -> dict[int, bytes]:
    paths = {
        1: GATES / "fixtures/ckir12-static-byte-view/one-byte.omg",
        2: GATES / "fixtures/ckir12-static-byte-view/empty.omg",
    }
    missing = [str(path) for path in paths.values() if not path.is_file()]
    if missing:
        raise FileNotFoundError("CKIR12 producer source fixtures not landed: " + ", ".join(missing))
    return {profile: path.read_bytes() for profile, path in paths.items()}


def source_profile_code(sources: dict[int, bytes], base: int = 7_000_000) -> str:
    init = init_procedures("source_expected", sources, base)
    lengths = {profile: len(source) for profile, source in sources.items()}
    return init + f"""
proc source_expected_clear() {{
    let i=0
    state loop {{ to one when(i<{max(lengths.values())}) return 0 }}
    state one {{ byte[{base}+i]=0 i=i+1 to loop }}
}}
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

proc omgrfn14_source_profile() {{
    let one=source_contains(1) let empty=source_contains(2)
    state exact {{ to p1 when(one==1) to p2 when(empty==1) return 0 }}
    state p1 {{ to bad when(empty!=0) return 1 }}
    state p2 {{ return 2 }}
    state bad {{ return 0 }}
}}
"""


def procedures(source: str) -> tuple[int, int]:
    pattern = re.compile(r"(?m)^proc\s+([A-Za-z_]\w*)\s*\(([^)]*)\)\s*\{")
    count = maximum = 0
    for match in pattern.finditer(source):
        depth = 1
        cursor = match.end()
        while depth:
            depth += (source[cursor] == "{") - (source[cursor] == "}")
            cursor += 1
        params = sum(bool(item.strip()) for item in match.group(2).split(","))
        maximum = max(maximum, params + len(re.findall(r"\blet\s+[A-Za-z_]\w*", source[match.start():cursor])))
        count += 1
    return count, maximum


def write_checked(output: Path, name: str, source: str) -> str:
    count, locals_count = procedures(source)
    if count > 128 or locals_count > 32:
        raise ValueError(f"{name} exceeds Beta shape: {count}/128, {locals_count}/32")
    (output / f"{name}.beta").write_text(source, encoding="ascii")
    return f"{name}\t{count}\t{locals_count}\n"
