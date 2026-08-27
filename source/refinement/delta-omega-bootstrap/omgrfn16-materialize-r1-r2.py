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


def replace_procedure(source: str, name: str, body: str) -> str:
    match = re.search(rf"(?m)^proc {re.escape(name)}\([^)]*\) \{{", source)
    if match is None:
        raise ValueError(f"OMGRFN16 missing procedure anchor: {name}")
    depth = 1
    end = match.end()
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    return source[:match.start()] + body.rstrip() + "\n" + source[end:]


def patch_r1(frame: str) -> str:
    # OMGRFN4 contributes the complete OMGCOMP custody procedures and also has
    # its retired frame reader.  Patch only the OMGRFN5 adapter which R1 calls;
    # retaining the old reader byte-for-byte avoids accidentally broadening a
    # shared custody implementation while still giving V16 exact framing.
    adapter_at = frame.index("proc omgrfn5_read()")
    custody = frame[:adapter_at]
    frame = frame[adapter_at:]
    frame = replace(
        frame,
        "        to version14_magic when (omgrfn5_byte(6) == 'E')\n        to malformed",
        "        to version14_magic when (omgrfn5_byte(6) == 'E')\n"
        "        to version16_magic when (omgrfn5_byte(6) == 'G')\n        to malformed",
    )
    frame = replace(
        frame,
        "    state version14_magic { word[500104] = 14  to magic7 }",
        "    state version14_magic { word[500104] = 14  to magic7 }\n"
        "    state version16_magic { word[500104] = 16  to magic7 }",
    )
    frame = replace(
        frame,
        "        to malformed when (flags > 1)",
        "        to malformed when (flags > 3)\n"
        "        to malformed when (flags == 0)\n"
        "        to malformed when (flags == 2)",
    )
    frame = replace(
        frame,
        "        to library when (flags == 0)\n        to entry",
        "        to success when (flags == 1)\n        to trap",
    )
    frame = replace(
        frame,
        "    state library {\n"
        "        to malformed when (elf_length != 0)\n"
        "        to malformed when (result != 4294967295)\n"
        "        to malformed when (exit_projection != 4294967295)\n"
        "        return 0\n"
        "    }\n"
        "    state entry {",
        "    state success {",
    )
    frame = replace(
        frame,
        "        to malformed when (exit_projection != (result % 256))\n"
        "        return 0\n"
        "    }\n"
        "    state exhausted",
        "        to malformed when (exit_projection != (result % 256))\n"
        "        return 0\n"
        "    }\n"
        "    state trap {\n"
        "        to malformed when (elf_length < 1)\n"
        "        to malformed when (result != 4294967295)\n"
        "        to malformed when (exit_projection != 4294967295)\n"
        "        return 0\n"
        "    }\n"
        "    state exhausted",
    )
    frame += """
proc omgrfn16_layer1_check() {
    let status=omgrfn5_layer1_check()
    state checked { to done when(status!=0) to exact when(word[500104]==16) return 251 }
    state exact { return 0 }
    state done { return status }
}
proc main() { return omgrfn16_layer1_check() }
"""
    return custody + frame


R2_EXTRA = r"""
proc omgrfn16_r2_register_extra(raw) {
    let i=0 let n=word[1319000]
    state find { to one when(i<n) to room }
    state one { to found when(word[1319008+i*8]==raw) i=i+1 to find }
    state room { to exhausted when(n>=3) word[1319008+n*8]=raw word[1319000]=n+1 return n }
    state found { return i }
    state exhausted { omgrfn4_l2_exhaust() return 4294967295 }
}
"""


R2_TYPE_ID = r"""
proc omgrfn5_r2_type_id(raw) {
    let i=0 let base=word[879008]+word[879344]
    state nominal0 { to nominal1 when(raw>=100) to bad }
    state nominal1 { to nominal when(raw<100+word[879008]) to sum0 }
    state nominal { return raw-100 }
    state sum0 { to sum1 when(raw>=300) to scalar }
    state sum1 { to sum when(raw<300+word[879344]) to scalar }
    state sum { return word[879008]+raw-300 }
    state scalar { to boolean when(raw==200) to full when(raw==202) to extras }
    state boolean { return base }
    state full { return base+1 }
    state extras { to extra when(i<word[1319000]) to bad }
    state extra { to found when(word[1319008+i*8]==raw) i=i+1 to extras }
    state found { return base+2+i }
    state bad { return 4294967295 }
}
"""


R2_PARSE_TYPE = r"""
proc omgrfn5_r2_parse_type(source_id) {
    let t=omgrfn4_r2_cur(source_id) let rid=0
    word[879100]=4294967295 word[879108]=0 word[879116]=0 word[879124]=0
    state room { to bad when(t>=word[880000+source_id*8]) to shared when(omgrfn4_r2_tkind(source_id,t)==38) to bad when(omgrfn4_r2_tkind(source_id,t)==91) to scalar }
    state scalar {
        to bool when(omgrfn4_r2_is_word(source_id,t,6)==1)
        to u8 when(omgrfn4_r2_is_word(source_id,t,7)==1)
        to u32 when(omgrfn4_r2_is_word(source_id,t,8)==1)
        to nominal
    }
    state bool { word[879100]=200 omgrfn4_r2_next(source_id) return 0 }
    state u8 { word[879100]=201 omgrfn16_r2_register_extra(201) to bad when(word[700000]!=0) omgrfn4_r2_next(source_id) return 0 }
    state shared { omgrfn4_r2_next(source_id) to bad when(omgrfn4_r2_expect_kind(source_id,91)!=0) to element }
    state element { to bad when(omgrfn4_r2_expect_word(source_id,7)!=0) omgrfn16_r2_register_extra(201) to bad when(word[700000]!=0) to close_shared }
    state close_shared { to bad when(omgrfn4_r2_expect_kind(source_id,93)!=0) word[879100]=204 omgrfn16_r2_register_extra(204) to bad when(word[700000]!=0) return 0 }
    state nominal {
        to bad when(omgrfn4_r2_tkind(source_id,t)!=256)
        omgrfn4_r2_find_record(source_id,t) rid=word[879132]
        to record when(rid!=4294967295)
        omgrfn7_r2_find_sum(source_id,t) rid=word[879132]
        to bad when(rid==4294967295)
        word[879100]=300+rid word[879108]=source_id word[879116]=t
        omgrfn4_r2_next(source_id) return 0
    }
    state record { word[879100]=100+rid word[879108]=source_id word[879116]=t omgrfn4_r2_next(source_id) return 0 }
    state u32 { word[879100]=202 omgrfn4_r2_next(source_id) to u32_constraint }
    state u32_constraint { to trapping when(omgrfn4_r2_cur(source_id)<word[880000+source_id*8]) return 0 }
    state trapping { to no_constraint when(omgrfn4_r2_is_word(source_id,omgrfn4_r2_cur(source_id),9)==0) omgrfn4_r2_next(source_id) to bad when(omgrfn4_r2_expect_word(source_id,10)!=0) word[879100]=203 omgrfn16_r2_register_extra(203) to bad when(word[700000]!=0) return 0 }
    state no_constraint { return 0 }
    state bad { return omgrfn4_l2_reject() }
}
"""


R2_EMIT_TYPE = r"""
proc omgrfn5_r2_emit_type(id) {
    let kind=0 let flags=0 let a=0 let b=0 let lo=0 let hi=0 let base=word[879008]+word[879344] let raw=0 let extra=0
    state select { to nominal when(id<word[879008]) to sum when(id<base) to bool when(id==base) to full when(id==base+1) extra=id-base-2 to extra_bounds }
    state nominal { kind=4 a=id to emit }
    state sum { kind=6 a=id-word[879008] to emit }
    state bool { kind=3 hi=1 to emit }
    state full { kind=2 hi=4294967295 to emit }
    state extra_bounds { to bad when(extra<0) to bad when(extra>=word[1319000]) raw=word[1319008+extra*8] to extra_kind }
    state extra_kind { to u8 when(raw==201) to trapping when(raw==203) to shared when(raw==204) to bad }
    state u8 { kind=1 hi=255 to emit }
    state trapping { kind=2 flags=1 hi=4294967295 to emit }
    state shared { kind=7 a=omgrfn5_r2_type_id(201) to bad when(a==4294967295) to emit }
    state emit { omgrfn4_l2_put_u32(id) omgrfn4_l2_put_byte(kind) omgrfn4_l2_put_byte(flags) omgrfn4_l2_put_u16(0) omgrfn4_l2_put_u32(a) omgrfn4_l2_put_u32(b) omgrfn4_l2_put_u32(lo) omgrfn4_l2_put_u32(hi) return 0 }
    state bad { return omgrfn4_l2_reject() }
}
"""


R2_TOKENIZE = r"""
proc omgrfn4_r2_tokenize(source_id) {
    let p=0 let n=omgrfn4_l2_source_length(source_id) let count=0 let start=0 let c=0 let depth=0 let payload=0
    state skip { to classify when(p<n) to finish }
    state classify {
        c=omgrfn4_l2_source_byte(source_id,p)
        to whitespace when(c==' ') to whitespace when(c==9) to whitespace when(c==10) to whitespace when(c==13)
        to string_open when(c==34) to slash when(c=='/') to capacity
    }
    state whitespace { p=p+1 to skip }
    state slash { to capacity when(p+1>=n) to line_begin when(omgrfn4_l2_source_byte(source_id,p+1)=='/') to block_begin when(omgrfn4_l2_source_byte(source_id,p+1)=='*') to capacity }
    state line_begin { p=p+2 to line }
    state line { to finish when(p>=n) to whitespace when(omgrfn4_l2_source_byte(source_id,p)==10) p=p+1 to line }
    state block_begin { depth=1 p=p+2 to block }
    state block { to bad when(p>=n) to block_slash }
    state block_slash { to block_star when(omgrfn4_l2_source_byte(source_id,p)!='/') to block_open_room }
    state block_open_room { to block_advance when(p+1>=n) to block_open }
    state block_open { to block_advance when(omgrfn4_l2_source_byte(source_id,p+1)!='*') depth=depth+1 p=p+2 to block }
    state block_star { to block_advance when(omgrfn4_l2_source_byte(source_id,p)!='*') to block_close_room }
    state block_close_room { to block_advance when(p+1>=n) to block_close }
    state block_close { to block_advance when(omgrfn4_l2_source_byte(source_id,p+1)!='/') depth=depth-1 p=p+2 to block_done when(depth==0) to block }
    state block_advance { p=p+1 to block }
    state block_done { to skip }
    state string_open { start=p payload=0 p=p+1 to string }
    state string { to bad when(p>=n) c=omgrfn4_l2_source_byte(source_id,p) to string_close when(c==34) to bad when(c==92) to string_one }
    state string_one { payload=payload+1 to exhausted when(payload>32) p=p+1 to string }
    state string_close { p=p+1 to token_room }
    state token_room { to exhausted when(count>=18000) word[6000000+omgrfn4_r2_slot(source_id,count)*8]=start word[6290000+omgrfn4_r2_slot(source_id,count)*8]=p-start word[6580000+omgrfn4_r2_slot(source_id,count)*8]=258 count=count+1 to skip }
    state capacity { to exhausted when(count>=18000) start=p to ident when(omgrfn4_r2_ident_start(c)==1) to number0 }
    state ident { p=p+1 to save_ident when(p>=n) c=omgrfn4_l2_source_byte(source_id,p) to save_ident when(omgrfn4_r2_ident_continue(c)==0) to ident }
    state save_ident { word[6000000+omgrfn4_r2_slot(source_id,count)*8]=start word[6290000+omgrfn4_r2_slot(source_id,count)*8]=p-start word[6580000+omgrfn4_r2_slot(source_id,count)*8]=256 count=count+1 to skip }
    state number0 { to punctuation when(c<'0') to punctuation when(c>'9') p=p+1 to number }
    state number { to save_number when(p>=n) c=omgrfn4_l2_source_byte(source_id,p) to save_number when(c<'0') to save_number when(c>'9') p=p+1 to number }
    state save_number { word[6000000+omgrfn4_r2_slot(source_id,count)*8]=start word[6290000+omgrfn4_r2_slot(source_id,count)*8]=p-start word[6580000+omgrfn4_r2_slot(source_id,count)*8]=257 count=count+1 to skip }
    state punctuation { to bad when(c>=128) word[6000000+omgrfn4_r2_slot(source_id,count)*8]=p word[6290000+omgrfn4_r2_slot(source_id,count)*8]=1 word[6580000+omgrfn4_r2_slot(source_id,count)*8]=c count=count+1 p=p+1 to skip }
    state finish { word[880000+source_id*8]=count word[880016+source_id*8]=0 return 0 }
    state exhausted { return omgrfn4_l2_exhaust() }
    state bad { return omgrfn4_l2_reject() }
}
"""


R2_ARITHMETIC = r"""
; OMGRFN16 R2: bounded, source-only recognition of the selected recursive
; same-carrier arithmetic grammar.  The exact witness comparison above owns
; every named declaration/type link; this parser proves that each selected
; punctuation token belongs to one admitted expression rather than merely
; counting punctuation in comments, strings, arrows, or excluded syntax.
proc a_cur(depth) { return word[1320100+depth*32] }
proc a_type(depth) { return word[1320108+depth*32] }
proc a_tree(depth) { return word[1320116+depth*32] }
proc a_ops(depth) { return word[1320124+depth*32] }
proc a_set(depth,cursor,type_id,tree) { word[1320100+depth*32]=cursor word[1320108+depth*32]=type_id word[1320116+depth*32]=tree word[1320124+depth*32]=0 return 0 }
proc a_set_ops(depth,ops) { word[1320124+depth*32]=ops return 0 }
proc a_max(a,b) { state left { to right when(b>a) return a } state right { return b } }

proc a_record_op(token) {
    let n=word[1320008]
    state room { to exhausted when(n>=256) word[1321000+n*8]=token word[1320008]=n+1 return 0 }
    state exhausted { return omgrfn4_l2_exhaust() }
}

proc a_block(source,token) {
    let i=0 let position=omgrfn4_r2_tstart(source,token) let machine=0
    state blocks { to one when(i<word[879040]) return 4294967295 }
    state one { machine=word[888000+i*96] to next when(word[884000+machine*128]!=source) to next when(position<word[888024+i*96]) to next when(position>=word[888032+i*96]) return i }
    state next { i=i+1 to blocks }
}

proc a_leaf_type(source,token,block,self_field) {
    let i=0 let machine=0 let owner=0 let at=0 let start=omgrfn4_r2_tstart(source,token) let length=omgrfn4_r2_tlen(source,token)
    state context { to missing when(block==4294967295) machine=word[888000+block*96] owner=word[884088+machine*128] to fields when(self_field==1) to mparams }
    state fields { to field when(i<word[879016]) i=0 to missing }
    state field { at=882000+i*64 to field_next when(word[at]!=owner) to field_next when(word[at+40]!=length) word[700056]=length to field_next when(omgrfn4_l2_span_equal(source,start,word[881000+owner*64],word[at+32])==0) return word[at+16] }
    state field_next { i=i+1 to fields }
    state mparams { to mparam when(i<word[879032]) i=0 to bparams }
    state mparam { at=886000+i*48 to mparam_next when(word[at]!=machine) to mparam_next when(word[at+32]!=length) word[700056]=length to mparam_next when(omgrfn4_l2_span_equal(source,start,word[884000+machine*128],word[at+24])==0) return word[at+16] }
    state mparam_next { i=i+1 to mparams }
    state bparams { to bparam when(i<word[879048]) to missing }
    state bparam { at=893000+i*48 to bparam_next when(word[at]!=block) to bparam_next when(word[at+32]!=length) word[700056]=length to bparam_next when(omgrfn4_l2_span_equal(source,start,word[884000+machine*128],word[at+24])==0) return word[at+16] }
    state bparam_next { i=i+1 to bparams }
    state missing { return 4294967295 }
}

proc a_decimal(source,token) {
    let i=0 let value=0 let digit=0 let n=omgrfn4_r2_tlen(source,token) let at=omgrfn4_r2_tstart(source,token)
    state digits { to one when(i<n) word[1320032]=value return 0 }
    state one { digit=omgrfn4_l2_source_byte(source,at+i)-48 to bad when(digit<0) to bad when(digit>9) to bad when(value>429496729) to last when(value==429496729) to add }
    state last { to bad when(digit>5) to add }
    state add { value=value*10+digit i=i+1 to digits }
    state bad { return 1 }
}

proc a_primary(depth) {
    let source=word[1320000] let cursor=a_cur(depth) let count=word[880000+source*8] let kind=0 let type_id=0 let block=0 let tree=1 let ops=0
    state room { to no when(depth>=16) to no when(cursor>=count) kind=omgrfn4_r2_tkind(source,cursor) to number when(kind==257) to grouped when(kind==40) to word when(kind==256) to no }
    state number { to no when(a_decimal(source,cursor)!=0) type_id=omgrfn5_r2_type_id(203) cursor=cursor+1 to postfix }
    state grouped { a_set(depth+1,cursor+1,0,0) to no when(a_add(depth+1)!=0) cursor=a_cur(depth+1) type_id=a_type(depth+1) tree=a_tree(depth+1) ops=a_ops(depth+1) to no when(cursor>=count) to no when(omgrfn4_r2_tkind(source,cursor)!=41) cursor=cursor+1 to postfix }
    state word { block=a_block(source,cursor) to self when(omgrfn4_r2_is_word(source,cursor,5)==1) type_id=a_leaf_type(source,cursor,block,0) cursor=cursor+1 to no when(type_id==4294967295) to postfix }
    state self { to no when(cursor+2>=count) to no when(omgrfn4_r2_tkind(source,cursor+1)!=46) to no when(omgrfn4_r2_tkind(source,cursor+2)!=256) type_id=a_leaf_type(source,cursor+2,block,1) cursor=cursor+3 to no when(type_id==4294967295) to postfix }
    state postfix { to plain when(cursor>=count) to cast when(omgrfn12_r2_is_as(source,cursor)==1) to plain }
    state cast { to no when(type_id!=omgrfn5_r2_type_id(201)) to no when(cursor+3>=count) to no when(omgrfn4_r2_is_word(source,cursor+1,8)==0) to no when(omgrfn4_r2_is_word(source,cursor+2,9)==0) to no when(omgrfn4_r2_is_word(source,cursor+3,10)==0) type_id=omgrfn5_r2_type_id(203) cursor=cursor+4 tree=tree+1 to over when(tree>8) a_set(depth,cursor,type_id,tree) a_set_ops(depth,ops) return 0 }
    state plain { to no when(type_id!=omgrfn5_r2_type_id(203)) a_set(depth,cursor,type_id,tree) a_set_ops(depth,ops) return 0 }
    state over { return omgrfn4_l2_exhaust() }
    state no { return 1 }
}

proc a_mul(depth) {
    let source=word[1320000] let cursor=0 let type_id=0 let tree=0 let ops=0 let right_tree=0 let right_ops=0 let token=0
    state first { to no when(a_primary(depth)!=0) cursor=a_cur(depth) type_id=a_type(depth) tree=a_tree(depth) ops=a_ops(depth) to loop }
    state loop { to done when(cursor>=word[880000+source*8]) to done when(omgrfn4_r2_tkind(source,cursor)!=42) token=cursor a_set(depth+1,cursor+1,0,0) to no when(a_primary(depth+1)!=0) to no when(type_id!=omgrfn5_r2_type_id(203)) to no when(a_type(depth+1)!=type_id) right_tree=a_tree(depth+1) right_ops=a_ops(depth+1) tree=a_max(tree,right_tree)+1 ops=ops+right_ops+1 cursor=a_cur(depth+1) to over when(tree>8) to over when(a_record_op(token)!=0) to loop }
    state done { a_set(depth,cursor,type_id,tree) a_set_ops(depth,ops) return 0 }
    state over { return omgrfn4_l2_exhaust() }
    state no { return 1 }
}

proc a_add(depth) {
    let source=word[1320000] let cursor=0 let type_id=0 let tree=0 let ops=0 let right_tree=0 let right_ops=0 let token=0 let kind=0
    state first { to no when(a_mul(depth)!=0) cursor=a_cur(depth) type_id=a_type(depth) tree=a_tree(depth) ops=a_ops(depth) to loop }
    state loop { to done when(cursor>=word[880000+source*8]) kind=omgrfn4_r2_tkind(source,cursor) to operator when(kind==43) to operator when(kind==45) to done }
    state operator { token=cursor a_set(depth+1,cursor+1,0,0) to no when(a_mul(depth+1)!=0) to no when(type_id!=omgrfn5_r2_type_id(203)) to no when(a_type(depth+1)!=type_id) right_tree=a_tree(depth+1) right_ops=a_ops(depth+1) tree=a_max(tree,right_tree)+1 ops=ops+right_ops+1 cursor=a_cur(depth+1) to over when(tree>8) to over when(a_record_op(token)!=0) to loop }
    state done { a_set(depth,cursor,type_id,tree) a_set_ops(depth,ops) return 0 }
    state over { return omgrfn4_l2_exhaust() }
    state no { return 1 }
}

proc a_boundary(source,cursor) {
    let count=word[880000+source*8] let kind=0 let next=0
    state room { to yes when(cursor>=count) kind=omgrfn4_r2_tkind(source,cursor) to yes when(kind==59) to yes when(kind==44) to yes when(kind==123) to yes when(kind==125) to equal when(kind==61) to close when(kind==41) to arrow when(kind==45) return 0 }
    state equal { to no when(cursor+1>=count) to yes when(omgrfn4_r2_tkind(source,cursor+1)==61) to no }
    state arrow { to no when(cursor+1>=count) to yes when(omgrfn4_r2_tkind(source,cursor+1)==62) to no }
    state close { to yes when(cursor+1>=count) next=omgrfn4_r2_tkind(source,cursor+1) to yes when(next==59) to yes when(next==44) to yes when(next==41) to yes when(next==123) to yes when(next==125) to equal_after when(next==61) return 0 }
    state equal_after { to no when(cursor+2>=count) to yes when(omgrfn4_r2_tkind(source,cursor+2)==61) to no }
    state yes { return 1 }
    state no { return 0 }
}

proc a_mark(source) {
    let i=0 let n=word[1320008]
    state loop { to one when(i<n) return 0 }
    state one { byte[16000000+source*18000+word[1321000+i*8]]=1 i=i+1 to loop }
}

proc a_source(source) {
    let token=0 let count=word[880000+source*8] let cursor=0 let ops=0 let kind=0 let previous=0
    word[1320000]=source
    state scan { to attempt when(token<count) to verify }
    state attempt { word[1320008]=0 a_set(0,token,0,0) to next when(a_add(0)!=0) cursor=a_cur(0) ops=a_ops(0) to next when(ops<1) to next when(a_boundary(source,cursor)==0) a_mark(source) word[1320040]=word[1320040]+ops token=cursor to scan }
    state next { token=token+1 to scan }
    state verify { token=0 to verify_loop }
    state verify_loop { to verify_one when(token<count) return 0 }
    state verify_one { kind=omgrfn4_r2_tkind(source,token) to selected when(kind==43) to selected when(kind==42) to minus when(kind==45) token=token+1 to verify_loop }
    state minus { to arrow when(token+1<count) to unary }
    state arrow { to unselected when(omgrfn4_r2_tkind(source,token+1)==62) to unary }
    state unary { to unselected when(token==0) previous=omgrfn4_r2_tkind(source,token-1) to selected when(previous==256) to selected when(previous==257) to selected when(previous==41) to unselected }
    state selected { to bad when(byte[16000000+source*18000+token]!=1) token=token+1 to verify_loop }
    state unselected { token=token+1 to verify_loop }
    state bad { return omgrfn4_l2_reject() }
}

proc omgrfn16_r2_arithmetic_check() {
    let source=0 let status=0
    word[1320040]=0
    state sources { to one when(source<word[500320]) to exact }
    state one { status=a_source(source) to done when(status!=0) source=source+1 to sources }
    state exact { to bad when(word[1320040]<1) return 0 }
    state bad { return omgrfn4_l2_reject() }
    state done { return status }
}
"""


def patch_r2(source: str) -> str:
    source = replace(source, "to v13 when (omgrfn4_l2_frame_byte(6)=='D') to bad",
                     "to v16 when (omgrfn4_l2_frame_byte(6)=='G') to bad")
    source = replace(source,
        "state v13 { to bad when (omgrfn5_r2_u32(8)!=13) word[879088]=13 to components }",
        "state v16 { to bad when (omgrfn5_r2_u32(8)!=16) word[879088]=16 to components }")
    source = replace(source, "to bad when (word[500016]>1)",
                     "to bad when (word[500016]>3) to bad when (word[500016]==0) to bad when (word[500016]==2)")
    source = replace(source,
        "to bad when (word[500080]!=n-cursor) to library when (word[500016]==0) to entry",
        "to bad when (word[500080]!=n-cursor) return 0")
    source = re.sub(r"\n    state library \{[^\n]+\}\n    state entry \{[^\n]+\}", "", source, count=1)
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
        "state v16 { word[879376]=7 status=omgrfn5_r2_resolve() to resolved }")
    source = replace(source,
        "state bound { to done when (status!=0) to v3_emit when (word[879376]==3) status=omgrfn8_r2_emit_expected_legacy() to built }",
        "state bound { to done when (status!=0) status=omgrfn16_r2_arithmetic_check() to arithmetic }\n"
        "    state arithmetic { to done when (status!=0) status=omgrfn7_r2_emit_expected_v3() to built }")
    source = replace(source,
        "state checked { to done when (status!=0) to exact when (word[879088]==13) return 251 }",
        "state checked { to done when (status!=0) to exact when (word[879088]==16) return 251 }")
    source = replace(source, "proc omgrfn13_r2_check()", "proc omgrfn16_r2_check()")
    source = replace(source, "proc main() { return omgrfn13_r2_check() }",
                     "proc main() { return omgrfn16_r2_check() }")
    # V7 retains the two canonical base scalars (bool and unqualified u32),
    # then interns u8, trapping-u32, and the optional shared-u8 slice in exact
    # first-encounter order.  Raw parser tags stay symbolic until every record,
    # sum, and parameter has been discovered.
    source = replace(source,
        "word[879376]=0 word[879384]=0 word[879392]=0 word[879432]=0 word[879440]=0 word[879448]=0 word[879456]=0 word[879464]=0 word[879472]=0",
        "word[879376]=0 word[879384]=0 word[879392]=0 word[879432]=0 word[879440]=0 word[879448]=0 word[879456]=0 word[879464]=0 word[879472]=0 word[1319000]=0")
    source = replace(source,
        "source=0 omgrfn8_r2_count_logical_not() omgrfn9_r2_count_logical_binary() omgrfn10_r2_count_scalar_equal() omgrfn11_r2_count_ordered() omgrfn12_r2_count_integer_widen() omgrfn13_r2_count_trapping_add() to parse",
        "source=0 to parse")

    source = replace_procedure(source, "omgrfn5_r2_type_id", R2_TYPE_ID)
    source = replace_procedure(source, "omgrfn5_r2_parse_type", R2_PARSE_TYPE)
    source = replace_procedure(source, "omgrfn5_r2_emit_type", R2_EMIT_TYPE)
    source = replace_procedure(source, "omgrfn4_r2_tokenize", R2_TOKENIZE)
    # Insert the extra-type registrar immediately before its first consumer.
    anchor = source.index("proc omgrfn5_r2_type_id(raw)")
    source = source[:anchor] + R2_EXTRA + "\n" + source[anchor:]

    source = replace(source,
        "state version { word[879456]=0 to v12 when (word[879088]>=12) to sized }\n"
        "    state v12 { word[879456]=1 to sized }\n"
        "    state sized { expected=84+word[500320]*36+word[879072]*28+word[879056]*28+(word[879008]+word[879344]+3+word[879456])*24",
        "state version { to sized }\n"
        "    state sized { expected=84+word[500320]*36+word[879072]*28+word[879056]*28+(word[879008]+word[879344]+2+word[1319000])*24")
    source = replace(source,
        "omgrfn4_l2_put_u32(word[879008]+word[879344]+3+word[879456])",
        "omgrfn4_l2_put_u32(word[879008]+word[879344]+2+word[1319000])")
    source = replace(source,
        "state types { to type when (i<word[879008]+word[879344]+3+word[879456]) i=0 to records }",
        "state types { to type when (i<word[879008]+word[879344]+2+word[1319000]) i=0 to records }")

    # Exact source-expression recognition is a separate conjunct inside R2:
    # it consumes source/token/declaration tables only, never CKIR or ELF.
    source += "\n" + R2_ARITHMETIC
    return source


def main() -> None:
    parser = argparse.ArgumentParser(); parser.add_argument("output", type=Path); args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    shared = load_shared()
    temp = args.output / ".v13"
    shared.materialize(temp, 13)
    r1 = patch_r1(
        (HERE / "omgrfn4-frame-omgcomp-custody.beta").read_text(encoding="ascii")
        + "\n"
        + (HERE / "omgrfn5-frame-omgcomp-custody.beta").read_text(encoding="ascii")
    )
    r2 = patch_r2((temp / "r2.beta").read_text(encoding="ascii"))
    r1_shape = shared.write_checked(args.output / "r1.beta", r1)
    r2_shape = shared.write_checked(args.output / "r2.beta", r2)
    (args.output / "manifest.tsv").write_text(
        f"r1\t{r1_shape[0]}\t{r1_shape[1]}\n"
        f"r2\t{r2_shape[0]}\t{r2_shape[1]}\n", encoding="ascii")


if __name__ == "__main__": main()
