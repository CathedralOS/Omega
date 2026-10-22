0000000100004cd0 <<source_files_to_tokens::lexer::Lexer>::tokenize>:
100004cd0: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
100004cd4: a90167fa    	stp	x26, x25, [sp, #0x10]
100004cd8: a9025ff8    	stp	x24, x23, [sp, #0x20]
100004cdc: a90357f6    	stp	x22, x21, [sp, #0x30]
100004ce0: a9044ff4    	stp	x20, x19, [sp, #0x40]
100004ce4: a9057bfd    	stp	x29, x30, [sp, #0x50]
100004ce8: 910143fd    	add	x29, sp, #0x50
100004cec: d10843ff    	sub	sp, sp, #0x210
100004cf0: aa0003f5    	mov	x21, x0
100004cf4: f90067e8    	str	x8, [sp, #0xc8]
100004cf8: a9400400    	ldp	x0, x1, [x0]
100004cfc: d342fc28    	lsr	x8, x1, #2
100004d00: 52800209    	mov	w9, #0x10               ; =16
100004d04: f100411f    	cmp	x8, #0x10
100004d08: 9a898102    	csel	x2, x8, x9, hi
100004d0c: 9103c3e8    	add	x8, sp, #0xf0
100004d10: 94001867    	bl	0x10000aeac <<tokens::token_stream::TokenStream>::with_capacity>
100004d14: d10283a8    	sub	x8, x29, #0xa0
100004d18: 91002109    	add	x9, x8, #0x8
100004d1c: 91004108    	add	x8, x8, #0x10
100004d20: a90927e8    	stp	x8, x9, [sp, #0x90]
100004d24: 12800039    	mov	w25, #-0x2              ; =-2
100004d28: 52800073    	mov	w19, #0x3               ; =3
100004d2c: 14000009    	b	0x100004d50 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x80>
100004d30: f9408be8    	ldr	x8, [sp, #0x110]
100004d34: 8b1a1508    	add	x8, x8, x26, lsl #5
100004d38: a9005d1f    	stp	xzr, x23, [x8]
100004d3c: f900091b    	str	x27, [x8, #0x10]
100004d40: b9001916    	str	w22, [x8, #0x18]
100004d44: 91000748    	add	x8, x26, #0x1
100004d48: f9008fe8    	str	x8, [sp, #0x118]
100004d4c: aa1c03f4    	mov	x20, x28
100004d50: f94016bc    	ldr	x28, [x21, #0x28]
100004d54: b94032a1    	ldr	w1, [x21, #0x30]
100004d58: b90032b9    	str	w25, [x21, #0x30]
100004d5c: 3100083f    	cmn	w1, #0x2
100004d60: 54000081    	b.ne	0x100004d70 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa0>
100004d64: 910042a0    	add	x0, x21, #0x10
100004d68: 94000671    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004d6c: aa0003fc    	mov	x28, x0
100004d70: 3100043f    	cmn	w1, #0x1
100004d74: 54000320    	b.eq	0x100004dd8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x108>
100004d78: 7101c83f    	cmp	w1, #0x72
100004d7c: f90073f4    	str	x20, [sp, #0xe0]
100004d80: 54000321    	b.ne	0x100004de4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x114>
100004d84: b100079f    	cmn	x28, #0x1
100004d88: 5400acc0    	b.eq	0x100006320 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1650>
100004d8c: 91000788    	add	x8, x28, #0x1
100004d90: f94006a9    	ldr	x9, [x21, #0x8]
100004d94: eb09011f    	cmp	x8, x9
100004d98: 540002a2    	b.hs	0x100004dec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11c>
100004d9c: f94002aa    	ldr	x10, [x21]
100004da0: 3868694b    	ldrb	w11, [x10, x8]
100004da4: 7100897f    	cmp	w11, #0x22
100004da8: 54001d60    	b.eq	0x100005154 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x484>
100004dac: 71008d7f    	cmp	w11, #0x23
100004db0: 540001e1    	b.ne	0x100004dec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11c>
100004db4: 3868694b    	ldrb	w11, [x10, x8]
100004db8: 71008d7f    	cmp	w11, #0x23
100004dbc: 54002c21    	b.ne	0x100005340 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x670>
100004dc0: 91000508    	add	x8, x8, #0x1
100004dc4: eb08013f    	cmp	x9, x8
100004dc8: 54ffff61    	b.ne	0x100004db4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe4>
100004dcc: d2800017    	mov	x23, #0x0               ; =0
100004dd0: aa0903e8    	mov	x8, x9
100004dd4: 1400015d    	b	0x100005348 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x678>
100004dd8: b2401ed6    	orr	x22, x22, #0xff
100004ddc: 92800008    	mov	x8, #-0x1               ; =-1
100004de0: 14000078    	b	0x100004fc0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2f0>
100004de4: 52800057    	mov	w23, #0x2               ; =2
100004de8: 14000002    	b	0x100004df0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x120>
100004dec: d2800017    	mov	x23, #0x0               ; =0
100004df0: 7100bc3f    	cmp	w1, #0x2f
100004df4: 54000768    	b.hi	0x100004ee0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x210>
100004df8: 2a0103e8    	mov	w8, w1
100004dfc: d284c009    	mov	x9, #0x2600             ; =9728
100004e00: f2c00029    	movk	x9, #0x1, lsl #32
100004e04: 9ac82529    	lsr	x9, x9, x8
100004e08: 36000169    	tbz	w9, #0x0, 0x100004e34 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x164>
100004e0c: 7102003f    	cmp	w1, #0x80
100004e10: 54000082    	b.hs	0x100004e20 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x150>
100004e14: 52800028    	mov	w8, #0x1                ; =1
100004e18: f94073f4    	ldr	x20, [sp, #0xe0]
100004e1c: 14000042    	b	0x100004f24 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x254>
100004e20: 7120003f    	cmp	w1, #0x800
100004e24: f94073f4    	ldr	x20, [sp, #0xe0]
100004e28: 540007a2    	b.hs	0x100004f1c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x24c>
100004e2c: 52800048    	mov	w8, #0x2                ; =2
100004e30: 1400003d    	b	0x100004f24 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x254>
100004e34: f100bd1f    	cmp	x8, #0x2f
100004e38: 54000541    	b.ne	0x100004ee0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x210>
100004e3c: b94032a8    	ldr	w8, [x21, #0x30]
100004e40: 3100091f    	cmn	w8, #0x2
100004e44: 54000101    	b.ne	0x100004e64 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x194>
100004e48: 910042a0    	add	x0, x21, #0x10
100004e4c: aa0103fa    	mov	x26, x1
100004e50: 94000637    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004e54: aa0103e8    	mov	x8, x1
100004e58: aa1a03e1    	mov	x1, x26
100004e5c: f90016a0    	str	x0, [x21, #0x28]
100004e60: b90032a8    	str	w8, [x21, #0x30]
100004e64: b94032a8    	ldr	w8, [x21, #0x30]
100004e68: 7100a91f    	cmp	w8, #0x2a
100004e6c: 540017e0    	b.eq	0x100005168 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x498>
100004e70: 7100bd1f    	cmp	w8, #0x2f
100004e74: 54000361    	b.ne	0x100004ee0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x210>
100004e78: b100079f    	cmn	x28, #0x1
100004e7c: 5400a6a0    	b.eq	0x100006350 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1680>
100004e80: b90032b9    	str	w25, [x21, #0x30]
100004e84: b1000b9f    	cmn	x28, #0x2
100004e88: f94073f4    	ldr	x20, [sp, #0xe0]
100004e8c: 5400a5c0    	b.eq	0x100006344 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1674>
100004e90: 91000b9b    	add	x27, x28, #0x2
100004e94: b94032a1    	ldr	w1, [x21, #0x30]
100004e98: b90032b9    	str	w25, [x21, #0x30]
100004e9c: 3100083f    	cmn	w1, #0x2
100004ea0: 54000061    	b.ne	0x100004eac <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1dc>
100004ea4: 910042a0    	add	x0, x21, #0x10
100004ea8: 94000621    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004eac: 3100043f    	cmn	w1, #0x1
100004eb0: 7a4a1824    	ccmp	w1, #0xa, #0x4, ne
100004eb4: 54001f80    	b.eq	0x1000052a4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x5d4>
100004eb8: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100004ebc: 9a933668    	cinc	x8, x19, hs
100004ec0: 7120003f    	cmp	w1, #0x800
100004ec4: 52800049    	mov	w9, #0x2                ; =2
100004ec8: 9a883128    	csel	x8, x9, x8, lo
100004ecc: 7102003f    	cmp	w1, #0x80
100004ed0: 9a9f2508    	csinc	x8, x8, xzr, hs
100004ed4: ab1b011b    	adds	x27, x8, x27
100004ed8: 54fffde3    	b.lo	0x100004e94 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1c4>
100004edc: 1400050b    	b	0x100006308 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1638>
100004ee0: 37001057    	tbnz	w23, #0x0, 0x1000050e8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x418>
100004ee4: 529ffbe8    	mov	w8, #0xffdf             ; =65503
100004ee8: 72a003e8    	movk	w8, #0x1f, lsl #16
100004eec: 0a080028    	and	w8, w1, w8
100004ef0: 51010508    	sub	w8, w8, #0x41
100004ef4: 7100691f    	cmp	w8, #0x1a
100004ef8: 52800be8    	mov	w8, #0x5f               ; =95
100004efc: 7a482024    	ccmp	w1, w8, #0x4, hs
100004f00: 54001921    	b.ne	0x100005224 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x554>
100004f04: 7102003f    	cmp	w1, #0x80
100004f08: 900002f4    	adrp	x20, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100004f0c: 91278294    	add	x20, x20, #0x9e0
100004f10: 54002242    	b.hs	0x100005358 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x688>
100004f14: 52800028    	mov	w8, #0x1                ; =1
100004f18: 14000214    	b	0x100005768 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa98>
100004f1c: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100004f20: 9a933668    	cinc	x8, x19, hs
100004f24: ab1c011b    	adds	x27, x8, x28
100004f28: 54009f62    	b.hs	0x100006314 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1644>
100004f2c: b94032a8    	ldr	w8, [x21, #0x30]
100004f30: 3100091f    	cmn	w8, #0x2
100004f34: 540000a1    	b.ne	0x100004f48 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x278>
100004f38: 910042a0    	add	x0, x21, #0x10
100004f3c: 940005fc    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004f40: f90016a0    	str	x0, [x21, #0x28]
100004f44: b90032a1    	str	w1, [x21, #0x30]
100004f48: b94032a8    	ldr	w8, [x21, #0x30]
100004f4c: 7100811f    	cmp	w8, #0x20
100004f50: 54000228    	b.hi	0x100004f94 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2c4>
100004f54: d284c009    	mov	x9, #0x2600             ; =9728
100004f58: f2c00029    	movk	x9, #0x1, lsl #32
100004f5c: 9ac82529    	lsr	x9, x9, x8
100004f60: 360001a9    	tbz	w9, #0x0, 0x100004f94 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2c4>
100004f64: f94016a9    	ldr	x9, [x21, #0x28]
100004f68: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
100004f6c: 9a93366a    	cinc	x10, x19, hs
100004f70: 7120011f    	cmp	w8, #0x800
100004f74: 5280004b    	mov	w11, #0x2               ; =2
100004f78: 9a8a316a    	csel	x10, x11, x10, lo
100004f7c: 7102011f    	cmp	w8, #0x80
100004f80: 9a9f2548    	csinc	x8, x10, xzr, hs
100004f84: ab09011b    	adds	x27, x8, x9
100004f88: 540099a2    	b.hs	0x1000062bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15ec>
100004f8c: b90032b9    	str	w25, [x21, #0x30]
100004f90: 17ffffe7    	b	0x100004f2c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x25c>
100004f94: 52800008    	mov	w8, #0x0                ; =0
100004f98: 5280000a    	mov	w10, #0x0               ; =0
100004f9c: d2800009    	mov	x9, #0x0                ; =0
100004fa0: 528000eb    	mov	w11, #0x7               ; =7
100004fa4: 12181d4a    	and	w10, w10, #0xff00
100004fa8: 2a080148    	orr	w8, w10, w8
100004fac: 12001d6a    	and	w10, w11, #0xff
100004fb0: 2a0a0108    	orr	w8, w8, w10
100004fb4: aa098116    	orr	x22, x8, x9, lsl #32
100004fb8: 92800008    	mov	x8, #-0x1               ; =-1
100004fbc: aa1c03f7    	mov	x23, x28
100004fc0: 12001ec9    	and	w9, w22, #0xff
100004fc4: b100051f    	cmn	x8, #0x1
100004fc8: f94077ea    	ldr	x10, [sp, #0xe8]
100004fcc: f9406feb    	ldr	x11, [sp, #0xd8]
100004fd0: 9a8b014a    	csel	x10, x10, x11, eq
100004fd4: f90077ea    	str	x10, [sp, #0xe8]
100004fd8: 54008e01    	b.ne	0x100006198 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x14c8>
100004fdc: 7103fd3f    	cmp	w9, #0xff
100004fe0: 54008fc0    	b.eq	0x1000061d8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1508>
100004fe4: 7100113f    	cmp	w9, #0x4
100004fe8: 54000101    	b.ne	0x100005008 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x338>
100004fec: a94006a0    	ldp	x0, x1, [x21]
100004ff0: eb01037f    	cmp	x27, x1
100004ff4: fa579360    	ccmp	x27, x23, #0x0, ls
100004ff8: 540001a2    	b.hs	0x10000502c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x35c>
100004ffc: d2800009    	mov	x9, #0x0                ; =0
100005000: b5000389    	cbnz	x9, 0x100005070 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x3a0>
100005004: 1400048c    	b	0x100006234 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1564>
100005008: aa1403fc    	mov	x28, x20
10000500c: f9408ffa    	ldr	x26, [sp, #0x118]
100005010: f94087e8    	ldr	x8, [sp, #0x108]
100005014: eb08035f    	cmp	x26, x8
100005018: 54ffe8c1    	b.ne	0x100004d30 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x60>
10000501c: 9103c3e8    	add	x8, sp, #0xf0
100005020: 91006100    	add	x0, x8, #0x18
100005024: 94000671    	bl	0x1000069e8 <<alloc::raw_vec::RawVec<tokens::token_stream::token::Token>>::grow_one>
100005028: 17ffff42    	b	0x100004d30 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x60>
10000502c: eb0102ff    	cmp	x23, x1
100005030: 54000140    	b.eq	0x100005058 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x388>
100005034: b4000097    	cbz	x23, 0x100005044 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x374>
100005038: 38f76808    	ldrsb	w8, [x0, x23]
10000503c: 3101051f    	cmn	w8, #0x41
100005040: 54fffded    	b.le	0x100004ffc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x32c>
100005044: eb01037f    	cmp	x27, x1
100005048: 54000080    	b.eq	0x100005058 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x388>
10000504c: 38fb6808    	ldrsb	w8, [x0, x27]
100005050: 3101011f    	cmn	w8, #0x40
100005054: 54fffd4b    	b.lt	0x100004ffc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x32c>
100005058: eb170368    	subs	x8, x27, x23
10000505c: 54008fc3    	b.lo	0x100006254 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1584>
100005060: eb01037f    	cmp	x27, x1
100005064: 54008f88    	b.hi	0x100006254 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1584>
100005068: 8b170009    	add	x9, x0, x23
10000506c: b4008e49    	cbz	x9, 0x100006234 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1564>
100005070: aa1703f8    	mov	x24, x23
100005074: aa1403fc    	mov	x28, x20
100005078: f940dff4    	ldr	x20, [sp, #0x1b8]
10000507c: 9103c3ea    	add	x10, sp, #0xf0
100005080: 9102e142    	add	x2, x10, #0xb8
100005084: aa0903e0    	mov	x0, x9
100005088: aa0803e1    	mov	x1, x8
10000508c: 9400067b    	bl	0x100006a78 <source_files_to_tokens::lexer::strings::decode::decode_string_literal>
100005090: f940dfe2    	ldr	x2, [sp, #0x1b8]
100005094: 9103c3e8    	add	x8, sp, #0xf0
100005098: 9100c100    	add	x0, x8, #0x30
10000509c: aa1403e1    	mov	x1, x20
1000050a0: 94000743    	bl	0x100006dac <<arena::arena::Arena<tokens::token_stream::DecodedRange>>::append>
1000050a4: aa0003f4    	mov	x20, x0
1000050a8: aa0103f7    	mov	x23, x1
1000050ac: f9408ffa    	ldr	x26, [sp, #0x118]
1000050b0: f94087e8    	ldr	x8, [sp, #0x108]
1000050b4: eb08035f    	cmp	x26, x8
1000050b8: 54000081    	b.ne	0x1000050c8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x3f8>
1000050bc: 9103c3e8    	add	x8, sp, #0xf0
1000050c0: 91006100    	add	x0, x8, #0x18
1000050c4: 94000649    	bl	0x1000069e8 <<alloc::raw_vec::RawVec<tokens::token_stream::token::Token>>::grow_one>
1000050c8: f9408be8    	ldr	x8, [sp, #0x110]
1000050cc: 8b1a1508    	add	x8, x8, x26, lsl #5
1000050d0: 29005d14    	stp	w20, w23, [x8]
1000050d4: aa1803f7    	mov	x23, x24
1000050d8: a900ed18    	stp	x24, x27, [x8, #0x8]
1000050dc: 52800089    	mov	w9, #0x4                ; =4
1000050e0: 39006109    	strb	w9, [x8, #0x18]
1000050e4: 17ffff18    	b	0x100004d44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x74>
1000050e8: d10283a8    	sub	x8, x29, #0xa0
1000050ec: 52800600    	mov	w0, #0x30               ; =48
1000050f0: 52800001    	mov	w1, #0x0                ; =0
1000050f4: 52800022    	mov	w2, #0x1                ; =1
1000050f8: 52800023    	mov	w3, #0x1                ; =1
1000050fc: 9400abfc    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005100: b85603a8    	ldur	w8, [x29, #-0xa0]
100005104: 37008d28    	tbnz	w8, #0x0, 0x1000062a8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
100005108: f85683a8    	ldur	x8, [x29, #-0x98]
10000510c: f100bd1f    	cmp	x8, #0x2f
100005110: 54008b09    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
100005114: f85703b7    	ldur	x23, [x29, #-0x90]
100005118: f000020a    	adrp	x10, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
10000511c: 911b294a    	add	x10, x10, #0x6ca
100005120: eb0a02e9    	subs	x9, x23, x10
100005124: da898529    	cneg	x9, x9, ls
100005128: f100bd3f    	cmp	x9, #0x2f
10000512c: 54008b09    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005130: ad400540    	ldp	q0, q1, [x10]
100005134: ad0006e0    	stp	q0, q1, [x23]
100005138: 3dc00940    	ldr	q0, [x10, #0x20]
10000513c: 3d800ae0    	str	q0, [x23, #0x20]
100005140: f9006ff4    	str	x20, [sp, #0xd8]
100005144: aa1c03f6    	mov	x22, x28
100005148: 5280061b    	mov	w27, #0x30              ; =48
10000514c: f94073f4    	ldr	x20, [sp, #0xe0]
100005150: 17ffff9c    	b	0x100004fc0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2f0>
100005154: 91000b94    	add	x20, x28, #0x2
100005158: eb08029f    	cmp	x20, x8
10000515c: 54008ee3    	b.lo	0x100006338 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1668>
100005160: 52800037    	mov	w23, #0x1               ; =1
100005164: 17ffff23    	b	0x100004df0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x120>
100005168: b90032b9    	str	w25, [x21, #0x30]
10000516c: 5280003a    	mov	w26, #0x1               ; =1
100005170: 14000003    	b	0x10000517c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4ac>
100005174: 7100bd1f    	cmp	w8, #0x2f
100005178: 540003e0    	b.eq	0x1000051f4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x524>
10000517c: f94016b4    	ldr	x20, [x21, #0x28]
100005180: b94032b7    	ldr	w23, [x21, #0x30]
100005184: b90032b9    	str	w25, [x21, #0x30]
100005188: 31000aff    	cmn	w23, #0x2
10000518c: 540000a1    	b.ne	0x1000051a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4d0>
100005190: 910042a0    	add	x0, x21, #0x10
100005194: 94000566    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005198: aa0003f4    	mov	x20, x0
10000519c: aa0103f7    	mov	x23, x1
1000051a0: 310006ff    	cmn	w23, #0x1
1000051a4: 540008a0    	b.eq	0x1000052b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x5e8>
1000051a8: b94032a8    	ldr	w8, [x21, #0x30]
1000051ac: 3100091f    	cmn	w8, #0x2
1000051b0: 540000a1    	b.ne	0x1000051c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4f4>
1000051b4: 910042a0    	add	x0, x21, #0x10
1000051b8: 9400055d    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000051bc: f90016a0    	str	x0, [x21, #0x28]
1000051c0: b90032a1    	str	w1, [x21, #0x30]
1000051c4: b94032a8    	ldr	w8, [x21, #0x30]
1000051c8: 7100aaff    	cmp	w23, #0x2a
1000051cc: 54fffd40    	b.eq	0x100005174 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4a4>
1000051d0: 7100beff    	cmp	w23, #0x2f
1000051d4: 54fffd41    	b.ne	0x10000517c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4ac>
1000051d8: 7100a91f    	cmp	w8, #0x2a
1000051dc: 54fffd01    	b.ne	0x10000517c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4ac>
1000051e0: b90032b9    	str	w25, [x21, #0x30]
1000051e4: b100075f    	cmn	x26, #0x1
1000051e8: 54008c00    	b.eq	0x100006368 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1698>
1000051ec: 9100075a    	add	x26, x26, #0x1
1000051f0: 17ffffe3    	b	0x10000517c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4ac>
1000051f4: b90032b9    	str	w25, [x21, #0x30]
1000051f8: f100075a    	subs	x26, x26, #0x1
1000051fc: 54fffc01    	b.ne	0x10000517c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4ac>
100005200: b1000e9f    	cmn	x20, #0x3
100005204: 540090c8    	b.hi	0x10000641c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x174c>
100005208: 91000a9b    	add	x27, x20, #0x2
10000520c: d2802108    	mov	x8, #0x108              ; =264
100005210: f94057e9    	ldr	x9, [sp, #0xa8]
100005214: b3407d09    	bfxil	x9, x8, #0, #32
100005218: f90057e9    	str	x9, [sp, #0xa8]
10000521c: 92800008    	mov	x8, #-0x1               ; =-1
100005220: 14000041    	b	0x100005324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x654>
100005224: 5100c028    	sub	w8, w1, #0x30
100005228: 7100291f    	cmp	w8, #0xa
10000522c: 540009e2    	b.hs	0x100005368 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x698>
100005230: 7100b83f    	cmp	w1, #0x2e
100005234: 540028e1    	b.ne	0x100005750 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa80>
100005238: b100079f    	cmn	x28, #0x1
10000523c: 540089c0    	b.eq	0x100006374 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16a4>
100005240: 91000788    	add	x8, x28, #0x1
100005244: f81603a8    	stur	x8, [x29, #-0xa0]
100005248: d10283a1    	sub	x1, x29, #0xa0
10000524c: aa1503e0    	mov	x0, x21
100005250: 52800142    	mov	w2, #0xa                ; =10
100005254: 940004f5    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005258: b94032a8    	ldr	w8, [x21, #0x30]
10000525c: 3100091f    	cmn	w8, #0x2
100005260: 540000a1    	b.ne	0x100005274 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x5a4>
100005264: 910042a0    	add	x0, x21, #0x10
100005268: 94000531    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
10000526c: f90016a0    	str	x0, [x21, #0x28]
100005270: b90032a1    	str	w1, [x21, #0x30]
100005274: b94032a9    	ldr	w9, [x21, #0x30]
100005278: 321b0128    	orr	w8, w9, #0x20
10000527c: 7101951f    	cmp	w8, #0x65
100005280: 540035e1    	b.ne	0x10000593c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc6c>
100005284: f94016a8    	ldr	x8, [x21, #0x28]
100005288: b90032b9    	str	w25, [x21, #0x30]
10000528c: 3100053f    	cmn	w9, #0x1
100005290: 54005700    	b.eq	0x100005d70 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10a0>
100005294: 7102013f    	cmp	w9, #0x80
100005298: 54004522    	b.hs	0x100005b3c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe6c>
10000529c: 52800029    	mov	w9, #0x1                ; =1
1000052a0: 140002b1    	b	0x100005d64 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1094>
1000052a4: 52800008    	mov	w8, #0x0                ; =0
1000052a8: 5280000a    	mov	w10, #0x0               ; =0
1000052ac: d2800009    	mov	x9, #0x0                ; =0
1000052b0: 5280010b    	mov	w11, #0x8               ; =8
1000052b4: 17ffff3c    	b	0x100004fa4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2d4>
1000052b8: f94006a8    	ldr	x8, [x21, #0x8]
1000052bc: f90043e8    	str	x8, [sp, #0x80]
1000052c0: d10283a8    	sub	x8, x29, #0xa0
1000052c4: 52800340    	mov	w0, #0x1a               ; =26
1000052c8: 52800001    	mov	w1, #0x0                ; =0
1000052cc: 52800022    	mov	w2, #0x1                ; =1
1000052d0: 52800023    	mov	w3, #0x1                ; =1
1000052d4: 9400ab86    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
1000052d8: b85603a8    	ldur	w8, [x29, #-0xa0]
1000052dc: 37007e68    	tbnz	w8, #0x0, 0x1000062a8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
1000052e0: f85683a8    	ldur	x8, [x29, #-0x98]
1000052e4: f100651f    	cmp	x8, #0x19
1000052e8: 54007c49    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
1000052ec: f85703a9    	ldur	x9, [x29, #-0x90]
1000052f0: f000020b    	adrp	x11, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
1000052f4: 9107816b    	add	x11, x11, #0x1e0
1000052f8: eb0b012a    	subs	x10, x9, x11
1000052fc: da8a854a    	cneg	x10, x10, ls
100005300: f100655f    	cmp	x10, #0x19
100005304: 54007c49    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005308: 3dc00160    	ldr	q0, [x11]
10000530c: 3d800120    	str	q0, [x9]
100005310: 3cc0a160    	ldur	q0, [x11, #0xa]
100005314: 3c80a120    	stur	q0, [x9, #0xa]
100005318: f90057fc    	str	x28, [sp, #0xa8]
10000531c: 5280035b    	mov	w27, #0x1a              ; =26
100005320: aa0903fc    	mov	x28, x9
100005324: b100051f    	cmn	x8, #0x1
100005328: f94073f4    	ldr	x20, [sp, #0xe0]
10000532c: 54002080    	b.eq	0x10000573c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa6c>
100005330: f94043e9    	ldr	x9, [sp, #0x80]
100005334: f9006fe9    	str	x9, [sp, #0xd8]
100005338: f94057f6    	ldr	x22, [sp, #0xa8]
10000533c: 17ffff20    	b	0x100004fbc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
100005340: 7100897f    	cmp	w11, #0x22
100005344: 1a9f17f7    	cset	w23, eq
100005348: b100051f    	cmn	x8, #0x1
10000534c: 54007f00    	b.eq	0x10000632c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x165c>
100005350: 91000514    	add	x20, x8, #0x1
100005354: 17fffea7    	b	0x100004df0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x120>
100005358: 7120003f    	cmp	w1, #0x800
10000535c: 54002022    	b.hs	0x100005760 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa90>
100005360: 52800048    	mov	w8, #0x2                ; =2
100005364: 14000101    	b	0x100005768 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa98>
100005368: 7100883f    	cmp	w1, #0x22
10000536c: 54000960    	b.eq	0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
100005370: 7100b83f    	cmp	w1, #0x2e
100005374: 54000221    	b.ne	0x1000053b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x6e8>
100005378: b94032a8    	ldr	w8, [x21, #0x30]
10000537c: 3100091f    	cmn	w8, #0x2
100005380: 54000101    	b.ne	0x1000053a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x6d0>
100005384: 910042a0    	add	x0, x21, #0x10
100005388: aa0103f4    	mov	x20, x1
10000538c: 940004e8    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005390: aa0103e8    	mov	x8, x1
100005394: aa1403e1    	mov	x1, x20
100005398: f90016a0    	str	x0, [x21, #0x28]
10000539c: b90032a8    	str	w8, [x21, #0x30]
1000053a0: b94032a8    	ldr	w8, [x21, #0x30]
1000053a4: 5100c108    	sub	w8, w8, #0x30
1000053a8: 7100291f    	cmp	w8, #0xa
1000053ac: 54fff423    	b.lo	0x100005230 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x560>
1000053b0: 7100883f    	cmp	w1, #0x22
1000053b4: 54000720    	b.eq	0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
1000053b8: 7102003f    	cmp	w1, #0x80
1000053bc: 540027c2    	b.hs	0x1000058b4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xbe4>
1000053c0: f94006a8    	ldr	x8, [x21, #0x8]
1000053c4: f9005bfc    	str	x28, [sp, #0xb0]
1000053c8: f9003fe8    	str	x8, [sp, #0x78]
1000053cc: eb1c0108    	subs	x8, x8, x28
1000053d0: f9003be8    	str	x8, [sp, #0x70]
1000053d4: 54007863    	b.lo	0x1000062e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1610>
1000053d8: f94002aa    	ldr	x10, [x21]
1000053dc: 1a9f17e8    	cset	w8, eq
1000053e0: b900a7e8    	str	w8, [sp, #0xa4]
1000053e4: f00002dc    	adrp	x28, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000053e8: 912e839c    	add	x28, x28, #0xba0
1000053ec: 52807b14    	mov	w20, #0x3d8             ; =984
1000053f0: a97f5f82    	ldp	x2, x23, [x28, #-0x10]
1000053f4: b940a7e8    	ldr	w8, [sp, #0xa4]
1000053f8: f9400389    	ldr	x9, [x28]
1000053fc: f9006be9    	str	x9, [sp, #0xd0]
100005400: b4000157    	cbz	x23, 0x100005428 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x758>
100005404: f9405be8    	ldr	x8, [sp, #0xb0]
100005408: f9403fe9    	ldr	x9, [sp, #0x78]
10000540c: eb08013f    	cmp	x9, x8
100005410: 54000200    	b.eq	0x100005450 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x780>
100005414: 39400048    	ldrb	w8, [x2]
100005418: f9405be9    	ldr	x9, [sp, #0xb0]
10000541c: 38696949    	ldrb	w9, [x10, x9]
100005420: 6b09011f    	cmp	w8, w9
100005424: 1a9f17e8    	cset	w8, eq
100005428: 34000148    	cbz	w8, 0x100005450 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x780>
10000542c: f9405be8    	ldr	x8, [sp, #0xb0]
100005430: aa0a03f8    	mov	x24, x10
100005434: 8b080140    	add	x0, x10, x8
100005438: f9403be1    	ldr	x1, [sp, #0x70]
10000543c: aa0203f6    	mov	x22, x2
100005440: aa1703e3    	mov	x3, x23
100005444: 9400a757    	bl	0x10002f1a0 <<[u8]>::starts_with>
100005448: aa1803ea    	mov	x10, x24
10000544c: 37000080    	tbnz	w0, #0x0, 0x10000545c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x78c>
100005450: 52801fe8    	mov	w8, #0xff               ; =255
100005454: f9006be8    	str	x8, [sp, #0xd0]
100005458: a94bdbf7    	ldp	x23, x22, [sp, #0xb8]
10000545c: f9406be9    	ldr	x9, [sp, #0xd0]
100005460: 12001d38    	and	w24, w9, #0xff
100005464: 7103ff1f    	cmp	w24, #0xff
100005468: 9a96037b    	csel	x27, x27, x22, eq
10000546c: 9a97035a    	csel	x26, x26, x23, eq
100005470: 540036e1    	b.ne	0x100005b4c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe7c>
100005474: 9100639c    	add	x28, x28, #0x18
100005478: d1006294    	sub	x20, x20, #0x18
10000547c: a90bdbf7    	stp	x23, x22, [sp, #0xb8]
100005480: b100629f    	cmn	x20, #0x18
100005484: 54fffb61    	b.ne	0x1000053f0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x720>
100005488: 52801ff8    	mov	w24, #0xff              ; =255
10000548c: 140001b2    	b	0x100005b54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe84>
100005490: 7100883f    	cmp	w1, #0x22
100005494: 54004360    	b.eq	0x100005d00 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1030>
100005498: f94016bb    	ldr	x27, [x21, #0x28]
10000549c: b94032a1    	ldr	w1, [x21, #0x30]
1000054a0: b90032b9    	str	w25, [x21, #0x30]
1000054a4: 3100083f    	cmn	w1, #0x2
1000054a8: 54000081    	b.ne	0x1000054b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7e8>
1000054ac: 910042a0    	add	x0, x21, #0x10
1000054b0: 9400049f    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000054b4: aa0003fb    	mov	x27, x0
1000054b8: 7100843f    	cmp	w1, #0x21
1000054bc: 540008ad    	b.le	0x1000055d0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x900>
1000054c0: 7101703f    	cmp	w1, #0x5c
1000054c4: 54fffe61    	b.ne	0x100005490 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c0>
1000054c8: f94016a0    	ldr	x0, [x21, #0x28]
1000054cc: b94032a1    	ldr	w1, [x21, #0x30]
1000054d0: b90032b9    	str	w25, [x21, #0x30]
1000054d4: 3100083f    	cmn	w1, #0x2
1000054d8: 54000061    	b.ne	0x1000054e4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x814>
1000054dc: 910042a0    	add	x0, x21, #0x10
1000054e0: 94000493    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000054e4: 71016c3f    	cmp	w1, #0x5b
1000054e8: 5400082d    	b.le	0x1000055ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x91c>
1000054ec: 51017028    	sub	w8, w1, #0x5c
1000054f0: 7100711f    	cmp	w8, #0x1c
1000054f4: 54006128    	b.hi	0x100006118 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1448>
1000054f8: 52800029    	mov	w9, #0x1                ; =1
1000054fc: 72a02889    	movk	w9, #0x144, lsl #16
100005500: 1ac82529    	lsr	w9, w9, w8
100005504: 3707fca9    	tbnz	w9, #0x0, 0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
100005508: 7100711f    	cmp	w8, #0x1c
10000550c: 54005a01    	b.ne	0x10000604c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x137c>
100005510: f94016bb    	ldr	x27, [x21, #0x28]
100005514: b94032a1    	ldr	w1, [x21, #0x30]
100005518: b90032b9    	str	w25, [x21, #0x30]
10000551c: 3100083f    	cmn	w1, #0x2
100005520: 54000081    	b.ne	0x100005530 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x860>
100005524: 910042a0    	add	x0, x21, #0x10
100005528: 94000481    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
10000552c: aa0003fb    	mov	x27, x0
100005530: 3100043f    	cmn	w1, #0x1
100005534: 54000660    	b.eq	0x100005600 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x930>
100005538: 5100c028    	sub	w8, w1, #0x30
10000553c: 7100291f    	cmp	w8, #0xa
100005540: 1a9f27e8    	cset	w8, lo
100005544: 71011c3f    	cmp	w1, #0x47
100005548: 1a9f27e9    	cset	w9, lo
10000554c: 7101003f    	cmp	w1, #0x40
100005550: 1a888128    	csel	w8, w9, w8, hi
100005554: 71019c3f    	cmp	w1, #0x67
100005558: 1a9f27e9    	cset	w9, lo
10000555c: 7101803f    	cmp	w1, #0x60
100005560: 1a888128    	csel	w8, w9, w8, hi
100005564: 34000848    	cbz	w8, 0x10000566c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x99c>
100005568: f94016bb    	ldr	x27, [x21, #0x28]
10000556c: b94032a1    	ldr	w1, [x21, #0x30]
100005570: b90032b9    	str	w25, [x21, #0x30]
100005574: 3100083f    	cmn	w1, #0x2
100005578: 54000081    	b.ne	0x100005588 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x8b8>
10000557c: 910042a0    	add	x0, x21, #0x10
100005580: 9400046b    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005584: aa0003fb    	mov	x27, x0
100005588: 3100043f    	cmn	w1, #0x1
10000558c: 540003a0    	b.eq	0x100005600 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x930>
100005590: 5100c028    	sub	w8, w1, #0x30
100005594: 7100291f    	cmp	w8, #0xa
100005598: 1a9f27e8    	cset	w8, lo
10000559c: 71011c3f    	cmp	w1, #0x47
1000055a0: 1a9f27e9    	cset	w9, lo
1000055a4: 7101003f    	cmp	w1, #0x40
1000055a8: 1a888128    	csel	w8, w9, w8, hi
1000055ac: 71019c3f    	cmp	w1, #0x67
1000055b0: 1a9f27e9    	cset	w9, lo
1000055b4: 7101803f    	cmp	w1, #0x60
1000055b8: 1a888128    	csel	w8, w9, w8, hi
1000055bc: 36000588    	tbz	w8, #0x0, 0x10000566c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x99c>
1000055c0: 92800008    	mov	x8, #-0x1               ; =-1
1000055c4: a943affb    	ldp	x27, x11, [sp, #0x38]
1000055c8: f9401be9    	ldr	x9, [sp, #0x30]
1000055cc: 1400004c    	b	0x1000056fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa2c>
1000055d0: 3100043f    	cmn	w1, #0x1
1000055d4: 54003580    	b.eq	0x100005c84 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xfb4>
1000055d8: 7100283f    	cmp	w1, #0xa
1000055dc: 54001b40    	b.eq	0x100005944 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc74>
1000055e0: 7100343f    	cmp	w1, #0xd
1000055e4: 54fff5a1    	b.ne	0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
1000055e8: 140000d7    	b	0x100005944 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc74>
1000055ec: 7100883f    	cmp	w1, #0x22
1000055f0: 54fff540    	b.eq	0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
1000055f4: 7100c03f    	cmp	w1, #0x30
1000055f8: 54fff500    	b.eq	0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
1000055fc: 1400028c    	b	0x10000602c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x135c>
100005600: f94006a8    	ldr	x8, [x21, #0x8]
100005604: f90033e8    	str	x8, [sp, #0x60]
100005608: d10283a8    	sub	x8, x29, #0xa0
10000560c: 528002e0    	mov	w0, #0x17               ; =23
100005610: 52800001    	mov	w1, #0x0                ; =0
100005614: 52800022    	mov	w2, #0x1                ; =1
100005618: 52800023    	mov	w3, #0x1                ; =1
10000561c: 9400aab4    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005620: f85603a8    	ldur	x8, [x29, #-0xa0]
100005624: b5006548    	cbnz	x8, 0x1000062cc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15fc>
100005628: f85683a8    	ldur	x8, [x29, #-0x98]
10000562c: f100591f    	cmp	x8, #0x16
100005630: 54006209    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
100005634: f85703ab    	ldur	x11, [x29, #-0x90]
100005638: f000020a    	adrp	x10, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
10000563c: 91084d4a    	add	x10, x10, #0x213
100005640: eb0a0169    	subs	x9, x11, x10
100005644: da898529    	cneg	x9, x9, ls
100005648: f100593f    	cmp	x9, #0x16
10000564c: 54006209    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005650: 3dc00140    	ldr	q0, [x10]
100005654: 3d800160    	str	q0, [x11]
100005658: f840f149    	ldur	x9, [x10, #0xf]
10000565c: f800f169    	stur	x9, [x11, #0xf]
100005660: aa1c03fb    	mov	x27, x28
100005664: 528002e9    	mov	w9, #0x17               ; =23
100005668: 14000025    	b	0x1000056fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa2c>
10000566c: 7102003f    	cmp	w1, #0x80
100005670: 54000062    	b.hs	0x10000567c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x9ac>
100005674: 52800028    	mov	w8, #0x1                ; =1
100005678: 14000007    	b	0x100005694 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x9c4>
10000567c: 7120003f    	cmp	w1, #0x800
100005680: 54000062    	b.hs	0x10000568c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x9bc>
100005684: 52800048    	mov	w8, #0x2                ; =2
100005688: 14000003    	b	0x100005694 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x9c4>
10000568c: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005690: 9a933668    	cinc	x8, x19, hs
100005694: ab1b0108    	adds	x8, x8, x27
100005698: f90033e8    	str	x8, [sp, #0x60]
10000569c: 54006962    	b.hs	0x1000063c8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16f8>
1000056a0: d10283a8    	sub	x8, x29, #0xa0
1000056a4: 52800300    	mov	w0, #0x18               ; =24
1000056a8: 52800001    	mov	w1, #0x0                ; =0
1000056ac: 52800022    	mov	w2, #0x1                ; =1
1000056b0: 52800023    	mov	w3, #0x1                ; =1
1000056b4: 9400aa8e    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
1000056b8: b85603a8    	ldur	w8, [x29, #-0xa0]
1000056bc: 37006088    	tbnz	w8, #0x0, 0x1000062cc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15fc>
1000056c0: f85683a8    	ldur	x8, [x29, #-0x98]
1000056c4: f1005d1f    	cmp	x8, #0x17
1000056c8: 54005d49    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
1000056cc: f85703ab    	ldur	x11, [x29, #-0x90]
1000056d0: f000020a    	adrp	x10, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
1000056d4: 9107ed4a    	add	x10, x10, #0x1fb
1000056d8: eb0a0169    	subs	x9, x11, x10
1000056dc: da898529    	cneg	x9, x9, ls
1000056e0: f1005d3f    	cmp	x9, #0x17
1000056e4: 54005d49    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
1000056e8: 3dc00140    	ldr	q0, [x10]
1000056ec: 3d800160    	str	q0, [x11]
1000056f0: f9400949    	ldr	x9, [x10, #0x10]
1000056f4: f9000969    	str	x9, [x11, #0x10]
1000056f8: 52800309    	mov	w9, #0x18               ; =24
1000056fc: b100051f    	cmn	x8, #0x1
100005700: a903affb    	stp	x27, x11, [sp, #0x38]
100005704: f9001be9    	str	x9, [sp, #0x30]
100005708: 54000080    	b.eq	0x100005718 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa48>
10000570c: a90227eb    	stp	x11, x9, [sp, #0x20]
100005710: f94033e9    	ldr	x9, [sp, #0x60]
100005714: a9016fe9    	stp	x9, x27, [sp, #0x10]
100005718: b100051f    	cmn	x8, #0x1
10000571c: 54ffebe0    	b.eq	0x100005498 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c8>
100005720: f94017ea    	ldr	x10, [sp, #0x28]
100005724: a9412fe9    	ldp	x9, x11, [sp, #0x10]
100005728: a9372faa    	stp	x10, x11, [x29, #-0x90]
10000572c: f81803a9    	stur	x9, [x29, #-0x80]
100005730: f94013e9    	ldr	x9, [sp, #0x20]
100005734: f81683a9    	stur	x9, [x29, #-0x98]
100005738: 14000177    	b	0x100005d14 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1044>
10000573c: f94057eb    	ldr	x11, [sp, #0xa8]
100005740: d360fd69    	lsr	x9, x11, #32
100005744: 12103d68    	and	w8, w11, #0xffff0000
100005748: aa0b03ea    	mov	x10, x11
10000574c: 17fffe16    	b	0x100004fa4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2d4>
100005750: 7102003f    	cmp	w1, #0x80
100005754: 54000a82    	b.hs	0x1000058a4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xbd4>
100005758: 52800028    	mov	w8, #0x1                ; =1
10000575c: 14000080    	b	0x10000595c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc8c>
100005760: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005764: 9a933668    	cinc	x8, x19, hs
100005768: ab1c011b    	adds	x27, x8, x28
10000576c: 54005f82    	b.hs	0x10000635c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x168c>
100005770: b94032a8    	ldr	w8, [x21, #0x30]
100005774: 3100091f    	cmn	w8, #0x2
100005778: 540000a1    	b.ne	0x10000578c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xabc>
10000577c: 910042a0    	add	x0, x21, #0x10
100005780: 940003eb    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005784: f90016a0    	str	x0, [x21, #0x28]
100005788: b90032a1    	str	w1, [x21, #0x30]
10000578c: b94032a9    	ldr	w9, [x21, #0x30]
100005790: f94016a8    	ldr	x8, [x21, #0x28]
100005794: 71017d3f    	cmp	w9, #0x5f
100005798: 540001e0    	b.eq	0x1000057d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb04>
10000579c: 3100053f    	cmn	w9, #0x1
1000057a0: 54000300    	b.eq	0x100005800 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb30>
1000057a4: 5100c12a    	sub	w10, w9, #0x30
1000057a8: 7100295f    	cmp	w10, #0xa
1000057ac: 1a9f27ea    	cset	w10, lo
1000057b0: 71016d3f    	cmp	w9, #0x5b
1000057b4: 1a9f27eb    	cset	w11, lo
1000057b8: 7101013f    	cmp	w9, #0x40
1000057bc: 1a8a816a    	csel	w10, w11, w10, hi
1000057c0: 7101ed3f    	cmp	w9, #0x7b
1000057c4: 1a9f27eb    	cset	w11, lo
1000057c8: 7101813f    	cmp	w9, #0x60
1000057cc: 1a8a816a    	csel	w10, w11, w10, hi
1000057d0: 3400018a    	cbz	w10, 0x100005800 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb30>
1000057d4: 7140413f    	cmp	w9, #0x10, lsl #12      ; =0x10000
1000057d8: 9a93366a    	cinc	x10, x19, hs
1000057dc: 7120013f    	cmp	w9, #0x800
1000057e0: 5280004b    	mov	w11, #0x2               ; =2
1000057e4: 9a8a316a    	csel	x10, x11, x10, lo
1000057e8: 7102013f    	cmp	w9, #0x80
1000057ec: 9a9f2549    	csinc	x9, x10, xzr, hs
1000057f0: ab08013b    	adds	x27, x9, x8
1000057f4: 54005842    	b.hs	0x1000062fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x162c>
1000057f8: b90032b9    	str	w25, [x21, #0x30]
1000057fc: 17ffffdd    	b	0x100005770 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xaa0>
100005800: a94006a0    	ldp	x0, x1, [x21]
100005804: eb01037f    	cmp	x27, x1
100005808: fa5c9360    	ccmp	x27, x28, #0x0, ls
10000580c: 54000082    	b.hs	0x10000581c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb4c>
100005810: d2800009    	mov	x9, #0x0                ; =0
100005814: b5000269    	cbnz	x9, 0x100005860 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb90>
100005818: 1400028a    	b	0x100006240 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1570>
10000581c: eb01039f    	cmp	x28, x1
100005820: 54000140    	b.eq	0x100005848 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb78>
100005824: b400009c    	cbz	x28, 0x100005834 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb64>
100005828: 38fc6808    	ldrsb	w8, [x0, x28]
10000582c: 3101051f    	cmn	w8, #0x41
100005830: 54ffff0d    	b.le	0x100005810 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb40>
100005834: eb01037f    	cmp	x27, x1
100005838: 54000080    	b.eq	0x100005848 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb78>
10000583c: 38fb6808    	ldrsb	w8, [x0, x27]
100005840: 3101011f    	cmn	w8, #0x40
100005844: 54fffe6b    	b.lt	0x100005810 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb40>
100005848: eb1c0368    	subs	x8, x27, x28
10000584c: 54005043    	b.lo	0x100006254 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1584>
100005850: eb01037f    	cmp	x27, x1
100005854: 54005008    	b.hi	0x100006254 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1584>
100005858: 8b1c0009    	add	x9, x0, x28
10000585c: b4004f29    	cbz	x9, 0x100006240 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1570>
100005860: aa0903e0    	mov	x0, x9
100005864: aa0803e1    	mov	x1, x8
100005868: 94001418    	bl	0x10000a8c8 <<tokens::token_stream::token_kind::keyword_kind::KeywordKind>::from_lexeme>
10000586c: 52800008    	mov	w8, #0x0                ; =0
100005870: d2800009    	mov	x9, #0x0                ; =0
100005874: 12001c0a    	and	w10, w0, #0xff
100005878: 528000ab    	mov	w11, #0x5               ; =5
10000587c: 33181c0b    	bfi	w11, w0, #8, #8
100005880: 7103fd5f    	cmp	w10, #0xff
100005884: 1a8b014a    	csel	w10, w10, w11, eq
100005888: 52801f4b    	mov	w11, #0xfa              ; =250
10000588c: 6a2a017f    	bics	wzr, w11, w10
100005890: 5280004b    	mov	w11, #0x2               ; =2
100005894: 1a8a016b    	csel	w11, w11, w10, eq
100005898: 1218196a    	and	w10, w11, #0x7f00
10000589c: f94073f4    	ldr	x20, [sp, #0xe0]
1000058a0: 17fffdc1    	b	0x100004fa4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2d4>
1000058a4: 7120003f    	cmp	w1, #0x800
1000058a8: 54000562    	b.hs	0x100005954 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc84>
1000058ac: 52800048    	mov	w8, #0x2                ; =2
1000058b0: 1400002b    	b	0x10000595c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc8c>
1000058b4: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
1000058b8: 9a933668    	cinc	x8, x19, hs
1000058bc: 7120003f    	cmp	w1, #0x800
1000058c0: 52800049    	mov	w9, #0x2                ; =2
1000058c4: 9a883128    	csel	x8, x9, x8, lo
1000058c8: ab1c0108    	adds	x8, x8, x28
1000058cc: f9006fe8    	str	x8, [sp, #0xd8]
1000058d0: 54005642    	b.hs	0x100006398 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16c8>
1000058d4: d10283a8    	sub	x8, x29, #0xa0
1000058d8: 52800600    	mov	w0, #0x30               ; =48
1000058dc: 52800001    	mov	w1, #0x0                ; =0
1000058e0: 52800022    	mov	w2, #0x1                ; =1
1000058e4: 52800023    	mov	w3, #0x1                ; =1
1000058e8: 9400aa01    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
1000058ec: f85603a8    	ldur	x8, [x29, #-0xa0]
1000058f0: b5004dc8    	cbnz	x8, 0x1000062a8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
1000058f4: f85683a8    	ldur	x8, [x29, #-0x98]
1000058f8: f100bd1f    	cmp	x8, #0x2f
1000058fc: f000020a    	adrp	x10, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100005900: 911b294a    	add	x10, x10, #0x6ca
100005904: f94073f4    	ldr	x20, [sp, #0xe0]
100005908: 54004b49    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
10000590c: f85703b7    	ldur	x23, [x29, #-0x90]
100005910: eb0a02e9    	subs	x9, x23, x10
100005914: da898529    	cneg	x9, x9, ls
100005918: f100bd3f    	cmp	x9, #0x2f
10000591c: 54004b89    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005920: ad400540    	ldp	q0, q1, [x10]
100005924: ad0006e0    	stp	q0, q1, [x23]
100005928: 3dc00940    	ldr	q0, [x10, #0x20]
10000592c: 3d800ae0    	str	q0, [x23, #0x20]
100005930: aa1c03f6    	mov	x22, x28
100005934: 5280061b    	mov	w27, #0x30              ; =48
100005938: 17fffda2    	b	0x100004fc0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2f0>
10000593c: 52800034    	mov	w20, #0x1               ; =1
100005940: 1400016b    	b	0x100005eec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x121c>
100005944: 7102003f    	cmp	w1, #0x80
100005948: 54001d42    	b.hs	0x100005cf0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1020>
10000594c: 52800028    	mov	w8, #0x1                ; =1
100005950: 14000121    	b	0x100005dd4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1104>
100005954: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005958: 9a933668    	cinc	x8, x19, hs
10000595c: ab1c0114    	adds	x20, x8, x28
100005960: 54005102    	b.hs	0x100006380 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16b0>
100005964: f81903b4    	stur	x20, [x29, #-0x70]
100005968: 7100c03f    	cmp	w1, #0x30
10000596c: 540003a1    	b.ne	0x1000059e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd10>
100005970: b94032a8    	ldr	w8, [x21, #0x30]
100005974: 3100091f    	cmn	w8, #0x2
100005978: 540000a1    	b.ne	0x10000598c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcbc>
10000597c: 910042a0    	add	x0, x21, #0x10
100005980: 9400036b    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005984: f90016a0    	str	x0, [x21, #0x28]
100005988: b90032a1    	str	w1, [x21, #0x30]
10000598c: b94032a8    	ldr	w8, [x21, #0x30]
100005990: 51010909    	sub	w9, w8, #0x42
100005994: 7100d93f    	cmp	w9, #0x36
100005998: 54000248    	b.hi	0x1000059e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd10>
10000599c: b20003ea    	mov	x10, #0x100000001       ; =4294967297
1000059a0: 9ac9254a    	lsr	x10, x10, x9
1000059a4: 37001cca    	tbnz	w10, #0x0, 0x100005d3c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x106c>
1000059a8: b21303ea    	mov	x10, #0x200000002000    ; =35184372097024
1000059ac: 9ac9254a    	lsr	x10, x10, x9
1000059b0: 37001b6a    	tbnz	w10, #0x0, 0x100005d1c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x104c>
1000059b4: b20a03ea    	mov	x10, #0x40000000400000  ; =18014398513676288
1000059b8: 9ac92549    	lsr	x9, x10, x9
1000059bc: 36000129    	tbz	w9, #0x0, 0x1000059e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd10>
1000059c0: f94016a9    	ldr	x9, [x21, #0x28]
1000059c4: b90032b9    	str	w25, [x21, #0x30]
1000059c8: 3100051f    	cmn	w8, #0x1
1000059cc: 540037a0    	b.eq	0x1000060c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13f0>
1000059d0: 7102011f    	cmp	w8, #0x80
1000059d4: 54002e42    	b.hs	0x100005f9c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12cc>
1000059d8: 52800028    	mov	w8, #0x1                ; =1
1000059dc: 140001b7    	b	0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13e8>
1000059e0: d101c3a1    	sub	x1, x29, #0x70
1000059e4: aa1503e0    	mov	x0, x21
1000059e8: 52800142    	mov	w2, #0xa                ; =10
1000059ec: 9400030f    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
1000059f0: 52800014    	mov	w20, #0x0               ; =0
1000059f4: 52a00057    	mov	w23, #0x20000           ; =131072
1000059f8: 5280003a    	mov	w26, #0x1               ; =1
1000059fc: 340005ba    	cbz	w26, 0x100005ab0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xde0>
100005a00: b94032a8    	ldr	w8, [x21, #0x30]
100005a04: 3100091f    	cmn	w8, #0x2
100005a08: 540000a1    	b.ne	0x100005a1c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd4c>
100005a0c: 910042a0    	add	x0, x21, #0x10
100005a10: 94000347    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005a14: f90016a0    	str	x0, [x21, #0x28]
100005a18: b90032a1    	str	w1, [x21, #0x30]
100005a1c: b94032a8    	ldr	w8, [x21, #0x30]
100005a20: 7100b91f    	cmp	w8, #0x2e
100005a24: 54000461    	b.ne	0x100005ab0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xde0>
100005a28: 3dc006a0    	ldr	q0, [x21, #0x10]
100005a2c: a9426ea8    	ldp	x8, x27, [x21, #0x20]
100005a30: 3c9603a0    	stur	q0, [x29, #-0xa0]
100005a34: a9376fa8    	stp	x8, x27, [x29, #-0x90]
100005a38: b81803b9    	stur	w25, [x29, #-0x80]
100005a3c: d10283a0    	sub	x0, x29, #0xa0
100005a40: 9400033b    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005a44: 5100c028    	sub	w8, w1, #0x30
100005a48: 7100291f    	cmp	w8, #0xa
100005a4c: 540001a3    	b.lo	0x100005a80 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdb0>
100005a50: 7100b83f    	cmp	w1, #0x2e
100005a54: 540002e0    	b.eq	0x100005ab0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xde0>
100005a58: 3100043f    	cmn	w1, #0x1
100005a5c: 54000120    	b.eq	0x100005a80 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdb0>
100005a60: 529ffbe8    	mov	w8, #0xffdf             ; =65503
100005a64: 72a003e8    	movk	w8, #0x1f, lsl #16
100005a68: 0a080028    	and	w8, w1, w8
100005a6c: 51016d08    	sub	w8, w8, #0x5b
100005a70: 3100691f    	cmn	w8, #0x1a
100005a74: 52800be8    	mov	w8, #0x5f               ; =95
100005a78: 7a483024    	ccmp	w1, w8, #0x4, lo
100005a7c: 540001a0    	b.eq	0x100005ab0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xde0>
100005a80: b90032b9    	str	w25, [x21, #0x30]
100005a84: b100077f    	cmn	x27, #0x1
100005a88: 54004ac0    	b.eq	0x1000063e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1710>
100005a8c: 91000768    	add	x8, x27, #0x1
100005a90: f81903a8    	stur	x8, [x29, #-0x70]
100005a94: d101c3a1    	sub	x1, x29, #0x70
100005a98: aa1503e0    	mov	x0, x21
100005a9c: 52800142    	mov	w2, #0xa                ; =10
100005aa0: 940002e2    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005aa4: 5280003b    	mov	w27, #0x1               ; =1
100005aa8: 3700009a    	tbnz	w26, #0x0, 0x100005ab8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xde8>
100005aac: 14000016    	b	0x100005b04 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe34>
100005ab0: 5280001b    	mov	w27, #0x0               ; =0
100005ab4: 3600029a    	tbz	w26, #0x0, 0x100005b04 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe34>
100005ab8: b94032a8    	ldr	w8, [x21, #0x30]
100005abc: 3100091f    	cmn	w8, #0x2
100005ac0: 540000a1    	b.ne	0x100005ad4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe04>
100005ac4: 910042a0    	add	x0, x21, #0x10
100005ac8: 94000319    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005acc: f90016a0    	str	x0, [x21, #0x28]
100005ad0: b90032a1    	str	w1, [x21, #0x30]
100005ad4: b94032a9    	ldr	w9, [x21, #0x30]
100005ad8: 321b0128    	orr	w8, w9, #0x20
100005adc: 7101951f    	cmp	w8, #0x65
100005ae0: 54000121    	b.ne	0x100005b04 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe34>
100005ae4: f94016a8    	ldr	x8, [x21, #0x28]
100005ae8: b90032b9    	str	w25, [x21, #0x30]
100005aec: 3100053f    	cmn	w9, #0x1
100005af0: 54002280    	b.eq	0x100005f40 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1270>
100005af4: 7102013f    	cmp	w9, #0x80
100005af8: 54001622    	b.hs	0x100005dbc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10ec>
100005afc: 52800029    	mov	w9, #0x1                ; =1
100005b00: 1400010d    	b	0x100005f34 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1264>
100005b04: d101c3a1    	sub	x1, x29, #0x70
100005b08: aa1503e0    	mov	x0, x21
100005b0c: 94000266    	bl	0x1000064a4 <<source_files_to_tokens::lexer::Lexer>::consume_literal_suffix>
100005b10: 3600009b    	tbz	w27, #0x0, 0x100005b20 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe50>
100005b14: 52800008    	mov	w8, #0x0                ; =0
100005b18: 52800029    	mov	w9, #0x1                ; =1
100005b1c: 14000139    	b	0x100006000 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1330>
100005b20: 7100001f    	cmp	w0, #0x0
100005b24: 52802008    	mov	w8, #0x100              ; =256
100005b28: 1a9f1108    	csel	w8, w8, wzr, ne
100005b2c: 2a1402e9    	orr	w9, w23, w20
100005b30: 2a080128    	orr	w8, w9, w8
100005b34: 53185d0a    	lsl	w10, w8, #8
100005b38: 14000137    	b	0x100006014 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1344>
100005b3c: 7120013f    	cmp	w9, #0x800
100005b40: 540010e2    	b.hs	0x100005d5c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x108c>
100005b44: 52800049    	mov	w9, #0x2                ; =2
100005b48: 14000087    	b	0x100005d64 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1094>
100005b4c: f90003fa    	str	x26, [sp]
100005b50: f90047fb    	str	x27, [sp, #0x88]
100005b54: 7103ff1f    	cmp	w24, #0xff
100005b58: f9405bfc    	ldr	x28, [sp, #0xb0]
100005b5c: 54000461    	b.ne	0x100005be8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf18>
100005b60: b100079f    	cmn	x28, #0x1
100005b64: 54004440    	b.eq	0x1000063ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x171c>
100005b68: d10283a8    	sub	x8, x29, #0xa0
100005b6c: 52800600    	mov	w0, #0x30               ; =48
100005b70: 52800001    	mov	w1, #0x0                ; =0
100005b74: 52800022    	mov	w2, #0x1                ; =1
100005b78: 52800023    	mov	w3, #0x1                ; =1
100005b7c: 9400a95c    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005b80: b85603a8    	ldur	w8, [x29, #-0xa0]
100005b84: 37003928    	tbnz	w8, #0x0, 0x1000062a8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
100005b88: f85683a8    	ldur	x8, [x29, #-0x98]
100005b8c: f100bd1f    	cmp	x8, #0x2f
100005b90: f000020b    	adrp	x11, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100005b94: 911b296b    	add	x11, x11, #0x6ca
100005b98: f94073f4    	ldr	x20, [sp, #0xe0]
100005b9c: 540036a9    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
100005ba0: f85703a9    	ldur	x9, [x29, #-0x90]
100005ba4: eb0b012a    	subs	x10, x9, x11
100005ba8: da8a854a    	cneg	x10, x10, ls
100005bac: f100bd5f    	cmp	x10, #0x2f
100005bb0: 540036e9    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005bb4: 9100078a    	add	x10, x28, #0x1
100005bb8: f90007ea    	str	x10, [sp, #0x8]
100005bbc: ad400560    	ldp	q0, q1, [x11]
100005bc0: ad000520    	stp	q0, q1, [x9]
100005bc4: 3dc00960    	ldr	q0, [x11, #0x20]
100005bc8: 3d800920    	str	q0, [x9, #0x20]
100005bcc: f9002bfc    	str	x28, [sp, #0x50]
100005bd0: 5280061b    	mov	w27, #0x30              ; =48
100005bd4: aa0903fc    	mov	x28, x9
100005bd8: b100051f    	cmn	x8, #0x1
100005bdc: 540004c1    	b.ne	0x100005c74 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xfa4>
100005be0: f9402beb    	ldr	x11, [sp, #0x50]
100005be4: 17fffed7    	b	0x100005740 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa70>
100005be8: f94003f7    	ldr	x23, [sp]
100005bec: b7f841f7    	tbnz	x23, #0x3f, 0x100006428 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1758>
100005bf0: 52800034    	mov	w20, #0x1               ; =1
100005bf4: 14000002    	b	0x100005bfc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf2c>
100005bf8: 91000694    	add	x20, x20, #0x1
100005bfc: f10082ff    	cmp	x23, #0x20
100005c00: 540000a2    	b.hs	0x100005c14 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf44>
100005c04: f94047e0    	ldr	x0, [sp, #0x88]
100005c08: aa1703e1    	mov	x1, x23
100005c0c: 9400d040    	bl	0x100039d0c <core::str::count::char_count_general_case>
100005c10: 14000004    	b	0x100005c20 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf50>
100005c14: f94047e0    	ldr	x0, [sp, #0x88]
100005c18: aa1703e1    	mov	x1, x23
100005c1c: 9400ce78    	bl	0x1000395fc <core::str::count::do_count_chars>
100005c20: eb00029f    	cmp	x20, x0
100005c24: 54000102    	b.hs	0x100005c44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf74>
100005c28: b94032a8    	ldr	w8, [x21, #0x30]
100005c2c: b90032b9    	str	w25, [x21, #0x30]
100005c30: 3100091f    	cmn	w8, #0x2
100005c34: 54fffe21    	b.ne	0x100005bf8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf28>
100005c38: 910042a0    	add	x0, x21, #0x10
100005c3c: 940002bc    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005c40: 17ffffee    	b	0x100005bf8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf28>
100005c44: ab1c02fb    	adds	x27, x23, x28
100005c48: 54003d82    	b.hs	0x1000063f8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1728>
100005c4c: 2a1803e8    	mov	w8, w24
100005c50: f9402be9    	ldr	x9, [sp, #0x50]
100005c54: 92607d29    	and	x9, x9, #0xffffffff00000000
100005c58: aa082128    	orr	x8, x9, x8, lsl #8
100005c5c: b27f0508    	orr	x8, x8, #0x6
100005c60: f9002be8    	str	x8, [sp, #0x50]
100005c64: 92800008    	mov	x8, #-0x1               ; =-1
100005c68: f94073f4    	ldr	x20, [sp, #0xe0]
100005c6c: b100051f    	cmn	x8, #0x1
100005c70: 54fffb80    	b.eq	0x100005be0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf10>
100005c74: f94007e9    	ldr	x9, [sp, #0x8]
100005c78: f9006fe9    	str	x9, [sp, #0xd8]
100005c7c: f9402bf6    	ldr	x22, [sp, #0x50]
100005c80: 17fffccf    	b	0x100004fbc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
100005c84: f94006b4    	ldr	x20, [x21, #0x8]
100005c88: d10283a8    	sub	x8, x29, #0xa0
100005c8c: 52800360    	mov	w0, #0x1b               ; =27
100005c90: 52800001    	mov	w1, #0x0                ; =0
100005c94: 52800022    	mov	w2, #0x1                ; =1
100005c98: 52800023    	mov	w3, #0x1                ; =1
100005c9c: 9400a914    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005ca0: b85603a8    	ldur	w8, [x29, #-0xa0]
100005ca4: 37003028    	tbnz	w8, #0x0, 0x1000062a8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
100005ca8: f85683a8    	ldur	x8, [x29, #-0x98]
100005cac: f100691f    	cmp	x8, #0x1a
100005cb0: 54002e09    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
100005cb4: f85703a9    	ldur	x9, [x29, #-0x90]
100005cb8: f000020b    	adrp	x11, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100005cbc: 9109116b    	add	x11, x11, #0x244
100005cc0: eb0b012a    	subs	x10, x9, x11
100005cc4: da8a854a    	cneg	x10, x10, ls
100005cc8: f100695f    	cmp	x10, #0x1a
100005ccc: 54002e09    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005cd0: 3dc00160    	ldr	q0, [x11]
100005cd4: 3d800120    	str	q0, [x9]
100005cd8: 3cc0b160    	ldur	q0, [x11, #0xb]
100005cdc: 3c80b120    	stur	q0, [x9, #0xb]
100005ce0: a93627a8    	stp	x8, x9, [x29, #-0xa0]
100005ce4: 52800368    	mov	w8, #0x1b               ; =27
100005ce8: a93773a8    	stp	x8, x28, [x29, #-0x90]
100005cec: 14000055    	b	0x100005e40 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1170>
100005cf0: 7120003f    	cmp	w1, #0x800
100005cf4: 540006c2    	b.hs	0x100005dcc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10fc>
100005cf8: 52800048    	mov	w8, #0x2                ; =2
100005cfc: 14000036    	b	0x100005dd4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1104>
100005d00: b100077f    	cmn	x27, #0x1
100005d04: 54003800    	b.eq	0x100006404 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1734>
100005d08: 91000768    	add	x8, x27, #0x1
100005d0c: f81683a8    	stur	x8, [x29, #-0x98]
100005d10: 92800008    	mov	x8, #-0x1               ; =-1
100005d14: f81603a8    	stur	x8, [x29, #-0xa0]
100005d18: 1400004b    	b	0x100005e44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1174>
100005d1c: f94016a9    	ldr	x9, [x21, #0x28]
100005d20: b90032b9    	str	w25, [x21, #0x30]
100005d24: 3100051f    	cmn	w8, #0x1
100005d28: 54001b20    	b.eq	0x10000608c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13bc>
100005d2c: 7102011f    	cmp	w8, #0x80
100005d30: 540012e2    	b.hs	0x100005f8c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12bc>
100005d34: 52800028    	mov	w8, #0x1                ; =1
100005d38: 140000d3    	b	0x100006084 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13b4>
100005d3c: f94016a9    	ldr	x9, [x21, #0x28]
100005d40: b90032b9    	str	w25, [x21, #0x30]
100005d44: 3100051f    	cmn	w8, #0x1
100005d48: 54001d60    	b.eq	0x1000060f4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1424>
100005d4c: 7102011f    	cmp	w8, #0x80
100005d50: 540012e2    	b.hs	0x100005fac <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12dc>
100005d54: 52800028    	mov	w8, #0x1                ; =1
100005d58: 140000e5    	b	0x1000060ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x141c>
100005d5c: 7140413f    	cmp	w9, #0x10, lsl #12      ; =0x10000
100005d60: 9a933669    	cinc	x9, x19, hs
100005d64: ab080128    	adds	x8, x9, x8
100005d68: 54003122    	b.hs	0x10000638c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16bc>
100005d6c: f81603a8    	stur	x8, [x29, #-0xa0]
100005d70: 910042a0    	add	x0, x21, #0x10
100005d74: 9400026e    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005d78: f90016a0    	str	x0, [x21, #0x28]
100005d7c: b90032a1    	str	w1, [x21, #0x30]
100005d80: 7100b43f    	cmp	w1, #0x2d
100005d84: 54000060    	b.eq	0x100005d90 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10c0>
100005d88: 7100ac3f    	cmp	w1, #0x2b
100005d8c: 540009e1    	b.ne	0x100005ec8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11f8>
100005d90: b90032b9    	str	w25, [x21, #0x30]
100005d94: 3100043f    	cmn	w1, #0x1
100005d98: 54000980    	b.eq	0x100005ec8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11f8>
100005d9c: 7102003f    	cmp	w1, #0x80
100005da0: 54000062    	b.hs	0x100005dac <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10dc>
100005da4: 52800028    	mov	w8, #0x1                ; =1
100005da8: 14000045    	b	0x100005ebc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11ec>
100005dac: 7120003f    	cmp	w1, #0x800
100005db0: 54000822    	b.hs	0x100005eb4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11e4>
100005db4: 52800048    	mov	w8, #0x2                ; =2
100005db8: 14000041    	b	0x100005ebc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11ec>
100005dbc: 7120013f    	cmp	w9, #0x800
100005dc0: 54000b62    	b.hs	0x100005f2c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x125c>
100005dc4: 52800049    	mov	w9, #0x2                ; =2
100005dc8: 1400005b    	b	0x100005f34 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1264>
100005dcc: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005dd0: 9a933668    	cinc	x8, x19, hs
100005dd4: ab1b0114    	adds	x20, x8, x27
100005dd8: 54002e62    	b.hs	0x1000063a4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16d4>
100005ddc: d10283a8    	sub	x8, x29, #0xa0
100005de0: 52800600    	mov	w0, #0x30               ; =48
100005de4: 52800001    	mov	w1, #0x0                ; =0
100005de8: 52800022    	mov	w2, #0x1                ; =1
100005dec: 52800023    	mov	w3, #0x1                ; =1
100005df0: 9400a8bf    	bl	0x1000300ec <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005df4: b85603a8    	ldur	w8, [x29, #-0xa0]
100005df8: 37002588    	tbnz	w8, #0x0, 0x1000062a8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
100005dfc: f85683a8    	ldur	x8, [x29, #-0x98]
100005e00: f100bd1f    	cmp	x8, #0x2f
100005e04: 54002369    	b.ls	0x100006270 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15a0>
100005e08: f85703a9    	ldur	x9, [x29, #-0x90]
100005e0c: f000020b    	adrp	x11, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100005e10: 911b296b    	add	x11, x11, #0x6ca
100005e14: eb0b012a    	subs	x10, x9, x11
100005e18: da8a854a    	cneg	x10, x10, ls
100005e1c: f100bd5f    	cmp	x10, #0x2f
100005e20: 54002369    	b.ls	0x10000628c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15bc>
100005e24: ad400560    	ldp	q0, q1, [x11]
100005e28: ad000520    	stp	q0, q1, [x9]
100005e2c: 3dc00960    	ldr	q0, [x11, #0x20]
100005e30: 3d800920    	str	q0, [x9, #0x20]
100005e34: a93627a8    	stp	x8, x9, [x29, #-0xa0]
100005e38: 52800608    	mov	w8, #0x30               ; =48
100005e3c: a9376fa8    	stp	x8, x27, [x29, #-0x90]
100005e40: f81803b4    	stur	x20, [x29, #-0x80]
100005e44: a97627a8    	ldp	x8, x9, [x29, #-0xa0]
100005e48: b100051f    	cmn	x8, #0x1
100005e4c: f94073f4    	ldr	x20, [sp, #0xe0]
100005e50: 540000c0    	b.eq	0x100005e68 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1198>
100005e54: f85703b4    	ldur	x20, [x29, #-0x90]
100005e58: 296f2faa    	ldp	w10, w11, [x29, #-0x88]
100005e5c: f85803ac    	ldur	x12, [x29, #-0x80]
100005e60: f90027ec    	str	x12, [sp, #0x48]
100005e64: 14000002    	b	0x100005e6c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x119c>
100005e68: 294b2beb    	ldp	w11, w10, [sp, #0x58]
100005e6c: b100051f    	cmn	x8, #0x1
100005e70: 9a890397    	csel	x23, x28, x9, eq
100005e74: 9a94013b    	csel	x27, x9, x20, eq
100005e78: 52800089    	mov	w9, #0x4                ; =4
100005e7c: 290b2beb    	stp	w11, w10, [sp, #0x58]
100005e80: 1a8a012a    	csel	w10, w9, w10, eq
100005e84: b9406fe9    	ldr	w9, [sp, #0x6c]
100005e88: 1a8b0129    	csel	w9, w9, w11, eq
100005e8c: b9006fe9    	str	w9, [sp, #0x6c]
100005e90: 540000a0    	b.eq	0x100005ea4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11d4>
100005e94: aa098156    	orr	x22, x10, x9, lsl #32
100005e98: f94027e9    	ldr	x9, [sp, #0x48]
100005e9c: f9006fe9    	str	x9, [sp, #0xd8]
100005ea0: 17fffc48    	b	0x100004fc0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2f0>
100005ea4: 12103d48    	and	w8, w10, #0xffff0000
100005ea8: aa0a03eb    	mov	x11, x10
100005eac: aa1703fc    	mov	x28, x23
100005eb0: 17fffc3d    	b	0x100004fa4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2d4>
100005eb4: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005eb8: 9a933668    	cinc	x8, x19, hs
100005ebc: ab000108    	adds	x8, x8, x0
100005ec0: 54002782    	b.hs	0x1000063b0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16e0>
100005ec4: f81603a8    	stur	x8, [x29, #-0xa0]
100005ec8: d10283a1    	sub	x1, x29, #0xa0
100005ecc: aa1503e0    	mov	x0, x21
100005ed0: 52800142    	mov	w2, #0xa                ; =10
100005ed4: 940001d5    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005ed8: 320083e8    	mov	w8, #0x10001            ; =65537
100005edc: 11040108    	add	w8, w8, #0x100
100005ee0: 7100001f    	cmp	w0, #0x0
100005ee4: 52802029    	mov	w9, #0x101              ; =257
100005ee8: 1a881134    	csel	w20, w9, w8, ne
100005eec: d10283a1    	sub	x1, x29, #0xa0
100005ef0: aa1503e0    	mov	x0, x21
100005ef4: 9400016c    	bl	0x1000064a4 <<source_files_to_tokens::lexer::Lexer>::consume_literal_suffix>
100005ef8: d2800009    	mov	x9, #0x0                ; =0
100005efc: 7100001f    	cmp	w0, #0x0
100005f00: 52a02008    	mov	w8, #0x1000000          ; =16777216
100005f04: 1a9f1108    	csel	w8, w8, wzr, ne
100005f08: f85603bb    	ldur	x27, [x29, #-0xa0]
100005f0c: 320083ea    	mov	w10, #0x10001           ; =65537
100005f10: 1103fd4a    	add	w10, w10, #0xff
100005f14: 0a0a028a    	and	w10, w20, w10
100005f18: 1210028b    	and	w11, w20, #0x10000
100005f1c: 2a0b0108    	orr	w8, w8, w11
100005f20: 5280002b    	mov	w11, #0x1               ; =1
100005f24: f94073f4    	ldr	x20, [sp, #0xe0]
100005f28: 17fffc1f    	b	0x100004fa4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2d4>
100005f2c: 7140413f    	cmp	w9, #0x10, lsl #12      ; =0x10000
100005f30: 9a933669    	cinc	x9, x19, hs
100005f34: ab080128    	adds	x8, x9, x8
100005f38: 540024e2    	b.hs	0x1000063d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1704>
100005f3c: f81903a8    	stur	x8, [x29, #-0x70]
100005f40: 910042a0    	add	x0, x21, #0x10
100005f44: 940001fa    	bl	0x10000672c <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005f48: f90016a0    	str	x0, [x21, #0x28]
100005f4c: b90032a1    	str	w1, [x21, #0x30]
100005f50: 7100b43f    	cmp	w1, #0x2d
100005f54: 54000060    	b.eq	0x100005f60 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1290>
100005f58: 7100ac3f    	cmp	w1, #0x2b
100005f5c: 540003a1    	b.ne	0x100005fd0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1300>
100005f60: b90032b9    	str	w25, [x21, #0x30]
100005f64: 3100043f    	cmn	w1, #0x1
100005f68: 54000340    	b.eq	0x100005fd0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1300>
100005f6c: 7102003f    	cmp	w1, #0x80
100005f70: 54000062    	b.hs	0x100005f7c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12ac>
100005f74: 52800028    	mov	w8, #0x1                ; =1
100005f78: 14000013    	b	0x100005fc4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12f4>
100005f7c: 7120003f    	cmp	w1, #0x800
100005f80: 540001e2    	b.hs	0x100005fbc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12ec>
100005f84: 52800048    	mov	w8, #0x2                ; =2
100005f88: 1400000f    	b	0x100005fc4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12f4>
100005f8c: 7120011f    	cmp	w8, #0x800
100005f90: 54000762    	b.hs	0x10000607c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13ac>
100005f94: 52800048    	mov	w8, #0x2                ; =2
100005f98: 1400003b    	b	0x100006084 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13b4>
100005f9c: 7120011f    	cmp	w8, #0x800
100005fa0: 54000882    	b.hs	0x1000060b0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13e0>
100005fa4: 52800048    	mov	w8, #0x2                ; =2
100005fa8: 14000044    	b	0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13e8>
100005fac: 7120011f    	cmp	w8, #0x800
100005fb0: 540009a2    	b.hs	0x1000060e4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1414>
100005fb4: 52800048    	mov	w8, #0x2                ; =2
100005fb8: 1400004d    	b	0x1000060ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x141c>
100005fbc: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005fc0: 9a933668    	cinc	x8, x19, hs
100005fc4: ab000108    	adds	x8, x8, x0
100005fc8: 54002242    	b.hs	0x100006410 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1740>
100005fcc: f81903a8    	stur	x8, [x29, #-0x70]
100005fd0: d101c3a1    	sub	x1, x29, #0x70
100005fd4: aa1503e0    	mov	x0, x21
100005fd8: 52800142    	mov	w2, #0xa                ; =10
100005fdc: 94000193    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005fe0: aa0003f4    	mov	x20, x0
100005fe4: d101c3a1    	sub	x1, x29, #0x70
100005fe8: aa1503e0    	mov	x0, x21
100005fec: 9400012e    	bl	0x1000064a4 <<source_files_to_tokens::lexer::Lexer>::consume_literal_suffix>
100005ff0: 7100029f    	cmp	w20, #0x0
100005ff4: 320083e8    	mov	w8, #0x10001            ; =65537
100005ff8: 1a9f0509    	csinc	w9, w8, wzr, eq
100005ffc: 52802008    	mov	w8, #0x100              ; =256
100006000: 7100001f    	cmp	w0, #0x0
100006004: 52a0200a    	mov	w10, #0x1000000         ; =16777216
100006008: 1a9f114a    	csel	w10, w10, wzr, ne
10000600c: 2a080128    	orr	w8, w9, w8
100006010: 2a0a010a    	orr	w10, w8, w10
100006014: f94073f4    	ldr	x20, [sp, #0xe0]
100006018: d2800009    	mov	x9, #0x0                ; =0
10000601c: f85903bb    	ldur	x27, [x29, #-0x70]
100006020: 12102548    	and	w8, w10, #0x3ff0000
100006024: aa0a03eb    	mov	x11, x10
100006028: 17fffbdf    	b	0x100004fa4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2d4>
10000602c: 3100043f    	cmn	w1, #0x1
100006030: 54000741    	b.ne	0x100006118 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1448>
100006034: f94006a3    	ldr	x3, [x21, #0x8]
100006038: d0000200    	adrp	x0, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
10000603c: 9108a800    	add	x0, x0, #0x22a
100006040: 52800341    	mov	w1, #0x1a               ; =26
100006044: aa1c03fb    	mov	x27, x28
100006048: 14000009    	b	0x10000606c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x139c>
10000604c: 7100651f    	cmp	w8, #0x19
100006050: 54000641    	b.ne	0x100006118 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1448>
100006054: b100041f    	cmn	x0, #0x1
100006058: 54001f60    	b.eq	0x100006444 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1774>
10000605c: 91000403    	add	x3, x0, #0x1
100006060: d0000200    	adrp	x0, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100006064: 911b2800    	add	x0, x0, #0x6ca
100006068: 52800601    	mov	w1, #0x30               ; =48
10000606c: d10283a8    	sub	x8, x29, #0xa0
100006070: aa1b03e2    	mov	x2, x27
100006074: 94000216    	bl	0x1000068cc <<source_files_to_tokens::lexer::lex_error::LexError>::new::<&str>>
100006078: 17ffff73    	b	0x100005e44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1174>
10000607c: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
100006080: 9a933668    	cinc	x8, x19, hs
100006084: ab090114    	adds	x20, x8, x9
100006088: 540019a2    	b.hs	0x1000063bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16ec>
10000608c: f81903b4    	stur	x20, [x29, #-0x70]
100006090: d101c3a1    	sub	x1, x29, #0x70
100006094: aa1503e0    	mov	x0, x21
100006098: 52800102    	mov	w2, #0x8                ; =8
10000609c: 94000163    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
1000060a0: 5280001a    	mov	w26, #0x0               ; =0
1000060a4: 52000014    	eor	w20, w0, #0x1
1000060a8: 52a00037    	mov	w23, #0x10000           ; =65536
1000060ac: 17fffe54    	b	0x1000059fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd2c>
1000060b0: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
1000060b4: 9a933668    	cinc	x8, x19, hs
1000060b8: ab090114    	adds	x20, x8, x9
1000060bc: 54001802    	b.hs	0x1000063bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16ec>
1000060c0: f81903b4    	stur	x20, [x29, #-0x70]
1000060c4: d101c3a1    	sub	x1, x29, #0x70
1000060c8: aa1503e0    	mov	x0, x21
1000060cc: 52800202    	mov	w2, #0x10               ; =16
1000060d0: 94000156    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
1000060d4: 5280001a    	mov	w26, #0x0               ; =0
1000060d8: 52000014    	eor	w20, w0, #0x1
1000060dc: 52a00077    	mov	w23, #0x30000           ; =196608
1000060e0: 17fffe47    	b	0x1000059fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd2c>
1000060e4: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
1000060e8: 9a933668    	cinc	x8, x19, hs
1000060ec: ab090114    	adds	x20, x8, x9
1000060f0: 54001662    	b.hs	0x1000063bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16ec>
1000060f4: f81903b4    	stur	x20, [x29, #-0x70]
1000060f8: d101c3a1    	sub	x1, x29, #0x70
1000060fc: aa1503e0    	mov	x0, x21
100006100: 52800042    	mov	w2, #0x2                ; =2
100006104: 94000149    	bl	0x100006628 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100006108: 5280001a    	mov	w26, #0x0               ; =0
10000610c: 52800017    	mov	w23, #0x0               ; =0
100006110: 52000014    	eor	w20, w0, #0x1
100006114: 17fffe3a    	b	0x1000059fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd2c>
100006118: aa0003f4    	mov	x20, x0
10000611c: aa0103f7    	mov	x23, x1
100006120: b818c3a1    	stur	w1, [x29, #-0x74]
100006124: d101d3a8    	sub	x8, x29, #0x74
100006128: f81603a8    	stur	x8, [x29, #-0xa0]
10000612c: d00001a8    	adrp	x8, 0x10003c000 <<u32 as core::fmt::Display>::fmt+0x38>
100006130: 913ff108    	add	x8, x8, #0xffc
100006134: f81683a8    	stur	x8, [x29, #-0x98]
100006138: d101c3a8    	sub	x8, x29, #0x70
10000613c: d10283a1    	sub	x1, x29, #0xa0
100006140: d00001e0    	adrp	x0, 0x100044000 <GCC_except_table669+0x4>
100006144: 913f2c00    	add	x0, x0, #0xfcb
100006148: 9400c252    	bl	0x100036a90 <alloc::fmt::format::format_inner>
10000614c: 710202ff    	cmp	w23, #0x80
100006150: aa1403e8    	mov	x8, x20
100006154: 54000062    	b.hs	0x100006160 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1490>
100006158: 52800029    	mov	w9, #0x1                ; =1
10000615c: 14000007    	b	0x100006178 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x14a8>
100006160: 712002ff    	cmp	w23, #0x800
100006164: 54000062    	b.hs	0x100006170 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x14a0>
100006168: 52800049    	mov	w9, #0x2                ; =2
10000616c: 14000003    	b	0x100006178 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x14a8>
100006170: 714042ff    	cmp	w23, #0x10, lsl #12     ; =0x10000
100006174: 9a933669    	cinc	x9, x19, hs
100006178: ab080128    	adds	x8, x9, x8
10000617c: 540016a2    	b.hs	0x100006450 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1780>
100006180: 3cd903a0    	ldur	q0, [x29, #-0x70]
100006184: 3c9603a0    	stur	q0, [x29, #-0xa0]
100006188: f85a03a9    	ldur	x9, [x29, #-0x60]
10000618c: a9376fa9    	stp	x9, x27, [x29, #-0x90]
100006190: f81803a8    	stur	x8, [x29, #-0x80]
100006194: 17ffff2c    	b	0x100005e44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1174>
100006198: 53087eca    	lsr	w10, w22, #8
10000619c: f94067ec    	ldr	x12, [sp, #0xc8]
1000061a0: a9016d97    	stp	x23, x27, [x12, #0x10]
1000061a4: d360fecb    	lsr	x11, x22, #32
1000061a8: 39008189    	strb	w9, [x12, #0x20]
1000061ac: 53187ec9    	lsr	w9, w22, #24
1000061b0: 39008d89    	strb	w9, [x12, #0x23]
1000061b4: 7802118a    	sturh	w10, [x12, #0x21]
1000061b8: b900258b    	str	w11, [x12, #0x24]
1000061bc: f94077e9    	ldr	x9, [sp, #0xe8]
1000061c0: f9001589    	str	x9, [x12, #0x28]
1000061c4: 52800049    	mov	w9, #0x2                ; =2
1000061c8: a9002189    	stp	x9, x8, [x12]
1000061cc: 9103c3e0    	add	x0, sp, #0xf0
1000061d0: 97fffa87    	bl	0x100004bec <core::ptr::drop_glue::<tokens::token_stream::TokenStream>>
1000061d4: 14000010    	b	0x100006214 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1544>
1000061d8: 3dc06fe2    	ldr	q2, [sp, #0x1b0]
1000061dc: f94067e8    	ldr	x8, [sp, #0xc8]
1000061e0: ad4b87e0    	ldp	q0, q1, [sp, #0x170]
1000061e4: ad040500    	stp	q0, q1, [x8, #0x80]
1000061e8: ad4c87e0    	ldp	q0, q1, [sp, #0x190]
1000061ec: 3d802900    	str	q0, [x8, #0xa0]
1000061f0: ad058901    	stp	q1, q2, [x8, #0xb0]
1000061f4: ad4987e0    	ldp	q0, q1, [sp, #0x130]
1000061f8: ad020500    	stp	q0, q1, [x8, #0x40]
1000061fc: ad4a87e0    	ldp	q0, q1, [sp, #0x150]
100006200: ad030500    	stp	q0, q1, [x8, #0x60]
100006204: ad4787e0    	ldp	q0, q1, [sp, #0xf0]
100006208: ad000500    	stp	q0, q1, [x8]
10000620c: ad4887e0    	ldp	q0, q1, [sp, #0x110]
100006210: ad010500    	stp	q0, q1, [x8, #0x20]
100006214: 910843ff    	add	sp, sp, #0x210
100006218: a9457bfd    	ldp	x29, x30, [sp, #0x50]
10000621c: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100006220: a94357f6    	ldp	x22, x21, [sp, #0x30]
100006224: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100006228: a94167fa    	ldp	x26, x25, [sp, #0x10]
10000622c: a8c66ffc    	ldp	x28, x27, [sp], #0x60
100006230: d65f03c0    	ret
100006234: aa1703fc    	mov	x28, x23
100006238: d00002d4    	adrp	x20, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
10000623c: 9127e294    	add	x20, x20, #0x9f8
100006240: aa1c03e2    	mov	x2, x28
100006244: aa1b03e3    	mov	x3, x27
100006248: aa1403e4    	mov	x4, x20
10000624c: 9400edf3    	bl	0x100041a18 <core::str::slice_error_fail>
100006250: 14000083    	b	0x10000645c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x178c>
100006254: d0000200    	adrp	x0, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100006258: 9117bc00    	add	x0, x0, #0x5ef
10000625c: d00002c3    	adrp	x3, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006260: 913f2063    	add	x3, x3, #0xfc8
100006264: 528036e1    	mov	w1, #0x1b7              ; =439
100006268: 52800002    	mov	w2, #0x0                ; =0
10000626c: 9400ee63    	bl	0x100041bf8 <core::panicking::panic_nounwind_fmt>
100006270: d0000200    	adrp	x0, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100006274: 91233000    	add	x0, x0, #0x8cc
100006278: d00002c3    	adrp	x3, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
10000627c: 913fe063    	add	x3, x3, #0xff8
100006280: 52803761    	mov	w1, #0x1bb              ; =443
100006284: 52800002    	mov	w2, #0x0                ; =0
100006288: 9400ee5c    	bl	0x100041bf8 <core::panicking::panic_nounwind_fmt>
10000628c: d0000200    	adrp	x0, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
100006290: 911ec400    	add	x0, x0, #0x7b1
100006294: d00002c3    	adrp	x3, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006298: 913f8063    	add	x3, x3, #0xfe0
10000629c: 528046e1    	mov	w1, #0x237              ; =567
1000062a0: 52800002    	mov	w2, #0x0                ; =0
1000062a4: 9400ee55    	bl	0x100041bf8 <core::panicking::panic_nounwind_fmt>
1000062a8: a94927e8    	ldp	x8, x9, [sp, #0x90]
1000062ac: f9400120    	ldr	x0, [x9]
1000062b0: f9400101    	ldr	x1, [x8]
1000062b4: 9400edad    	bl	0x100041968 <alloc::raw_vec::handle_error>
1000062b8: 14000069    	b	0x10000645c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x178c>
1000062bc: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000062c0: 91224000    	add	x0, x0, #0x890
1000062c4: 9400ef10    	bl	0x100041f04 <core::panicking::panic_const::panic_const_add_overflow>
1000062c8: 14000065    	b	0x10000645c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x178c>
1000062cc: a94927e8    	ldp	x8, x9, [sp, #0x90]
1000062d0: f9400120    	ldr	x0, [x9]
1000062d4: f9400101    	ldr	x1, [x8]
1000062d8: 9400eda4    	bl	0x100041968 <alloc::raw_vec::handle_error>
1000062dc: 14000060    	b	0x10000645c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x178c>
1000062e0: d00002c3    	adrp	x3, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000062e4: 9122a063    	add	x3, x3, #0x8a8
1000062e8: f9405be0    	ldr	x0, [sp, #0xb0]
1000062ec: f9403fe1    	ldr	x1, [sp, #0x78]
1000062f0: aa0103e2    	mov	x2, x1
1000062f4: 9400eec1    	bl	0x100041df8 <core::slice::index::slice_index_fail>
1000062f8: 14000059    	b	0x10000645c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x178c>
1000062fc: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006300: 91272000    	add	x0, x0, #0x9c8
100006304: 17fffff0    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006308: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
10000630c: 91248000    	add	x0, x0, #0x920
100006310: 17ffffed    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006314: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006318: 9121e000    	add	x0, x0, #0x878
10000631c: 17ffffea    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006320: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006324: 9125a000    	add	x0, x0, #0x968
100006328: 17ffffe7    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
10000632c: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006330: 91260000    	add	x0, x0, #0x980
100006334: 17ffffe4    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006338: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
10000633c: 91266000    	add	x0, x0, #0x998
100006340: 17ffffe1    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006344: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006348: 91242000    	add	x0, x0, #0x908
10000634c: 17ffffde    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006350: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006354: 9123c000    	add	x0, x0, #0x8f0
100006358: 17ffffdb    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
10000635c: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006360: 9126c000    	add	x0, x0, #0x9b0
100006364: 17ffffd8    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006368: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
10000636c: 9124e000    	add	x0, x0, #0x938
100006370: 17ffffd5    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006374: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006378: 912a2000    	add	x0, x0, #0xa88
10000637c: 17ffffd2    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006380: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006384: 91284000    	add	x0, x0, #0xa10
100006388: 17ffffcf    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
10000638c: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006390: 912a8000    	add	x0, x0, #0xaa0
100006394: 17ffffcc    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006398: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
10000639c: 91218000    	add	x0, x0, #0x860
1000063a0: 17ffffc9    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063a4: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063a8: 912d2000    	add	x0, x0, #0xb48
1000063ac: 17ffffc6    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063b0: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063b4: 912ae000    	add	x0, x0, #0xab8
1000063b8: 17ffffc3    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063bc: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063c0: 9129c000    	add	x0, x0, #0xa70
1000063c4: 17ffffc0    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063c8: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063cc: 912c6000    	add	x0, x0, #0xb18
1000063d0: 17ffffbd    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063d4: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063d8: 91290000    	add	x0, x0, #0xa40
1000063dc: 17ffffba    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063e0: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063e4: 9128a000    	add	x0, x0, #0xa28
1000063e8: 17ffffb7    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063ec: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063f0: 91236000    	add	x0, x0, #0x8d8
1000063f4: 17ffffb4    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000063f8: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
1000063fc: 91230000    	add	x0, x0, #0x8c0
100006400: 17ffffb1    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006404: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006408: 912cc000    	add	x0, x0, #0xb30
10000640c: 17ffffae    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006410: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006414: 91296000    	add	x0, x0, #0xa58
100006418: 17ffffab    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
10000641c: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006420: 91254000    	add	x0, x0, #0x950
100006424: 17ffffa8    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006428: d0000200    	adrp	x0, 0x100048000 <_anon.f099a9a5855ddd444200ff264f056ec5.31+0xae0>
10000642c: 91136000    	add	x0, x0, #0x4d8
100006430: d00002c3    	adrp	x3, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006434: 913e6063    	add	x3, x3, #0xf98
100006438: 528045e1    	mov	w1, #0x22f              ; =559
10000643c: 52800002    	mov	w2, #0x0                ; =0
100006440: 9400edee    	bl	0x100041bf8 <core::panicking::panic_nounwind_fmt>
100006444: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006448: 912d8000    	add	x0, x0, #0xb60
10000644c: 17ffff9e    	b	0x1000062c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100006450: d00002c0    	adrp	x0, 0x100060000 <_anon.f099a9a5855ddd444200ff264f056ec5.0>
100006454: 912de000    	add	x0, x0, #0xb78
100006458: 9400eeab    	bl	0x100041f04 <core::panicking::panic_const::panic_const_add_overflow>
10000645c: d4200020    	brk	#0x1
100006460: aa0003f3    	mov	x19, x0
100006464: d101c3a0    	sub	x0, x29, #0x70
100006468: 97fff9d2    	bl	0x100004bb0 <core::ptr::drop_glue::<alloc::string::String>>
10000646c: 14000009    	b	0x100006490 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17c0>
100006470: 9400edce    	bl	0x100041ba8 <core::panicking::panic_in_cleanup>
100006474: 14000006    	b	0x10000648c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17bc>
100006478: 14000005    	b	0x10000648c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17bc>
10000647c: 14000004    	b	0x10000648c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17bc>
100006480: 14000003    	b	0x10000648c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17bc>
100006484: 14000002    	b	0x10000648c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17bc>
100006488: 14000001    	b	0x10000648c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17bc>
10000648c: aa0003f3    	mov	x19, x0
100006490: 9103c3e0    	add	x0, sp, #0xf0
100006494: 97fff9d6    	bl	0x100004bec <core::ptr::drop_glue::<tokens::token_stream::TokenStream>>
100006498: aa1303e0    	mov	x0, x19
10000649c: 9400ef09    	bl	0x1000420c0 <dyld_stub_binder+0x1000420c0>
1000064a0: 9400edc2    	bl	0x100041ba8 <core::panicking::panic_in_cleanup>
