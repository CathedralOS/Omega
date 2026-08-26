#!/usr/bin/env python3
"""Materialize bounded persisted-Beta OMGRFN16 R1/R2 owners."""

from __future__ import annotations

import argparse
import importlib.util
import re
from pathlib import Path


HERE = Path(__file__).resolve().parent


def load_shared():
    path = HERE / "omgrfn8-materialize-r1-r2.py"
    spec = importlib.util.spec_from_file_location("omgrfn16_shared_r1_r2", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def replace(source: str, old: str, new: str, count: int = 1) -> str:
    actual = source.count(old)
    if actual != count:
        raise ValueError(f"OMGRFN16 R2 anchor {actual}, expected {count}: {old}")
    return source.replace(old, new)


R1 = r"""
proc fb(i){return byte[1048576+i]}
proc fu(i){return fb(i)+fb(i+1)*256+fb(i+2)*65536+fb(i+3)*16777216}
proc main(){
 let n=0 let over=0 let c=read_byte() let cursor=0 let flags=0 let result=0 let exit=0
 state read{to one when(c>=0) to header}
 state one{to overflow when(n>=4497544) byte[1048576+n]=c n=n+1 c=read_byte() to read}
 state overflow{over=1 c=read_byte() to read}
 state header{to exhausted when(over==1) to bad when(n<40) to magic}
 state magic{
  to bad when(fb(0)!='O') to bad when(fb(1)!='M') to bad when(fb(2)!='G')
  to bad when(fb(3)!='R') to bad when(fb(4)!='F') to bad when(fb(5)!='N')
  to bad when(fb(6)!='G') to bad when(fb(7)!=0) to bad when(fu(8)!=16)
  flags=fu(12) result=fu(32) exit=fu(36)
  to bad when(flags!=1&&flags!=3)
  to success when(flags==1) to trap
 }
 state success{to bad when(exit!=result%256) to components}
 state trap{to bad when(result!=0-1) to bad when(exit!=0-1) to components}
 state components{
  to bad when(fu(16)<1) to bad when(fu(20)<1) to bad when(fu(24)<1) to bad when(fu(28)<1)
  to exhausted when(fu(16)>267280) to exhausted when(fu(20)>524288)
  to exhausted when(fu(24)>2522192) to exhausted when(fu(28)>1183744)
  cursor=40 to bad when(fu(16)>n-cursor) cursor=cursor+fu(16)
  to bad when(fu(20)>n-cursor) cursor=cursor+fu(20)
  to bad when(fu(24)>n-cursor) cursor=cursor+fu(24)
  to bad when(fu(28)!=n-cursor) return 0
 }
 state exhausted{return 252}
 state bad{return 251}
}
"""


def patch_r2(source: str) -> str:
    source = replace(source, "to v13 when (omgrfn4_l2_frame_byte(6)=='D') to bad",
                     "to v16 when (omgrfn4_l2_frame_byte(6)=='G') to bad")
    source = replace(source,
        "state v13 { to bad when (omgrfn5_r2_u32(8)!=13) word[879088]=13 to components }",
        "state v16 { to bad when (omgrfn5_r2_u32(8)!=16) word[879088]=16 to components }")
    source = replace(source, "to bad when (word[500016]>1)",
                     "to bad when (word[500016]!=1&&word[500016]!=3)")
    source = replace(source,
        "to bad when (word[500080]!=n-cursor) to library when (word[500016]==0) to entry",
        "to bad when (word[500080]!=n-cursor) return 0")
    source = re.sub(r"\n    state library \{[^\n]+\}\n    state entry \{[^\n]+\}", "", source, count=1)
    source = replace(source,
        "state trapping { kind=2 flags=1 hi=2147483647 to emit }",
        "state trapping { kind=2 flags=1 hi=0-1 to emit }")
    source = replace(source,
        "omgrfn4_l2_put_byte('S') omgrfn4_l2_put_byte('W') omgrfn4_l2_put_byte('0'+word[879376]) omgrfn4_l2_put_byte(0)",
        "omgrfn4_l2_put_byte('S') omgrfn4_l2_put_byte('W') omgrfn4_l2_put_byte('7') omgrfn4_l2_put_byte(0)", 2)
    source = replace(source,
        "omgrfn4_l2_put_u16(word[879376]) omgrfn4_l2_put_u16(0) omgrfn4_l2_put_u16(0)",
        "omgrfn4_l2_put_u16(7) omgrfn4_l2_put_u16(0) omgrfn4_l2_put_u16(0)", 2)
    source = replace(source,
        "state parse { to parse_one when (source<word[500320]) to v7 when (word[879088]==7) to v8 when (word[879088]==8) to v9 when (word[879088]==9) to v10 when (word[879088]==10) to v11 when (word[879088]==11) to v12 when (word[879088]==12) to v13 }",
        "state parse { to parse_one when (source<word[500320]) to v16 }")
    source = replace(source,
        "state v13 { to reject when (word[879472]!=4) omgrfn8_r2_select_resolution() status=omgrfn5_r2_resolve() to resolved }",
        "state v16 { to reject when (word[879472]<1) word[879376]=7 status=omgrfn5_r2_resolve() to resolved }")
    source = replace(source,
        "state bound { to done when (status!=0) to v3_emit when (word[879376]==3) status=omgrfn8_r2_emit_expected_legacy() to built }",
        "state bound { to done when (status!=0) status=omgrfn7_r2_emit_expected_v3() to built }")
    source = replace(source,
        "state checked { to done when (status!=0) to exact when (word[879088]==13) return 251 }",
        "state checked { to done when (status!=0) to exact when (word[879088]==16) return 251 }")
    source = replace(source, "proc omgrfn13_r2_check()", "proc omgrfn16_r2_check()")
    source = replace(source, "proc main() { return omgrfn13_r2_check() }",
                     "proc main() { return omgrfn16_r2_check() }")
    # Tokenization has already removed comments and strings. Count each selected
    # arithmetic spelling; R4 owns precedence and full expression joining.
    old = re.search(r"proc omgrfn13_r2_count_trapping_add\(\) \{.*?\n\}", source, re.S)
    if old is None:
        raise ValueError("OMGRFN16 arithmetic counter anchor")
    counter = """proc omgrfn13_r2_count_trapping_add() {
    let source=0 let token=0 let kind=0
    word[879472]=0
    state sources { to source_one when (source<word[500320]) return 0 }
    state source_one { token=0 to tokens }
    state tokens { to token_one when (token<word[880000+source*8]) source=source+1 to sources }
    state token_one { kind=omgrfn4_r2_tkind(source,token) to selected when(kind==43) to selected when(kind==45) to selected when(kind==42) token=token+1 to tokens }
    state selected { word[879472]=word[879472]+1 token=token+1 to tokens }
}"""
    source = source[:old.start()] + counter + source[old.end():]
    return source


def main() -> None:
    parser = argparse.ArgumentParser(); parser.add_argument("output", type=Path); args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    shared = load_shared()
    temp = args.output / ".v13"
    shared.materialize(temp, 13)
    r2 = patch_r2((temp / "r2.beta").read_text(encoding="ascii"))
    r1_shape = shared.write_checked(args.output / "r1.beta", R1)
    r2_shape = shared.write_checked(args.output / "r2.beta", r2)
    (args.output / "manifest.tsv").write_text(
        f"r1\t{r1_shape[0]}\t{r1_shape[1]}\n"
        f"r2\t{r2_shape[0]}\t{r2_shape[1]}\n", encoding="ascii")


if __name__ == "__main__": main()
