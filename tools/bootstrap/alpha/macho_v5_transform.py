#!/usr/bin/env python3
"""Manufacture the AlphaBootstrapV5 macOS seed from the V4 binary.

Replicates what `clang -arch arm64 -Wl,-no_uuid alpha_arm64_macos.s` emits for
the updated source: the mmap startup sequence sits inline in __text (everything
after it shifts by one insertion delta), __bss loses the 1.75 GiB mem zerofill,
and every dependent field (section sizes, segment spans, symtab, linkedit item
offsets, function starts, dysymtab, indirect symbols) is recomputed. Verified
by capstone disassembly against the .s source.
"""
import struct, sys
from capstone import Cs, CS_ARCH_ARM64, CS_MODE_ARM

# argv: <committed V4 binary> <output path>  (run from the repository root)
old = open(sys.argv[1], 'rb').read()

# --- encodings (verified below against capstone) ---
def movz(reg, imm16, shift):  # 64-bit movz: sf opc hw imm16 rd
    return 0xD2800000 | ((shift // 16) << 21) | (imm16 << 5) | reg
def movn(reg, imm16, shift=0):
    return 0x92800000 | ((shift // 16) << 21) | (imm16 << 5) | reg
def mov_reg(dst, src):        # mov xD, xS = orr xD, xzr, xS
    return 0xAA000000 | (src << 16) | (31 << 5) | dst
def svc(imm):
    return 0xD4000001 | (imm << 5)
def b_cond(target, pc, cond):
    return 0x54000000 | ((((target - pc) // 4) & 0x7FFFF) << 5) | cond
def nop():
    return 0xD503201F

NEW = []          # init sequence replacing [0x4a8,0x4b0)
NEW += [movz(0, 0, 0)]            # x0 = NULL
NEW += [movz(1, 0x2000, 32)]      # x1 = MEMSIZE 0x2000000000
NEW += [movz(2, 3, 0)]            # x2 = PROT_READ|PROT_WRITE
NEW += [movz(3, 0x1002, 0)]       # x3 = MAP_PRIVATE|MAP_ANON
NEW += [movn(4, 0)]               # x4 = -1
NEW += [movz(5, 0, 0)]            # x5 = 0
NEW += [movz(16, 0xC5, 0)]        # x16 = 197 mmap
NEW += [svc(0x80)]
NEW += [None]                     # b.cs Lbounds (patched below)
NEW += [mov_reg(20, 0)]           # x20 = x0
DELTA = 4 * (len(NEW) - 2)        # +32 bytes: 10 insns replace 2

TEXT_OFF, TEXT_VMA, TEXT_SIZE = 0x490, 0x100000490, 0x454
STUBS_OFF = 0x8e4
LBOUNDS_VMA = 0x1000005e4         # trap label, shifts by DELTA

ins_vma = TEXT_VMA + 0x18         # 0x1000004a8 insertion point
fail_vma = LBOUNDS_VMA + DELTA    # 0x100000604
bcs_addr = ins_vma + 8 * 4        # svc is instr 8, b.cs is instr 9 at +0x20
NEW[8] = b_cond(fail_vma, bcs_addr, 2)   # cond 2 = CS/HS

init = b''.join(struct.pack('<I', w) for w in NEW)
movz23 = struct.pack('<I', movz(23, 0x2000, 32))  # replaces word at 0x4b0

# --- new __text ---
old_text = old[TEXT_OFF:TEXT_OFF + TEXT_SIZE]
cut = 0x4a8 - 0x490               # 0x18
new_text = old_text[:cut] + init + movz23 + old_text[cut + 0xC:]
assert len(new_text) == TEXT_SIZE + DELTA, hex(len(new_text))

# verify all new encodings decode as intended
md = Cs(CS_ARCH_ARM64, CS_MODE_ARM)
want = ['mov x0, #0', 'mov x1, #0x200000000000', 'mov x2, #3', 'mov x3, #0x1002',
        'mov x4, #-1', 'mov x5, #0', 'mov x16, #0xc5', 'svc #0x80', None, 'mov x20, x0']
got = ['%s %s' % (i.mnemonic, i.op_str) for i in md.disasm(init, ins_vma)]
for i, (g, w) in enumerate(zip(got, want)):
    if w is None:
        assert g in ('b.cs #0x100000604', 'b.hs #0x100000604'), g
    else:
        assert g == w, (i, g, w)

# --- patch io_byte adrp sites inside new_text ---
# io_byte: was bss+0x70010800 = 0x171018800 ; now bss+0x800 = 0x101008800.
# Every `adrp xN, 0x171018000` re-encodes to page 0x101008000 at its shifted pc.
def encode_adrp(reg, pc, target):
    d = (target - (pc & ~0xFFF)) >> 12
    immlo = d & 3; immhi = (d >> 2) & 0x7FFFF
    return 0x90000000 | (immlo << 29) | (immhi << 5) | reg

text = bytearray(new_text)
for off in range(0, len(text), 4):
    w, = struct.unpack_from('<I', text, off)
    if (w & 0x9F000000) == 0x90000000:      # adrp
        pc = TEXT_VMA + off
        immlo = (w >> 29) & 3; immhi = (w >> 5) & 0x7FFFF
        old_tgt = (pc & ~0xFFF) + (((immhi << 2) | immlo) << 12)
        if old_tgt == 0x171018000:
            reg = w & 31
            text[off:off + 4] = struct.pack('<I', encode_adrp(reg, pc, 0x101008000))
            print('adrp site vma %#x reg x%d retargeted' % (pc, reg))
new_text = bytes(text)

# --- assemble the file ---
out = bytearray()
out += old[:TEXT_OFF] + new_text + old[STUBS_OFF:STUBS_OFF + 0x18]
out += b'\0' * (0x4000 - len(out))
out += old[0x4000:0x1008000]                # __DATA_CONST __got + __DATA __tape raw

# linkedit: fixups, trie, funcstarts unchanged in place; rebuild symtab onward
out += old[0x1008000:0x10080a8]             # chained fixups + exports trie
funcstarts = old[0x10080a8:0x10080a8 + 0x20]
# regenerate the ULEB128 deltas from the shifted symbol values: _main stays
# 0x490; every text label after the insertion point sits DELTA bytes higher.
text_syms = [0x490]
for i in range(22):
    e = old[0x10080c8 + i * 16:0x10080c8 + i * 16 + 16]
    v, = struct.unpack_from('<Q', e, 8)
    text_syms.append(v + DELTA)
fs = bytearray()
fs_prev = text_syms[0]
for v in text_syms:
    d = v - fs_prev if v != text_syms[0] else v
    fs_prev = v
    while d >= 0x80:
        fs.append(0x80 | (d & 0x7f)); d >>= 7
    fs.append(d)
fs = fs.ljust(0x20, b'\x00')
assert len(fs) == 0x20
out += bytes(fs)

# symtab: copy nlist array minus mem (entry 23), patching shifted values
nlist = bytearray()
names = []
for i in range(30):
    e = bytearray(old[0x10080c8 + i * 16:0x10080c8 + i * 16 + 16])
    n_strx, n_type, n_sect, n_desc, n_value = struct.unpack('<IBBHQ', e)
    if i == 23:
        continue                            # mem
    if n_sect == 1 and n_value >= 0x1000004b0:
        n_value += DELTA
        struct.pack_into('<Q', e, 8, n_value)
    if i == 24:                             # io_byte -> bss+0x800
        struct.pack_into('<Q', e, 8, 0x101008800)
    nlist += e
    names.append(i)
nlist = bytes(nlist)

# strtab: old order minus 'mem' (clang emits names in definition order;
# the table leads with ' \0').
old_names = old[0x10082b8:0x10082b8 + 0xd0].split(b'\x00')
new_names = [n for n in old_names if n and n != b'mem']
newstr = b''
strx = {}
for n in new_names:
    strx[n.decode()] = len(newstr)
    newstr += n + b'\x00'
assert len(newstr) <= 0xcc, hex(len(newstr))
newstr = newstr.ljust(0xcc, b'\x00')

# nlist order kept; patch strx per name mapping (old index -> name)
idx2name = {0:'h_imm',1:'next',2:'h_mov',3:'h_add',4:'h_sub',5:'h_mul',6:'h_div',
            7:'h_mod',8:'h_loadb',9:'h_storeb',10:'h_load',11:'h_store',12:'h_jmp',
            13:'h_jz',14:'h_jnz',15:'h_jlt',16:'h_jeq',17:'h_read',18:'h_write',
            19:'h_call',20:'h_ret',21:'h_halt',22:'vregs',24:'io_byte',
            25:'__mh_execute_header',26:'_main',27:'_tape',28:'_read',29:'_write'}
fixed = bytearray(nlist)
for ni, oi in enumerate(names):
    struct.pack_into('<I', fixed, ni * 16, strx[idx2name[oi]])
nlist = bytes(fixed)

out += nlist                              # at fileoff 0x10080c8
indirect = struct.pack('<4I', 0x1b, 0x1c, 0x1b, 0x1c)   # undef indexes -1
out += indirect                           # at 0x1008298
out += newstr                             # at 0x10082a8
SIG_OFF = (0x10082a8 + len(newstr) + 15) & ~15
out += b'\0' * (SIG_OFF - len(out))
out += old[0x1008390:0x1008390 + 0x201a0]   # signature blob, stale until re-sign

# --- header patches ---
off = 32
for i in range(17):
    cmd, cs = struct.unpack_from('<II', out, off)
    if cmd == 0x19:
        nsects, = struct.unpack_from('<I', out, off + 64)
        soff = off + 72
        for j in range(nsects):
            sname = out[soff:soff + 16].rstrip(b'\0')
            if sname == b'__text':
                struct.pack_into('<Q', out, soff + 40, len(new_text))          # size
            elif sname == b'__stubs':
                struct.pack_into('<Q', out, soff + 32, 0x1000008e4 + DELTA)    # addr
                struct.pack_into('<I', out, soff + 48, 0x8e4 + DELTA)          # offset
            elif sname == b'__bss':
                struct.pack_into('<Q', out, soff + 40, 0x808)                  # size
                struct.pack_into('<I', out, soff + 52, 3)                      # align log2
            soff += 80
        segname = out[off + 8:off + 24].rstrip(b'\0')
        if segname == b'__DATA':
            struct.pack_into('<Q', out, off + 32, 0x10004000)                  # vmsize
        if segname == b'__LINKEDIT':
            struct.pack_into('<Q', out, off + 24, 0x10100C000)                 # vmaddr
            struct.pack_into('<Q', out, off + 48, len(out) - 0x1008000)        # filesize
            struct.pack_into('<Q', out, off + 32, 0x24000)                     # vmsize keep
    elif cmd == 0x2:   # LC_SYMTAB
        struct.pack_into('<IIII', out, off + 8, 0x10080c8, 29,
                         0x10080c8 + 29 * 16 + 16, len(newstr))
    elif cmd == 0xb:   # LC_DYSYMTAB
        struct.pack_into('<IIIIII', out, off + 8, 0, 24, 0x18, 3, 0x1b, 2)
        struct.pack_into('<II', out, off + 8 + 12 * 4, 0x10080c8 + 29 * 16, 4)
    elif cmd == 0x1d:  # LC_CODE_SIGNATURE
        struct.pack_into('<II', out, off + 8, SIG_OFF, 0x201a0)
    off += cs

open(sys.argv[2], 'wb').write(bytes(out))
print('wrote %s (%d bytes, %#x)' % (sys.argv[2], len(out), len(out)))
print('insertion delta', DELTA, 'sig at', hex(SIG_OFF))
