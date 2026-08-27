#!/usr/bin/env python3
"""Materialize the persisted-Beta OMGRFN16 responsibility 3--5 owners.

The materializer extends the responsibility-local, table-driven CKIR checker,
interpreter, and ELF reconstructor.  It deliberately contains no profile-byte
allowlist: every generated owner consumes and checks the supplied frame.
"""

from __future__ import annotations

import argparse
import importlib.util
import re
from pathlib import Path


HERE = Path(__file__).resolve().parent


def load_base():
    path = HERE / "omgrfn8-materialize-r3-r5.py"
    spec = importlib.util.spec_from_file_location("omgrfn16_r3_r5_base", path)
    if spec is None or spec.loader is None:
        raise ValueError("cannot load shared R3--R5 materializer")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = load_base()


def read(name: str) -> str:
    return (HERE / name).read_text(encoding="ascii")


def replace_exact(source: str, old: str, new: str, count: int = 1) -> str:
    actual = source.count(old)
    if actual != count:
        raise ValueError(f"anchor count {actual}, expected {count}: {old[:100]}")
    return source.replace(old, new)


def replace_proc(source: str, name: str, body: str) -> str:
    pattern = re.compile(rf"(?m)^proc\s+{re.escape(name)}\s*\([^)]*\)\s*\{{")
    match = pattern.search(source)
    if match is None:
        raise ValueError(f"missing procedure {name}")
    depth = 1
    cursor = match.end()
    while depth:
        depth += (source[cursor] == "{") - (source[cursor] == "}")
        cursor += 1
    return source[:match.start()] + body.rstrip() + "\n" + source[cursor:]


V16_ENVELOPE_READER = r"""proc omgrfn5_component_read() {
    let n=0 let overflow=0 let c=read_byte() let cursor=0 let flags=0
    let omg=0 let witness=0 let ckir=0 let elf=0 let result=0 let exit_code=0
    state loop { to one when(c>=0) to finish }
    state one { to over_byte when(n>=4497544) byte[1048576+n]=c n=n+1 c=read_byte() to loop }
    state over_byte { overflow=1 c=read_byte() to loop }
    state finish { word[524288]=n word[524296]=0 word[524376]=16 to exhausted when(overflow==1) to magic when(n>=40) to bad }
    state magic {
        to bad when(omgrfn5_component_byte(0)!='O') to bad when(omgrfn5_component_byte(1)!='M')
        to bad when(omgrfn5_component_byte(2)!='G') to bad when(omgrfn5_component_byte(3)!='R')
        to bad when(omgrfn5_component_byte(4)!='F') to bad when(omgrfn5_component_byte(5)!='N')
        to bad when(omgrfn5_component_byte(6)!='G') to bad when(omgrfn5_component_byte(7)!=0)
        to bad when(omgrfn5_component_u32(8)!=16) to fields
    }
    state fields {
        flags=omgrfn5_component_u32(12) omg=omgrfn5_component_u32(16)
        witness=omgrfn5_component_u32(20) ckir=omgrfn5_component_u32(24)
        elf=omgrfn5_component_u32(28) result=omgrfn5_component_u32(32) exit_code=omgrfn5_component_u32(36)
        to success when(flags==1) to trap when(flags==3) to bad
    }
    state success { to bad when(exit_code>255) to bad when(exit_code!=result%256) to extents }
    state trap { to bad when(result!=4294967295) to bad when(exit_code!=4294967295) to extents }
    state extents {
        to bad when(omg<1) to bad when(witness<1) to bad when(ckir<1) to bad when(elf<1)
        to exhausted when(omg>267280) to exhausted when(witness>524288)
        to exhausted when(ckir>2522192) to exhausted when(elf>1183744)
        cursor=40 word[524312]=cursor to bad when(omg>n-cursor) cursor=cursor+omg
        word[524320]=cursor to bad when(witness>n-cursor) cursor=cursor+witness word[524328]=cursor
        to bad when(ckir>n-cursor) cursor=cursor+ckir word[524344]=cursor
        to bad when(elf!=n-cursor)
        word[524304]=flags word[524336]=ckir word[524352]=elf
        word[524360]=result word[524368]=exit_code return 0
    }
    state exhausted { word[524296]=252 return 252 }
    state bad { word[524296]=251 return 251 }
}"""


def v16_envelope() -> str:
    source = read("omgrfn7-component-envelope-r5.beta")
    return replace_proc(source, "omgrfn5_component_read", V16_ENVELOPE_READER)


def patch_artifact(source: str) -> str:
    source = replace_exact(
        source,
        "to schema11 when (ckir_schema() == 11)\n        to bad",
        "to schema11 when (ckir_schema() == 11)\n        to schema14 when (ckir_schema() == 14)\n        to bad",
    )
    source = replace_exact(
        source,
        "state schema11 { to bad when (word[524376] != 13)  to schema_shared }",
        "state schema11 { to bad when (word[524376] != 13)  to schema_shared }\n"
        "    state schema14 { to bad when (word[524376] != 16) to schema_shared }",
    )
    source = replace_exact(source, "to bad when (kind > 6)", "to bad when (kind > 7)")
    source = replace_exact(
        source,
        "to array when (kind == 5)\n        to sum_nominal",
        "to array when (kind == 5)\n        to sum_nominal when (kind == 6)\n        to byte_view",
    )
    source = replace_exact(
        source,
        "state word_bounds { to bad when (high > 2147483647)  to intern_begin }",
        "state word_bounds { to intern_begin }",
    )
    view_type = """    state byte_view {
        to bad when(flags!=0) to bad when(payload0>=ckir_count(0))
        to bad when(ckir_type_byte(payload0,4)!=1) to bad when(ckir_type_byte(payload0,5)!=0)
        to bad when(ckir_type_word(payload0,8)!=0) to bad when(ckir_type_word(payload0,12)!=0)
        to bad when(ckir_type_word(payload0,16)!=0) to bad when(ckir_type_word(payload0,20)!=255)
        to bad when(payload1!=0) to bad when(low!=0) to bad when(high!=0) to intern_begin
    }
"""
    source = replace_exact(source, "    state sum_nominal {", view_type + "    state sum_nominal {")
    source = replace_exact(
        source,
        "to sum_begin\n    }\n    state byte_scalar",
        "to sum_begin when(kind==6)\n        size=16 alignment=8 to save\n    }\n    state byte_scalar",
    )
    source = replace_exact(
        source,
        "to sum when (kind == 6)\n        record =",
        "to sum when (kind == 6)\n        to yes_save when (kind == 7)\n        record =",
    )
    source = replace_exact(source, "to bad when (ckir_block_byte(i, 9) != 0)",
                           "to bad when (ckir_block_byte(i,9)>1)")
    source = replace_exact(source, "to bad when (opcode > 21)", "to bad when (opcode > 27)")
    source = replace_exact(
        source,
        "to arity1 when (opcode == 21)\n        to arity0",
        "to arity1 when (opcode == 21)\n        to arity0 when (opcode == 22)\n        to arity1 when (opcode == 23)\n        to arity1 when (opcode == 24)\n        to arity1 when (opcode == 25)\n        to arity0",
    )
    source = replace_exact(
        source,
        "to op_integer_widen when (opcode == 21)\n        to op_less",
        "to op_integer_widen when (opcode == 21)\n        to op_static_view when(opcode==22) to op_view when(opcode==23)\n"
        "        to op_view when(opcode==24) to op_view when(opcode==25)\n"
        "        to op_add when(opcode==26) to op_add when(opcode==27)\n        to op_less",
    )
    source = replace_exact(
        source,
        "to bad when (ckir_type_word(result_type,20)!=2147483647)",
        "to bad when (ckir_type_word(result_type,20)!=4294967295)",
    )
    source = replace_exact(source, "to trapping_add when (ckir_schema() == 11)",
                           "to trapping_add when (ckir_schema() == 11) to trapping_add when(ckir_schema()==14)")
    source = replace_exact(source, "to immediates_zero when (ckir_type_word(result_type,20) != 2147483647)",
                           "to immediates_zero when (ckir_type_word(result_type,20) != 4294967295)")
    source = replace_exact(source, "trapping_add_count=trapping_add_count+1",
                           "trapping_add_count=trapping_add_count+1", 1)
    view_ops = """    state op_static_view {
        to bad when(ckir_op_word(i,36)!=0) to bad when(ckir_op_word(i,32)>=ckir_count(7))
        to bad when(ckir_type_byte(result_type,4)!=7)
        to bad when(ckir_const_word(ckir_op_word(i,32),4)!=result_type)
        byte[13350000+ckir_op_word(i,32)]=1 to op_next
    }
    state op_view {
        to bad when(ckir_op_word(i,32)!=0) to bad when(ckir_op_word(i,36)!=0)
        to bad when(ckir_value_visible(a,owner,block,i)==0)
        source_type=ckir_value_type(a) to bad when(ckir_type_byte(source_type,4)!=7)
        to view_bool when(opcode==23) to view_head when(opcode==24)
        to bad when(result_type!=source_type) to op_next
    }
    state view_bool { to bad when(ckir_type_byte(result_type,4)!=3) to bad when(ckir_type_word(result_type,16)!=0) to bad when(ckir_type_word(result_type,20)!=1) to op_next }
    state view_head { to bad when(result_type!=ckir_type_word(source_type,8)) to op_next }
"""
    source = replace_exact(source, "    state op_store {", view_ops + "    state op_store {")
    source = replace_exact(
        source,
        "to trapping_add_required when (ckir_schema() == 11)\n        return 0",
        "to trapping_add_required when (ckir_schema() == 11)\n        to arithmetic_required when(ckir_schema()==14)\n        return 0",
    )
    source = replace_exact(
        source,
        "state trapping_add_required { to bad when (trapping_add_count != 4) return 0 }",
        "state trapping_add_required { to bad when (trapping_add_count != 4) return 0 }\n"
        "    state arithmetic_required { to bad when(trapping_add_count<1) return 0 }",
    )
    source = replace_exact(source, "state entry_flag { to bad when (word[524304]!=1) return 0 }",
                           "state entry_flag { to ok when(word[524304]==1) to ok when(word[524304]==3) to bad }\n    state ok { return 0 }")
    # Kind-7 constants are a bounded vector of exact-u8 scalar children.
    source = replace_exact(
        source,
        "to array when (kind == 5)\n        to bad\n    }\n    state scalar {",
        "to array when (kind == 5)\n        to byte_view when(kind==7)\n        to bad\n    }\n    state byte_view {\n"
        "        to bad when(word[13210000+i*8]!=0) to exhausted when(word[13140000+i*8]>32) to retain\n"
        "    }\n    state scalar {",
    )
    source = replace_exact(
        source,
        "to record_child when (kind==4) expected_type=ckir_type_word(type_id,8) to child_type",
        "to record_child when (kind==4) expected_type=ckir_type_word(type_id,8) to child_type",
    )
    # Generic child validation already uses payload0 for arrays and views.
    source = replace_exact(source, "state compound { to array when (kind==5)",
                           "state compound { to array when (kind==5) to view when(kind==7)")
    source = replace_exact(source, "state array { to done when (j>=word[13140000+node*8])",
                           "state view { to done when(j>=word[13140000+node*8]) child=word[13500000+(word[13070000+node*8]+j)*8] byte[13700000+base+j]=word[13210000+child*8]%256 j=j+1 to view }\n    state array { to done when (j>=word[13140000+node*8])")
    source = replace_exact(source, "size=word[6000000+word[13000000+i*8]*8]\n        to anchor when (size==0)",
                           "size=word[6000000+word[13000000+i*8]*8] to view_size when(ckir_type_byte(word[13000000+i*8],4)==7)\n        to anchor when(size==0)")
    source = replace_exact(source, "state anchor { size=1 to sized }",
                           "state view_size { size=word[13140000+i*8] to anchor when(size==0) to sized }\n    state anchor { size=1 to sized }")
    return source


R3_PAIR = r"""proc r316_wbyte(index) { return omgrfn5_component_byte(word[524320]+index) }
proc r316_wu32(at) { return r316_wbyte(at)+r316_wbyte(at+1)*256+r316_wbyte(at+2)*65536+r316_wbyte(at+3)*16777216 }
proc r316_pair() {
    let n=word[524328]-word[524320] let i=0 let types=0 let base=84 let selected=0 let at=0
    state size { to bad when(n<84) to magic }
    state magic {
        to bad when(r316_wbyte(0)!='O') to bad when(r316_wbyte(1)!='M')
        to bad when(r316_wbyte(2)!='G') to bad when(r316_wbyte(3)!='R')
        to bad when(r316_wbyte(4)!='S') to bad when(r316_wbyte(5)!='W')
        to bad when(r316_wbyte(6)!='7') to bad when(r316_wbyte(7)!=0)
        to bad when(r316_wu32(8)!=7) to bad when(r316_wbyte(12)!=0) to bad when(r316_wbyte(13)!=0)
        to bad when(r316_wbyte(14)!=84) to bad when(r316_wbyte(15)!=0)
        to bad when(r316_wu32(16)!=n) to bad when(r316_wu32(80)!=0) to extents
    }
    state extents {
        base=base+r316_wu32(20)*36 base=base+r316_wu32(24)*48
        base=base+r316_wu32(28)*28 base=base+r316_wu32(32)*28 types=r316_wu32(36)
        to bad when(types!=ckir_count(0)) to rows
    }
    state rows { to row when(i<types) to selected_exact }
    state row { at=base+i*24 to bad when(r316_wu32(at)!=i) to bytes }
    state bytes {
        to bad when(r316_wbyte(at+4)!=ckir_type_byte(i,4)) to bad when(r316_wbyte(at+5)!=ckir_type_byte(i,5))
        to bad when(r316_wu32(at+8)!=ckir_type_word(i,8)) to bad when(r316_wu32(at+12)!=ckir_type_word(i,12))
        to bad when(r316_wu32(at+16)!=ckir_type_word(i,16)) to bad when(r316_wu32(at+20)!=ckir_type_word(i,20))
        to full when(r316_wbyte(at+4)==2) i=i+1 to rows
    }
    state full { to next when(r316_wbyte(at+5)!=1) to next when(r316_wu32(at+8)!=0) to next when(r316_wu32(at+12)!=0) to next when(r316_wu32(at+16)!=0) to next when(r316_wu32(at+20)!=4294967295) selected=selected+1 to next }
    state next { i=i+1 to rows }
    state selected_exact { to bad when(selected!=1) return 0 }
    state bad { return 251 }
}
"""


R3_FRAME_READER = r"""proc l37_read_frame() {
    let n=0 let overflow=0 let c=read_byte() let cursor=0
    state read { to one when(c>=0) to header }
    state one { to over when(n>=4497544) byte[1048576+n]=c n=n+1 c=read_byte() to read }
    state over { overflow=1 c=read_byte() to read }
    state header { word[500000]=n word[500008]=0 word[500104]=16 to exhausted when(overflow==1) to bad when(n<40) to magic }
    state magic {
        to bad when(l3_frame_byte(0)!='O') to bad when(l3_frame_byte(1)!='M') to bad when(l3_frame_byte(2)!='G')
        to bad when(l3_frame_byte(3)!='R') to bad when(l3_frame_byte(4)!='F') to bad when(l3_frame_byte(5)!='N')
        to bad when(l3_frame_byte(6)!='G') to bad when(l3_frame_byte(7)!=0) to bad when(l3_frame_u32(8)!=16)
        word[500016]=l3_frame_u32(12) word[500032]=l3_frame_u32(16) word[500048]=l3_frame_u32(20)
        word[500064]=l3_frame_u32(24) word[500080]=l3_frame_u32(28)
        to accepted_flag when(word[500016]==1) to accepted_flag when(word[500016]==3) to bad
    }
    state accepted_flag {
        to bad when(word[500032]<1) to bad when(word[500048]<1) to bad when(word[500064]<1) to bad when(word[500080]<1)
        to exhausted when(word[500032]>267280) to exhausted when(word[500048]>524288)
        to exhausted when(word[500064]>2522192) to exhausted when(word[500080]>1183744)
        cursor=40 word[500024]=cursor to bad when(word[500032]>n-cursor) cursor=cursor+word[500032] word[500040]=cursor
        to bad when(word[500048]>n-cursor) cursor=cursor+word[500048] word[500056]=cursor
        to bad when(word[500064]>n-cursor) cursor=cursor+word[500064] word[500072]=cursor
        to bad when(word[500080]!=n-cursor) return 0
    }
    state exhausted { return l3_exhaust() }
    state bad { return l3_reject() }
}"""

R3_U32_WITNESS = r"""proc l3_w_u32(at) {
    return l3_witness_byte(at)+l3_witness_byte(at+1)*256
        +l3_witness_byte(at+2)*65536+l3_witness_byte(at+3)*16777216
}"""

R3_U32_CKIR = r"""proc l3_c_u32(at) {
    return l3_ckir_byte(at)+l3_ckir_byte(at+1)*256
        +l3_ckir_byte(at+2)*65536+l3_ckir_byte(at+3)*16777216
}"""

R3_ROOT = r"""proc l3_validate_root() {
    let selected=0-1 let extra=0
    state counts {
        to bad when(word[511008]!=word[510032]) to bad when(word[511016]!=word[510040])
        to bad when(word[511024]!=word[510048]) to bad when(word[511232]!=word[510184])
        to bad when(word[511240]!=word[510192]) to bad when(word[511248]!=word[510200])
        to bad when(word[511032]!=word[510056]) to bad when(word[511040]!=word[510064])
        to bad when(word[511048]<word[510072]) extra=word[511048]-word[510072]
        to bad when(extra>1) to bad when(word[511056]!=word[510080]+extra) to entry
    }
    state entry {
        selected=word[510088] to bad when(selected<0) to bad when(selected>=word[510056])
        to bad when(l3_ckir_byte(14)!=1) to bad when(word[511000]!=selected)
        to bad when(l3_machine(selected,5)!=0) to bad when(l3_machine(selected,3)<0)
        to bad when(l3_type(l3_machine(selected,3),0)>3)
        to bad when(l3_block(l3_machine(selected,8),6)!=0) return 0
    }
    state bad { return l3_reject() }
}"""

R3_MACHINE_JOIN = r"""proc l3_compare_machines_parameters() {
    let i=0 let p=0 let extra=word[511048]-word[510072]
    state machines { to machine when(i<word[510056]) i=0 to mparams }
    state machine {
        p=word[511128]+i*36
        to bad when(l3_c_u32(p)!=i) to bad when(l3_c_u32(p+4)!=l3_machine(i,1))
        to bad when(l3_ckir_byte(p+8)!=l3_machine(i,2)) to bad when(l3_ckir_byte(p+9)!=0)
        to bad when(l3_ckir_byte(p+10)!=0) to bad when(l3_ckir_byte(p+11)!=0)
        to bad when(l3_c_u32(p+12)!=l3_machine(i,3)) to bad when(l3_c_u32(p+16)!=l3_machine(i,4))
        to bad when(l3_c_u32(p+20)!=l3_machine(i,5)) to bad when(l3_c_u32(p+24)!=l3_machine(i,6))
        to selected_count when(i==word[510088]) to ordinary_count
    }
    state selected_count { to bad when(l3_c_u32(p+28)!=l3_machine(i,7)+extra) to entry }
    state ordinary_count { to bad when(l3_c_u32(p+28)!=l3_machine(i,7)) to entry }
    state entry { to bad when(l3_c_u32(p+32)!=l3_machine(i,8)) i=i+1 to machines }
    state mparams { to mparam when(i<word[510064]) return 0 }
    state mparam {
        p=word[511136]+i*20 to bad when(l3_c_u32(p)!=i)
        to bad when(l3_c_u32(p+4)!=l3_mparam(i,0)) to bad when(l3_c_u32(p+8)!=l3_mparam(i,1))
        to bad when(l3_c_u32(p+12)!=l3_mparam(i,2)) to bad when(l3_c_u32(p+16)!=i)
        i=i+1 to mparams
    }
    state bad { return l3_reject() }
}"""

R3_BLOCK_JOIN = r"""proc l3_compare_blocks_parameters() {
    let i=0 let p=0 let extra=word[511048]-word[510072] let selected=word[510088]
    state blocks { to block when(i<word[510072]) to synthetic }
    state block {
        p=word[511144]+i*32 to bad when(l3_c_u32(p)!=i) to bad when(l3_c_u32(p+4)!=l3_block(i,0))
        to bad when(l3_ckir_byte(p+8)!=l3_block(i,2)) to bad when(l3_ckir_byte(p+9)!=0)
        to bad when(l3_ckir_byte(p+10)!=0) to bad when(l3_ckir_byte(p+11)!=0)
        to bad when(l3_c_u32(p+12)!=l3_block(i,5)) to bad when(l3_c_u32(p+16)!=l3_block(i,6))
        i=i+1 to blocks
    }
    state synthetic { to no_synthetic when(extra==0) p=word[511144]+word[510072]*32
        to bad when(l3_c_u32(p)!=i) to bad when(l3_c_u32(p+4)!=selected)
        to bad when(l3_ckir_byte(p+8)!=l3_machine(selected,2)) to bad when(l3_ckir_byte(p+9)!=1)
        to bad when(l3_ckir_byte(p+10)!=0) to bad when(l3_ckir_byte(p+11)!=0)
        to bad when(l3_c_u32(p+12)!=word[510080]) to bad when(l3_c_u32(p+16)!=1) i=0 to bparams
    }
    state no_synthetic { i=0 to bparams }
    state bparams { to bparam when(i<word[510080]) to synthetic_param }
    state bparam {
        p=word[511152]+i*20 to bad when(l3_c_u32(p)!=i) to bad when(l3_c_u32(p+4)!=l3_bparam(i,0))
        to bad when(l3_c_u32(p+8)!=l3_bparam(i,1)) to bad when(l3_c_u32(p+12)!=l3_bparam(i,2))
        to bad when(l3_c_u32(p+16)!=word[510064]+i) i=i+1 to bparams
    }
    state synthetic_param { to done when(extra==0) p=word[511152]+i*20
        to bad when(l3_c_u32(p)!=i) to bad when(l3_c_u32(p+4)!=word[510072])
        to bad when(l3_c_u32(p+8)!=0) to bad when(l3_c_u32(p+12)>=word[511008])
        to bad when(l3_type(l3_c_u32(p+12),0)!=7) to bad when(l3_c_u32(p+16)!=word[510064]+i) to done
    }
    state done { return 0 }
    state bad { return l3_reject() }
}"""

R3_CKIR_STRUCTURE = r"""proc r316_op_word(id,off) { return l3_c_u32(word[511160]+id*40+off) }
proc r316_term_word(id,off) { return l3_c_u32(word[511176]+id*52+off) }
proc r316_operand(id) { return l3_c_u32(word[511168]+id*4) }

proc r316_value_visible(id,owner,block,op) {
    state bound { to no when(id>=word[12140000]) to no when(word[12150000+id*8]!=owner)
        to yes when(word[12160000+id*8]==4294967295) to no when(word[12160000+id*8]!=block)
        to yes when(word[12170000+id*8]==4294967295) to yes when(word[12170000+id*8]<op) to no }
    state yes { return 1 } state no { return 0 }
}
proc r316_place_visible(id,block,op) {
    state bound { to no when(id>=word[12140008]) to no when(word[12200000+id*8]!=block)
        to yes when(word[12210000+id*8]<op) to no }
    state yes { return 1 } state no { return 0 }
}
proc r316_expected_arity(opcode,imm) {
    state select { to zero when(opcode==1) to zero when(opcode==2) to one when(opcode==3)
        to two when(opcode==4) to one when(opcode==5) to two when(opcode==6) to two when(opcode==7)
        to two when(opcode==8) to two when(opcode==9) to call when(opcode==10) to one when(opcode==11)
        to two when(opcode==12) to variable when(opcode==13) to variable when(opcode==14)
        to one when(opcode==15) to two when(opcode==16) to two when(opcode==17) to two when(opcode==18)
        to two when(opcode==19) to two when(opcode==20) to one when(opcode==21) to zero when(opcode==22)
        to one when(opcode==23) to one when(opcode==24) to one when(opcode==25)
        to two when(opcode==26) to two when(opcode==27) to invalid }
    state zero { return 0 } state one { return 1 } state two { return 2 }
    state call { to invalid when(imm>=word[511032]) return 1+l3_c_u32(word[511128]+imm*36+20) }
    state variable { return 4294967295 } state invalid { return 4294967294 }
}
proc r316_expected_kind(opcode,imm) {
    state select { to none when(opcode==6) to none when(opcode==7) to none when(opcode==11)
        to place when(opcode==2) to place when(opcode==3) to place when(opcode==4)
        to call when(opcode==10) to value }
    state none { return 0 } state place { return 2 } state value { return 1 }
    state call { to none when(l3_c_u32(word[511128]+imm*36+12)==0-1) to value }
}
proc r316_edge(owner,block,op,target,start,count) {
    let i=0 let value=0 let p=0 let actual=0 let wanted=0
    state absent { to no_target when(target==4294967295) to target_row }
    state no_target { to bad when(count!=0) return 0 }
    state target_row { to bad when(target>=word[511048]) p=word[511144]+target*32
        to bad when(l3_c_u32(p+4)!=owner) to bad when(target==l3_c_u32(word[511128]+owner*36+32))
        to bad when(count!=l3_c_u32(p+16)) i=0 to args }
    state args { to one when(i<count) return 0 }
    state one { value=r316_operand(start+i) to bad when(r316_value_visible(value,owner,block,op)==0)
        actual=word[12180000+value*8] wanted=l3_c_u32(word[511152]+(l3_c_u32(p+12)+i)*20+12)
        to bad when(actual!=wanted) i=i+1 to args }
    state bad { return l3_reject() }
}
proc r316_validate_ckir_structure() {
    let i=0 let p=0 let owner=0 let block=0 let opcode=0 let kind=0 let result=0 let type_id=0
    let start=0 let count=0 let expected=0 let imm0=0 let j=0 let value=0 let next_operand=0
    let next_param=0 let next_op=0 let term_kind=0 let target0=0 let target1=0 let start1=0 let count1=0
    state machine_params { to mp when(i<word[511040]) i=0 to block_params }
    state mp { p=word[511136]+i*20 to bad when(l3_c_u32(p+16)!=i)
        word[12150000+i*8]=l3_c_u32(p+4) word[12160000+i*8]=4294967295 word[12170000+i*8]=4294967295
        word[12180000+i*8]=l3_c_u32(p+12) i=i+1 to machine_params }
    state block_params { to bp when(i<word[511056]) word[12140000]=word[511040]+word[511056] i=0 to blocks }
    state bp { p=word[511152]+i*20 value=word[511040]+i to bad when(l3_c_u32(p+16)!=value)
        block=l3_c_u32(p+4) to bad when(block>=word[511048]) word[12150000+value*8]=l3_c_u32(word[511144]+block*32+4)
        word[12160000+value*8]=block word[12170000+value*8]=4294967295 word[12180000+value*8]=l3_c_u32(p+12)
        i=i+1 to block_params }
    state blocks { to block_one when(i<word[511048]) to block_partition }
    state block_one { p=word[511144]+i*32 to bad when(l3_c_u32(p)!=i)
        to bad when(l3_c_u32(p+12)!=next_param) to bad when(l3_c_u32(p+16)>word[511056]-next_param)
        to bad when(l3_c_u32(p+20)!=next_op) to bad when(l3_c_u32(p+24)>word[511064]-next_op)
        to bad when(l3_c_u32(p+28)!=i) next_param=next_param+l3_c_u32(p+16) next_op=next_op+l3_c_u32(p+24)
        i=i+1 to blocks }
    state block_partition { to bad when(next_param!=word[511056]) to bad when(next_op!=word[511064]) i=0 to operations }
    state operations { to operation when(i<word[511064]) to operation_finish }
    state operation { p=word[511160]+i*40 to bad when(l3_c_u32(p)!=i) owner=l3_c_u32(p+4) block=l3_c_u32(p+8)
        to bad when(block>=word[511048]) to bad when(owner!=l3_c_u32(word[511144]+block*32+4))
        opcode=l3_ckir_byte(p+12) to bad when(opcode<1) to bad when(opcode>27)
        kind=l3_ckir_byte(p+13) to bad when(l3_ckir_byte(p+14)!=0) to bad when(l3_ckir_byte(p+15)!=0)
        result=l3_c_u32(p+16) type_id=l3_c_u32(p+20) start=l3_c_u32(p+24) count=l3_c_u32(p+28) imm0=l316_c_full(p+32)
        to bad when(start!=next_operand) to bad when(count>word[511072]-start)
        expected=r316_expected_arity(opcode,imm0) to bad when(expected==4294967294)
        to arity_ok when(expected==4294967295) to bad when(count!=expected) to arity_ok }
    state arity_ok { expected=r316_expected_kind(opcode,imm0) to bad when(kind!=expected)
        to no_result when(kind==0) to value_result when(kind==1) to place_result }
    state no_result { to bad when(result!=0-1) to bad when(type_id!=0-1) to operands }
    state value_result { to bad when(result!=word[12140000]) to bad when(type_id>=word[511008])
        word[12150000+result*8]=owner word[12160000+result*8]=block word[12170000+result*8]=i word[12180000+result*8]=type_id
        word[12140000]=result+1 to operands }
    state place_result { to bad when(result!=word[12140008]) to bad when(type_id>=word[511008])
        word[12190000+result*8]=type_id word[12200000+result*8]=block word[12210000+result*8]=i word[12140008]=result+1 to operands }
    state operands { j=0 to operand_loop }
    state operand_loop { to operand_one when(j<count) to semantic }
    state operand_one { value=r316_operand(start+j) to place_operand when(opcode==3) to place_probe when(opcode==4)
        to place_operand when(opcode==5) to place_probe when(opcode==6) to place_probe when(opcode==7)
        to place_probe when(opcode==10) to value_operand }
    state place_probe { to place_operand when(j==0) to copy_probe when(opcode==7) to value_operand }
    state copy_probe { to place_operand when(imm0==2) to value_operand }
    state value_operand { to bad when(r316_value_visible(value,owner,block,i)==0) j=j+1 to operand_loop }
    state place_operand { to bad when(r316_place_visible(value,block,i)==0) j=j+1 to operand_loop }
    state semantic { to literal when(opcode==1) to view when(opcode==22) to view when(opcode==23) to view when(opcode==24) to view when(opcode==25)
        to arithmetic when(opcode==8) to arithmetic when(opcode==26) to arithmetic when(opcode==27) to operation_next }
    state literal { to bad when(l3_type(type_id,0)>3) to bad when(imm0<l3_type(type_id,4)) to bad when(imm0>l3_type(type_id,5)) to operation_next }
    state arithmetic { to bad when(type_id>=word[511008]) value=r316_operand(start)
        to bad when(word[12180000+value*8]!=type_id) value=r316_operand(start+1)
        to bad when(word[12180000+value*8]!=type_id) to operation_next }
    state view { to static_view when(opcode==22) value=r316_operand(start) to bad when(l3_type(word[12180000+value*8],0)!=7)
        to view_bool when(opcode==23) to view_head when(opcode==24) to bad when(l3_type(type_id,0)!=7) to operation_next }
    state static_view { to bad when(type_id>=word[511008]) to bad when(l3_type(type_id,0)!=7)
        to bad when(imm0>=word[511200]) to bad when(l3_constant(imm0,1)!=type_id) to operation_next }
    state view_bool { to bad when(l3_type(type_id,0)!=3) to operation_next }
    state view_head { value=r316_operand(start) to bad when(type_id!=l3_type(word[12180000+value*8],2)) to operation_next }
    state operation_next { next_operand=next_operand+count i=i+1 to operations }
    state operation_finish { to bad when(word[12140000]!=word[511088]) to bad when(word[12140008]!=word[511096]) i=0 to terminators }
    state terminators { to term when(i<word[511080]) to finish }
    state term { p=word[511176]+i*52 to bad when(l3_c_u32(p)!=i) owner=l3_c_u32(p+4) block=l3_c_u32(p+8)
        to bad when(block!=i) to bad when(block>=word[511048]) to bad when(owner!=l3_c_u32(word[511144]+block*32+4))
        term_kind=l3_ckir_byte(p+12) to bad when(term_kind<1) to bad when(term_kind>5) to bad when(l3_ckir_byte(p+14)!=0) to bad when(l3_ckir_byte(p+15)!=0)
        start=l3_c_u32(p+24) count=l3_c_u32(p+28) target0=l3_c_u32(p+20)
        start1=l3_c_u32(p+36) count1=l3_c_u32(p+40) target1=l3_c_u32(p+32)
        to case_term when(term_kind==5) to bad when(l3_ckir_byte(p+13)!=0)
        to bad when(start!=next_operand) to bad when(count>word[511072]-start) next_operand=next_operand+count
        to bad when(start1!=next_operand) to bad when(count1>word[511072]-start1) next_operand=next_operand+count1
        expected=l3_c_u32(word[511144]+block*32+20)+l3_c_u32(word[511144]+block*32+24)
        to bad when(r316_edge(owner,block,expected,target0,start,count)!=0) to bad when(r316_edge(owner,block,expected,target1,start1,count1)!=0)
        value=l3_c_u32(p+16) to jump when(term_kind==1) to branch when(term_kind==2) to unit_return when(term_kind==3) to value_return }
    state jump { to bad when(value!=4294967295) to bad when(target0==4294967295) to bad when(target1!=4294967295) i=i+1 to terminators }
    state branch { to bad when(r316_value_visible(value,owner,block,expected)==0) to bad when(l3_type(word[12180000+value*8],0)!=3)
        to bad when(target0==4294967295) to bad when(target1==4294967295) i=i+1 to terminators }
    state unit_return { to bad when(value!=4294967295) to bad when(target0!=4294967295) to bad when(target1!=4294967295) i=i+1 to terminators }
    state value_return { to bad when(target0!=4294967295) to bad when(target1!=4294967295)
        to bad when(r316_value_visible(value,owner,block,expected)==0) i=i+1 to terminators }
    state case_term { to bad when(start!=next_operand) to bad when(start1!=next_operand) to bad when(count!=0) to bad when(count1!=0) i=i+1 to terminators }
    state finish { to bad when(i!=word[511048]) to bad when(next_operand!=word[511072]) return 0 }
    state bad { return l3_reject() }
}"""


def split_r3_structure() -> str:
    """Keep the independent structural walk below Beta's per-procedure budget."""
    marker = "proc r316_validate_ckir_structure() {\n"
    helpers, body = R3_CKIR_STRUCTURE.split(marker, 1)
    declarations, states = body.split("    state machine_params", 1)
    prepare, rest = ("    state machine_params" + states).split("    state operations", 1)
    operations, terms = ("    state operations" + rest).split("    state terminators", 1)
    terminators = "    state terminators" + terms
    bad = "    state bad { return l3_reject() }\n}"
    if not terminators.endswith(bad):
        raise ValueError("R3 structural split anchor")
    terminators = terminators[:-len(bad)]
    prepare = replace_exact(prepare, "i=0 to operations }", "return 0 }")
    operations = replace_exact(
        operations,
        "i=0 to terminators }",
        "word[12140016]=next_operand return 0 }",
    )
    pieces = [
        helpers,
        "proc r316_prepare_ckir_structure() {\n" + declarations + prepare + bad + "\n",
        "proc r316_validate_operations() {\n" + declarations + operations + bad + "\n",
        r"""proc r316_target_shape(owner,target,count) {
    let p=0
    state select { to absent when(target==0-1) to bad when(target>=word[511048]) p=word[511144]+target*32
        to bad when(l3_c_u32(p+4)!=owner) to bad when(l3_c_u32(p+16)!=count) return 0 }
    state absent { to bad when(count!=0) return 0 }
    state bad { return l3_reject() }
}
proc r316_validate_terminators() {
    let i=0 let p=0 let owner=0 let block=0 let kind=0 let flags=0 let value=0
    let target0=0 let start0=0 let count0=0 let target1=0 let start1=0 let count1=0
    let next=word[12140016] let end=0
    state loop { to one when(i<word[511080]) to finish }
    state one { p=word[511176]+i*52 to bad when(l3_c_u32(p)!=i) owner=l3_c_u32(p+4) block=l3_c_u32(p+8)
        to bad when(block!=i) to bad when(block>=word[511048]) to bad when(owner!=l3_c_u32(word[511144]+block*32+4))
        kind=l3_ckir_byte(p+12) flags=l3_ckir_byte(p+13) to bad when(kind<1) to bad when(kind>5)
        to bad when(l3_ckir_byte(p+14)!=0) to bad when(l3_ckir_byte(p+15)!=0)
        start0=l3_c_u32(p+24) count0=l3_c_u32(p+28) start1=l3_c_u32(p+36) count1=l3_c_u32(p+40)
        to cases when(kind==5) to bad when(flags!=0) to bad when(start0!=next)
        to bad when(count0>word[511072]-start0) next=next+count0 to bad when(start1!=next)
        to bad when(count1>word[511072]-start1) next=next+count1
        target0=l3_c_u32(p+20) target1=l3_c_u32(p+32)
        to bad when(r316_target_shape(owner,target0,count0)!=0) to bad when(r316_target_shape(owner,target1,count1)!=0)
        value=l3_c_u32(p+16) end=l3_c_u32(word[511144]+block*32+20)+l3_c_u32(word[511144]+block*32+24)
        to jump when(kind==1) to branch when(kind==2) to unit_return when(kind==3) to value_return }
    state jump { to bad when(value!=0-1) to bad when(target0==0-1) to bad when(target1!=0-1) i=i+1 to loop }
    state branch { to bad when(r316_value_visible(value,owner,block,end)==0) to bad when(l3_type(word[12180000+value*8],0)!=3)
        to bad when(target0==0-1) to bad when(target1==0-1) i=i+1 to loop }
    state unit_return { to bad when(value!=0-1) to bad when(target0!=0-1) to bad when(target1!=0-1) i=i+1 to loop }
    state value_return { to bad when(target0!=0-1) to bad when(target1!=0-1)
        to bad when(r316_value_visible(value,owner,block,end)==0) i=i+1 to loop }
    state cases { to bad when(start0!=next) to bad when(start1!=next) to bad when(count0!=0) to bad when(count1!=0) i=i+1 to loop }
    state finish { to bad when(i!=word[511048]) to bad when(next!=word[511072]) return 0 }
    state bad { return l3_reject() }
}
""",
        r"""proc r316_validate_ckir_structure() {
    let s=r316_prepare_ckir_structure()
    state prepare { to done when(s!=0) s=r316_validate_operations() to operations }
    state operations { to done when(s!=0) s=r316_validate_terminators() to done }
    state done { return s }
}
""",
    ]
    return "".join(pieces)


def patch_r3_independent(source: str) -> str:
    """Advance the independent OMGRSW/CKIR table-join lineage to V16.

    This deliberately does not import the artifact checker's validation
    conclusion.  The only commonality with R5 is the frozen wire format.
    """
    source = replace_proc(source, "l37_read_frame", R3_FRAME_READER)
    source = replace_proc(source, "l3_validate_root", R3_ROOT)
    source = replace_proc(source, "l3_compare_machines_parameters", R3_MACHINE_JOIN)
    source = replace_proc(source, "l3_compare_blocks_parameters", R3_BLOCK_JOIN)
    source += "\nproc l316_w_full(at) { return l3_witness_byte(at)+l3_witness_byte(at+1)*256+l3_witness_byte(at+2)*65536+l3_witness_byte(at+3)*16777216 }\n"
    source += "proc l316_c_full(at) { return l3_ckir_byte(at)+l3_ckir_byte(at+1)*256+l3_ckir_byte(at+2)*65536+l3_ckir_byte(at+3)*16777216 }\n"
    source = replace_exact(source, "l3_witness_byte(6)!='3'", "l3_witness_byte(6)!='7'")
    source = replace_exact(source, "l3_witness_byte(8)!=3", "l3_witness_byte(8)!=7")
    source = replace_exact(source, "to bad when (word[510184]<1) ", "")
    source = replace_exact(source,
        "l3_type_set(i,4,l3_w_u32(p+16)) l3_type_set(i,5,l3_w_u32(p+20))",
        "l3_type_set(i,4,l316_w_full(p+16)) l3_type_set(i,5,l316_w_full(p+20))")
    source = replace_exact(source, "to bad when (l3_c_u32(p+16) != l3_type(i,4))",
                           "to bad when(l316_c_full(p+16)!=l3_type(i,4))")
    source = replace_exact(source, "to bad when (l3_c_u32(p+20) != l3_type(i,5))",
                           "to bad when(l316_c_full(p+20)!=l3_type(i,5))")
    source = replace_exact(source, "to sum when (l3_type(i,0) == 6)\n        to bad",
                           "to sum when(l3_type(i,0)==6) to byte_view when(l3_type(i,0)==7) to bad")
    source = replace_exact(source, "    state scalar {\n", """    state byte_view {
        to bad when(l3_type(i,1)!=0) to bad when(l3_type(i,2)>=i)
        to bad when(l3_type(l3_type(i,2),0)!=1) to bad when(l3_type(l3_type(i,2),1)!=0)
        to bad when(l3_type(l3_type(i,2),4)!=0) to bad when(l3_type(l3_type(i,2),5)!=255)
        to bad when(l3_type(i,3)!=0) to bad when(l3_type(i,4)!=0) to bad when(l3_type(i,5)!=0) to unique
    }
    state scalar {
""", 1)
    source = source.replace("l3_type(i,5) != 2147483647", "l3_type(i,5) != 4294967295")
    source = source.replace("l3_type(type_id,5)!=2147483647", "l3_type(type_id,5)!=4294967295")
    source = replace_exact(source,
        "state array { to scalar when (l3_type(type_id,0) != 5)  l3_mark_nonarray_tree(l3_type(type_id,2))  return word[500008] }",
        "state array { to view when(l3_type(type_id,0)==7) to scalar when(l3_type(type_id,0)!=5) l3_mark_nonarray_tree(l3_type(type_id,2)) return word[500008] }\n"
        "    state view { l3_mark_nonarray_tree(l3_type(type_id,2)) to done when(word[500008]!=0) to scalar }")
    source = replace_exact(source,
        "state kind4 { to nominal when (l3_type(type_id,0) == 4) to array when (l3_type(type_id,0)==5) to sum }",
        "state kind4 { to nominal when(l3_type(type_id,0)==4) to array when(l3_type(type_id,0)==5) to sum when(l3_type(type_id,0)==6) to view }")
    source = replace_exact(source,
        "    state byte_scalar { word[500224]=1  word[500232]=1  return 0 }",
        "    state view { word[500224]=16 word[500232]=8 return 0 }\n"
        "    state byte_scalar { word[500224]=1  word[500232]=1  return 0 }")
    source = replace_exact(source, "to array when (kind == 5)\n        to bad",
                           "to array when(kind==5) to view when(kind==7) to bad")
    source = replace_exact(source, "    state array {\n        to bad when (scalar != 0)",
                           """    state view {
        to bad when(scalar!=0) expected=count i=0 to array_children
    }
    state array {
        to bad when (scalar != 0)""")
    source = replace_exact(source,
        "to bad when (l3_constant(child,1) != l3_type(type_id,2))",
        "to bad when(l3_constant(child,1)!=l3_type(type_id,2))")
    source = replace_exact(source,
        "proc l3_constant(id, column) { return l3_c_u32(word[511216] + id*24 + column*4) }",
        "proc l3_constant(id,column) { state select { to full when(column==4) return l3_c_u32(word[511216]+id*24+column*4) } state full { return l316_c_full(word[511216]+id*24+16) } }")
    source = replace_exact(source, "state complete { to bad when (count!=3) return 0 }",
                           "state complete { return 0 }")
    source = replace_exact(source,
        "state operation { p=word[511160]+i*40 to add_probe when (l3_ckir_byte(p+12)==8) to next }",
        "state operation { p=word[511160]+i*40 to add_probe when(l3_ckir_byte(p+12)==8) to add_probe when(l3_ckir_byte(p+12)==26) to add_probe when(l3_ckir_byte(p+12)==27) to next }")
    source = replace_exact(source, "state complete { to bad when (count!=4) return 0 }",
                           "state complete { to bad when(count<1) return 0 }")
    source += "\n" + split_r3_structure()
    source = replace_proc(source, "main", "proc main() { return omgrfn16_r3_check() }")
    source += r"""
proc omgrfn16_r3_check() {
    l37_read_frame()
    state frame { to done when(word[500008]!=0) l37_witness_header() to witness }
    state witness { to done when(word[500008]!=0) l37_ckir_header() to ckir }
    state ckir { to done when(word[500008]!=0) l3_load_declarations() to declarations }
    state declarations { to done when(word[500008]!=0) l3_load_types() to types }
    state types { to done when(word[500008]!=0) l3_load_records_fields() to records }
    state records { to done when(word[500008]!=0) l37_load_sums() to sums }
    state sums { to done when(word[500008]!=0) l3_load_machines() to machines }
    state machines { to done when(word[500008]!=0) l3_load_parameters_blocks() to parameters }
    state parameters { to done when(word[500008]!=0) l3_validate_layouts() to layouts }
    state layouts { to done when(word[500008]!=0) l3_validate_copyability() to copies }
    state copies { to done when(word[500008]!=0) l3_validate_constructor_nominals() to constructors }
    state constructors { to done when(word[500008]!=0) r316_validate_ckir_structure() to structure }
    state structure { to done when(word[500008]!=0) l37_validate_case_intrinsics() to cases }
    state cases { to done when(word[500008]!=0) l38_validate_logical_intrinsics() to unary }
    state unary { to done when(word[500008]!=0) l39_validate_logical_binary_intrinsics() to binary }
    state binary { to done when(word[500008]!=0) l310_validate_scalar_equal_intrinsics() to equality }
    state equality { to done when(word[500008]!=0) l311_validate_ordered_intrinsics() to ordered }
    state ordered { to done when(word[500008]!=0) l312_validate_integer_widen_intrinsics() to widen }
    state widen { to done when(word[500008]!=0) l313_validate_trapping_add_intrinsics() to arithmetic }
    state arithmetic { to done when(word[500008]!=0) l3_validate_root() to root }
    state root { to done when(word[500008]!=0) l3_compare_types() to types_join }
    state types_join { to done when(word[500008]!=0) l3_compare_records_fields() to records_join }
    state records_join { to done when(word[500008]!=0) l37_compare_sums() to sums_join }
    state sums_join { to done when(word[500008]!=0) l3_compare_machines_parameters() to machines_join }
    state machines_join { to done when(word[500008]!=0) l3_compare_blocks_parameters() to blocks_join }
    state blocks_join { to done when(word[500008]!=0) l3_validate_constants() to done }
    state done { return word[500008] }
}
"""
    return source


def patch_result(source: str) -> str:
    source = replace_exact(source, "to constructor when (ckir_op_byte(operation, 12) == 14)\n        to operation_next",
                           "to constructor when (ckir_op_byte(operation,12)==14)\n        to constructor when(ckir_op_byte(operation,12)==22)\n        to constructor when(ckir_op_byte(operation,12)==25)\n        to operation_next")
    source = replace_exact(source, "to integer_widen when (opcode == 21)\n        to less",
                           "to integer_widen when(opcode==21) to static_view when(opcode==22)\n        to view_nonempty when(opcode==23) to view_head when(opcode==24) to view_tail when(opcode==25)\n        to subtract when(opcode==26) to multiply when(opcode==27)\n        to less")
    old_add = """    state add {
        a = ckir_operand(start)
        b = ckir_operand(start + 1)
        value = word[8800000 + a * 8] + word[8800000 + b * 8]
        to byte_overflow when (ckir_type_byte(result_type, 4) == 1)
        to word_overflow
    }
    state byte_overflow { to trap when (value > 255)  to add_range }
    state word_overflow { to trap when (value > 4294967295)  to add_range }
    state add_range {
        to trap when (value < ckir_type_word(result_type, 16))
        to trap when (value > ckir_type_word(result_type, 20))
        word[8800000 + result * 8] = value
        to op_next
    }"""
    new_add = """    state add {
        a=ckir_operand(start) b=ckir_operand(start+1)
        to trap when(word[8800000+a*8]>4294967295-word[8800000+b*8])
        value=word[8800000+a*8]+word[8800000+b*8] to arithmetic_range
    }
    state subtract {
        a=ckir_operand(start) b=ckir_operand(start+1)
        to trap when(word[8800000+a*8]<word[8800000+b*8])
        value=word[8800000+a*8]-word[8800000+b*8] to arithmetic_range
    }
    state multiply {
        a=ckir_operand(start) b=ckir_operand(start+1)
        to multiply_zero when(word[8800000+b*8]==0)
        to trap when(word[8800000+a*8]>4294967295/word[8800000+b*8])
        value=word[8800000+a*8]*word[8800000+b*8] to arithmetic_range
    }
    state multiply_zero { value=0 to arithmetic_range }
    state arithmetic_range {
        to trap when(value<ckir_type_word(result_type,16)) to trap when(value>ckir_type_word(result_type,20))
        word[8800000+result*8]=value to op_next
    }"""
    source = replace_exact(source, old_add, new_add)
    views = """    state static_view {
        address=word[13900000+result*8] value=4300000+word[13400000+ckir_op_word(op,32)*8]
        base_type=ckir_op_word(op,32) count=ckir_const_word(base_type,12)
        byte[9400000+address]=value%256 byte[9400000+address+1]=(value/256)%256
        byte[9400000+address+2]=(value/65536)%256 byte[9400000+address+3]=(value/16777216)%256
        byte[9400000+address+8]=count%256 byte[9400000+address+9]=(count/256)%256
        byte[9400000+address+10]=(count/65536)%256 byte[9400000+address+11]=(count/16777216)%256
        word[8800000+result*8]=address to op_next
    }
    state view_nonempty {
        a=ckir_operand(start) address=word[8800000+a*8]
        count=byte[9400000+address+8]+byte[9400000+address+9]*256+byte[9400000+address+10]*65536+byte[9400000+address+11]*16777216
        word[8800000+result*8]=0 to view_nonempty_yes when(count>0) to op_next
    }
    state view_nonempty_yes { word[8800000+result*8]=1 to op_next }
    state view_head {
        a=ckir_operand(start) address=word[8800000+a*8]
        count=byte[9400000+address+8]+byte[9400000+address+9]*256+byte[9400000+address+10]*65536+byte[9400000+address+11]*16777216
        to trap when(count==0)
        value=byte[9400000+address]+byte[9400000+address+1]*256+byte[9400000+address+2]*65536+byte[9400000+address+3]*16777216
        word[8800000+result*8]=byte[9400000+value] to op_next
    }
    state view_tail {
        a=ckir_operand(start) base_type=word[8800000+a*8] address=word[13900000+result*8]
        count=byte[9400000+base_type+8]+byte[9400000+base_type+9]*256+byte[9400000+base_type+10]*65536+byte[9400000+base_type+11]*16777216
        to trap when(count==0)
        value=byte[9400000+base_type]+byte[9400000+base_type+1]*256+byte[9400000+base_type+2]*65536+byte[9400000+base_type+3]*16777216
        value=value+1 count=count-1
        byte[9400000+address]=value%256 byte[9400000+address+1]=(value/256)%256
        byte[9400000+address+2]=(value/65536)%256 byte[9400000+address+3]=(value/16777216)%256
        byte[9400000+address+8]=count%256 byte[9400000+address+9]=(count/256)%256
        byte[9400000+address+10]=(count/65536)%256 byte[9400000+address+11]=(count/16777216)%256
        word[8800000+result*8]=address to op_next
    }
"""
    source = replace_exact(source, "    state call {", views + "    state call {")
    old = "    state trap { return 251 }"
    at = source.rfind(old)
    if at < 0:
        raise ValueError("result trap anchor")
    source = source[:at] + "    state trap { to accepted_trap when(word[524304]==3) return 251 }\n    state accepted_trap { return 0 }" + source[at + len(old):]
    return source


def patch_elf(source: str) -> str:
    # Operation operands are the prefix of the CKIR operand table; terminator
    # edge arguments consume the remainder.  The structure owner validates the
    # exact partition independently.
    source = replace_exact(source, "to bad when (next_operand != ckir_count(10))",
                           "to bad when(next_operand>ckir_count(10))")
    source = replace_exact(source, "to bad when (opcode > 21)", "to bad when(opcode>27)")
    source = replace_exact(source, "state operation_ready { to trapping_add when (opcode == 8)",
                           "state operation_ready { to trapping_add when(opcode==8) to trapping_add when(opcode==26) to trapping_add when(opcode==27)")
    source = replace_exact(source, "to integer_widen when (opcode == 21) to next",
                           "to integer_widen when(opcode==21) to next")
    source = replace_exact(source, "state trapping_add { to next when (ckir_schema()!=11)",
                           "state trapping_add { to next when(ckir_schema()!=14)")
    source = replace_exact(source, "to trapping_add_required when (ckir_schema() == 11)\n        return 0",
                           "to trapping_add_required when(ckir_schema()==14)\n        return 0")
    source = replace_exact(source, "state trapping_add_required { to bad when (trapping_add_count != 4) return 0 }",
                           "state trapping_add_required { to bad when(trapping_add_count<1) return 0 }")
    source = replace_exact(source, "state object_one { to object_kind when (ckir_op_byte(i,12) == 13)  to object_kind when (ckir_op_byte(i,12) == 14)  to object_next }",
                           "state object_one { to object_kind when(ckir_op_byte(i,12)==13) to object_kind when(ckir_op_byte(i,12)==14) to object_kind when(ckir_op_byte(i,12)==22) to object_kind when(ckir_op_byte(i,12)==25) to object_next }")
    source = replace_exact(source, "to integer_widen when (opcode == 21)\n        to less_equal",
                           "to integer_widen when(opcode==21) to static_view when(opcode==22)\n        to view_nonempty when(opcode==23) to view_head when(opcode==24) to view_tail when(opcode==25)\n        to subtract when(opcode==26) to multiply when(opcode==27)\n        to less_equal")
    arithmetic = """    state subtract {
        a=ckir_operand(start) b=ckir_operand(start+1) status=elf_load_value(a)
        to done when(status!=0) status=elf_code2(43,133) to done when(status!=0)
        status=elf_code_s32(0-elf_value_slot(b)) to done when(status!=0)
        status=elf_trap_jump(130) to arithmetic_finish
    }
    state multiply {
        a=ckir_operand(start) b=ckir_operand(start+1) status=elf_load_value(a)
        to done when(status!=0) status=elf_code2(247,165) to done when(status!=0)
        status=elf_code_s32(0-elf_value_slot(b)) to done when(status!=0)
        status=elf_code2(133,210) to done when(status!=0) status=elf_trap_jump(133) to arithmetic_finish
    }
    state arithmetic_finish { to done when(status!=0) status=elf_range_check(result_type) to done when(status!=0) status=elf_store_value(result) to done }
"""
    source = replace_exact(source, "    state less {", arithmetic + "    state less {")
    views = """    state static_view {
        status=elf_code3(76,141,157) to done when(status!=0) status=elf_code_s32(0-word[13900000+result*8]) to done when(status!=0)
        status=elf_code3(72,141,5) to done when(status!=0) status=elf_code_rel32(word[525264]-4096+word[13400000+imm0*8]) to done when(status!=0)
        status=elf_code3(73,137,3) to done when(status!=0) status=elf_code_byte(184) to done when(status!=0)
        status=elf_code_u32(ckir_const_word(imm0,12)) to done when(status!=0) status=elf_code4(73,137,67,8) to done when(status!=0)
        status=elf_code3(76,137,157) to done when(status!=0) status=elf_code_s32(0-elf_value_slot(result)) to done
    }
    state view_nonempty {
        a=ckir_operand(start) status=elf_code3(76,139,157) to done when(status!=0) status=elf_code_s32(0-elf_value_slot(a)) to done when(status!=0)
        status=elf_code4(73,131,123,8) to done when(status!=0) status=elf_code_byte(0) to done when(status!=0)
        status=elf_code4(15,149,192,15) to done when(status!=0) status=elf_code2(182,192) to done when(status!=0)
        status=elf_store_value(result) to done
    }
    state view_head {
        a=ckir_operand(start) status=elf_code3(76,139,157) to done when(status!=0) status=elf_code_s32(0-elf_value_slot(a)) to done when(status!=0)
        status=elf_code4(73,131,123,8) to done when(status!=0) status=elf_code_byte(0) to done when(status!=0) status=elf_trap_jump(132) to done when(status!=0)
        status=elf_code3(73,139,3) to done when(status!=0) status=elf_code3(15,182,0) to done when(status!=0) status=elf_store_value(result) to done
    }
    state view_tail {
        a=ckir_operand(start) status=elf_code3(76,139,157) to done when(status!=0) status=elf_code_s32(0-elf_value_slot(a)) to done when(status!=0)
        status=elf_code3(76,141,149) to done when(status!=0) status=elf_code_s32(0-word[13900000+result*8]) to done when(status!=0)
        status=elf_code4(73,131,123,8) to done when(status!=0) status=elf_code_byte(0) to done when(status!=0) status=elf_trap_jump(132) to done when(status!=0)
        status=elf_code3(73,139,3) to done when(status!=0) status=elf_code4(72,131,192,1) to done when(status!=0) status=elf_code3(73,137,2) to done when(status!=0)
        status=elf_code4(73,139,67,8) to done when(status!=0) status=elf_code4(72,131,232,1) to done when(status!=0) status=elf_code4(73,137,66,8) to done when(status!=0)
        status=elf_code3(76,137,149) to done when(status!=0) status=elf_code_s32(0-elf_value_slot(result)) to done
    }
"""
    source = replace_exact(source, "    state add {", views + "    state add {")
    return source


R4_FRAME_READER = r"""proc omgrfn5_r2_read_frame() {
    let n=0 let c=read_byte() let overflow=0 let cursor=0
    state read { to one when(c>=0) to header }
    state one { to over when(n>=4497544) byte[1048576+n]=c n=n+1 c=read_byte() to read }
    state over { overflow=1 c=read_byte() to read }
    state header { to exhausted when(overflow==1) to bad when(n<40) to magic }
    state magic {
        to bad when(omgrfn4_l2_frame_byte(0)!='O') to bad when(omgrfn4_l2_frame_byte(1)!='M')
        to bad when(omgrfn4_l2_frame_byte(2)!='G') to bad when(omgrfn4_l2_frame_byte(3)!='R')
        to bad when(omgrfn4_l2_frame_byte(4)!='F') to bad when(omgrfn4_l2_frame_byte(5)!='N')
        to bad when(omgrfn4_l2_frame_byte(6)!='G') to bad when(omgrfn4_l2_frame_byte(7)!=0)
        to bad when(omgrfn5_r2_u32(8)!=16) word[879088]=16 to components
    }
    state components {
        word[500016]=omgrfn5_r2_u32(12) word[500032]=omgrfn5_r2_u32(16)
        word[500048]=omgrfn5_r2_u32(20) word[500064]=omgrfn5_r2_u32(24)
        word[500080]=omgrfn5_r2_u32(28) word[500088]=omgrfn5_r2_u32(32) word[500096]=omgrfn5_r2_u32(36)
        to flags_ok when(word[500016]==1) to flags_ok when(word[500016]==3) to bad
    }
    state flags_ok {
        to bad when(word[500032]<1) to bad when(word[500048]<1) to bad when(word[500064]<1) to bad when(word[500080]<1)
        to exhausted when(word[500032]>267280) to exhausted when(word[500048]>524288)
        to exhausted when(word[500064]>2522192) to exhausted when(word[500080]>1183744)
        cursor=40 word[500024]=cursor to bad when(word[500032]>n-cursor) cursor=cursor+word[500032] word[500040]=cursor
        to bad when(word[500048]>n-cursor) cursor=cursor+word[500048] word[500056]=cursor
        to bad when(word[500064]>n-cursor) cursor=cursor+word[500064] word[500072]=cursor
        to bad when(word[500080]!=n-cursor) return 0
    }
    state exhausted { return omgrfn4_l2_exhaust() }
    state bad { return omgrfn4_l2_reject() }
}"""


def patch_r4_common(source: str) -> str:
    source = replace_proc(source, "omgrfn5_r2_read_frame", R4_FRAME_READER)
    old = "state punctuation { to bad when (c>=128) to bad when (c==34) word[6000000+omgrfn4_r2_slot(source_id,count)*8]=p word[6290000+omgrfn4_r2_slot(source_id,count)*8]=1 word[6580000+omgrfn4_r2_slot(source_id,count)*8]=c count=count+1 p=p+1 to skip }"
    new = """state punctuation { to bad when(c>=128) to quote when(c==34) word[6000000+omgrfn4_r2_slot(source_id,count)*8]=p word[6290000+omgrfn4_r2_slot(source_id,count)*8]=1 word[6580000+omgrfn4_r2_slot(source_id,count)*8]=c count=count+1 p=p+1 to skip }
    state quote { start=p p=p+1 to quote_loop }
    state quote_loop { to bad when(p>=n) c=omgrfn4_l2_source_byte(source_id,p) to quote_done when(c==34) to bad when(c==92) p=p+1 to quote_loop }
    state quote_done { p=p+1 word[6000000+omgrfn4_r2_slot(source_id,count)*8]=start word[6290000+omgrfn4_r2_slot(source_id,count)*8]=p-start word[6580000+omgrfn4_r2_slot(source_id,count)*8]=258 count=count+1 to skip }"""
    source = replace_exact(source, old, new)
    return source


R4_LOWERING_JOIN = r"""proc r416_emit_source_opcode(opcode) {
    let i=word[12110000]
    state scan { to one when(i<word[950096]) to bad }
    state one { to found when(r47_ckir_byte(word[950296]+i*40+12)==8) to found when(r47_ckir_byte(word[950296]+i*40+12)==26) to found when(r47_ckir_byte(word[950296]+i*40+12)==27) i=i+1 to scan }
    state found { to bad when(r47_ckir_byte(word[950296]+i*40+12)!=opcode) word[12110000]=i+1 word[12110008]=word[12110008]+1 return 0 }
    state bad { return omgrfn4_l2_reject() }
}

proc r416_flush(precedence,all) {
    let top=word[12100000] let op=0 let p=0
    state loop { to done when(top==0) op=word[12100000+top*8] to marker when(op==0) p=1 to high when(op==42) p=2 to high }
    state marker { to drop when(all==1) to done }
    state high { to done when(p<precedence) to emit }
    state emit { to add when(op==43) to subtract when(op==45) to multiply }
    state add { to bad when(r416_emit_source_opcode(8)!=0) to drop }
    state subtract { to bad when(r416_emit_source_opcode(26)!=0) to drop }
    state multiply { to bad when(r416_emit_source_opcode(27)!=0) to drop }
    state drop { top=top-1 word[12100000]=top to loop }
    state done { return 0 }
    state bad { return word[700000] }
}

proc r416_operator_join() {
    let source=0 let token=0 let count=0 let kind=0 let next=0 let prior=0 let top=0 let precedence=0 let op=0 let i=0 let widen=0 let actual_widen=0
    word[12100000]=0 word[12110000]=0 word[12110008]=0
    state sources { to source_one when(source<word[500320]) to finish_source }
    state source_one { token=0 count=word[880000+source*8] prior=0 to tokens }
    state tokens { to token_one when(token<count) to source_done }
    state token_one { kind=omgrfn4_r2_tkind(source,token) next=0 to have_next when(token+1<count) to classify }
    state have_next { next=omgrfn4_r2_tkind(source,token+1) to classify }
    state classify {
        to widen_probe when(kind==256)
        to open when(kind==40) to close when(kind==41)
        to operator when(kind==43) to minus_probe when(kind==45) to operator when(kind==42)
        to boundary when(kind==59) to boundary when(kind==44) to boundary when(kind==123) to boundary when(kind==125) to boundary when(kind==61)
        prior=kind token=token+1 to tokens
    }
    state widen_probe { to ordinary when(omgrfn4_r2_tlen(source,token)!=2) to ordinary when(omgrfn4_l2_source_byte(source,omgrfn4_r2_tstart(source,token))!='a') to ordinary when(omgrfn4_l2_source_byte(source,omgrfn4_r2_tstart(source,token)+1)!='s') to widen_seen }
    state widen_seen { widen=widen+1 prior=kind token=token+1 to tokens }
    state minus_probe { to ordinary when(next==62) to operator }
    state ordinary { prior=kind token=token+1 to tokens }
    state operator { to operator_ok when(prior==256) to operator_ok when(prior==257) to operator_ok when(prior==41) to operator_ok when(prior==93) to ordinary }
    state operator_ok { op=kind precedence=1 to high when(kind==42) to flush }
    state high { precedence=2 to flush }
    state flush { to bad when(r416_flush(precedence,0)!=0) top=word[12100000]+1 to exhausted when(top>128) word[12100000]=top word[12100000+top*8]=op prior=kind token=token+1 to tokens }
    state open { top=word[12100000]+1 to exhausted when(top>128) word[12100000]=top word[12100000+top*8]=0 prior=kind token=token+1 to tokens }
    state close { to bad when(r416_flush(0,0)!=0) top=word[12100000] to close_drop when(top>0) prior=kind token=token+1 to tokens }
    state close_drop { to bad when(word[12100000+top*8]!=0) word[12100000]=top-1 prior=kind token=token+1 to tokens }
    state boundary { to bad when(r416_flush(0,1)!=0) prior=kind token=token+1 to tokens }
    state source_done { to bad when(r416_flush(0,1)!=0) source=source+1 to sources }
    state finish_source { i=word[12110000] to remaining }
    state remaining { to one when(i<word[950096]) to widen_ops }
    state one { kind=r47_ckir_byte(word[950296]+i*40+12) to bad when(kind==8) to bad when(kind==26) to bad when(kind==27) i=i+1 to remaining }
    state widen_ops { i=0 to widen_scan }
    state widen_scan { to widen_one when(i<word[950096]) to exact }
    state widen_one { to widen_yes when(r47_ckir_byte(word[950296]+i*40+12)==21) i=i+1 to widen_scan }
    state widen_yes { actual_widen=actual_widen+1 i=i+1 to widen_scan }
    state exact { to bad when(word[12110008]<1) to bad when(actual_widen!=widen) return 0 }
    state exhausted { return omgrfn4_l2_exhaust() }
    state bad { return omgrfn4_l2_reject() }
}

proc r416_lowering_check() {
    let status=0 let source=0
    state read { word[700000]=0 status=omgrfn5_r2_read_frame() to done when(status!=0) status=omgrfn4_l2_decode_compilation() to done when(status!=0) omgrfn4_r2_init_words() to tokenize }
    state tokenize { to one when(source<word[500320]) status=r47_ckir_prepare() to done when(status!=0) status=r416_operator_join() to done }
    state one { status=omgrfn4_r2_tokenize(source) to done when(status!=0) source=source+1 to tokenize }
    state done { return status }
}
proc main() { return r416_lowering_check() }
"""


VIEW_IMAGE_PREPARE = r"""proc r516_prepare_view_image() {
    let op=0 let root=0 let first=0 let count=0 let i=0 let child=0 let cursor=0
    state operations { to one when(op<ckir_count(9)) to finish }
    state one { to root_op when(ckir_op_byte(op,12)==22) op=op+1 to operations }
    state root_op {
        root=ckir_op_word(op,32) to bad when(root>=ckir_count(7))
        to bad when(ckir_type_byte(ckir_const_word(root,4),4)!=7)
        first=ckir_const_word(root,8) count=ckir_const_word(root,12)
        to exhausted when(count>32) to bad when(ckir_span_ok(first,count,ckir_count(8))==0)
        word[13400000+root*8]=cursor i=0 to children
    }
    state children { to child_one when(i<count) op=op+1 to operations }
    state child_one {
        child=ckir_child(first+i) to bad when(child>=ckir_count(7))
        to bad when(ckir_type_byte(ckir_const_word(child,4),4)!=1)
        to bad when(ckir_const_word(child,12)!=0) to bad when(ckir_const_word(child,16)>255)
        byte[13700000+cursor]=ckir_const_word(child,16)%256 cursor=cursor+1 i=i+1 to children
    }
    state finish { word[525304]=cursor word[525312]=0 to ro when(cursor>0) return 0 }
    state ro { word[525312]=ckir_align(cursor,4096) return 0 }
    state exhausted { return 252 }
    state bad { return 251 }
}"""

R5_LAYOUT_TYPES = r"""proc r516_layout_types() {
    let i=0 let s=0
    state loop { to one when(i<ckir_count(0)) return 0 }
    state one { s=ckir_layout_type(i) to done when(s!=0) i=i+1 to loop }
    state done { return s }
}"""


def materialize(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    envelope = v16_envelope()
    artifact = patch_artifact(read("ckir5-refinement-artifact.beta"))
    r5_structure = envelope + "\n" + artifact
    r3 = patch_r3_independent(read("omgrfn7-witness-ckir5-tables.beta"))
    result = patch_result(read("ckir5-refinement-result.beta"))
    elf = patch_elf(read("ckir5-refinement-elf.beta"))
    artifact_core = (
        BASE.before(artifact, "proc ckir_constant_key_after")
        + BASE.between(artifact, "proc ckir_value_type", "proc ckir_initialize_call_graph")
        + BASE.between(artifact, "proc ckir_validate_operations", "proc ckir5_preserve_tables")
    )
    graph_stubs = "proc ckir_initialize_call_graph(){return 0}\nproc ckir_record_call_edge(a,b){return 0}\nproc ckir_validate_call_graph(){return 0}\n"
    r5_result = BASE.prune(
        envelope + BASE.before(artifact, "proc ckir_constant_key_after")
        + BASE.between(artifact, "proc ckir_value_type", "proc ckir_initialize_call_graph")
        + BASE.procedure(elf, "elf_assign_operation_types")
        + BASE.before(result, "proc ckir5_refinement_artifact_check") + graph_stubs + VIEW_IMAGE_PREPARE + "\n"
        + """proc main(){let s=omgrfn5_component_read()
 state a {to z when(s!=0) s=ckir_decode_header() to z when(s!=0) s=ckir_validate_types_records() to z when(s!=0)
 s=ckir_validate_machines_blocks() to z when(s!=0) s=elf_assign_operation_types() to z when(s!=0)
 s=r516_prepare_view_image() to z when(s!=0)
 s=ckir_assign_constructor_objects() to z when(s!=0) s=ckir_interpret_selected() to z}
 state z{return s}}
"""
    )
    r5_elf = BASE.prune(
        envelope + artifact_core + BASE.procedure(artifact, "ckir5_preserve_tables")
        + BASE.before(elf, "proc main()") + graph_stubs + VIEW_IMAGE_PREPARE + "\n" + R5_LAYOUT_TYPES + "\n"
        + """proc main(){let s=omgrfn5_component_read()
 state a {to z when(s!=0) s=ckir_decode_header() to z when(s!=0) s=r516_layout_types() to z when(s!=0)
 s=elf_assign_operation_types() to z when(s!=0)
 s=r516_prepare_view_image() to z when(s!=0) s=ckir5_preserve_tables() to z when(s!=0)
 s=ckir5_refinement_elf_check() to z}
 state z{return s}}
"""
    )
    rows = [BASE.write_checked(output, "r3", BASE.prune(r3)),
            BASE.write_checked(output, "r5-structure", r5_structure),
            BASE.write_checked(output, "r5-result", r5_result),
            BASE.write_checked(output, "r5-elf", r5_elf)]
    (output / "manifest-r3-r5.tsv").write_text("".join(rows), encoding="ascii")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    materialize(args.output)


if __name__ == "__main__":
    main()
