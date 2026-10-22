000000010000232c <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter>:
10000232c: d10183ff    	sub	sp, sp, #0x60
100002330: a9025ff8    	stp	x24, x23, [sp, #0x20]
100002334: a90357f6    	stp	x22, x21, [sp, #0x30]
100002338: a9044ff4    	stp	x20, x19, [sp, #0x40]
10000233c: a9057bfd    	stp	x29, x30, [sp, #0x50]
100002340: 910143fd    	add	x29, sp, #0x50
100002344: aa0803f4    	mov	x20, x8
100002348: f9400815    	ldr	x21, [x0, #0x10]
10000234c: d37afea8    	lsr	x8, x21, #58
100002350: b5000c48    	cbnz	x8, 0x1000024d8 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x1ac>
100002354: aa0003f3    	mov	x19, x0
100002358: f9400c09    	ldr	x9, [x0, #0x18]
10000235c: a9402016    	ldp	x22, x8, [x0]
100002360: aa1603ea    	mov	x10, x22
100002364: eb09011f    	cmp	x8, x9
100002368: 54000420    	b.eq	0x1000023ec <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0xc0>
10000236c: f940126b    	ldr	x11, [x19, #0x20]
100002370: 5280002c    	mov	w12, #0x1               ; =1
100002374: aa1603ea    	mov	x10, x22
100002378: 1400000e    	b	0x1000023b0 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x84>
10000237c: a9408101    	ldp	x1, x0, [x8, #0x8]
100002380: 91008102    	add	x2, x8, #0x20
100002384: f9400d11    	ldr	x17, [x8, #0x18]
100002388: 91010108    	add	x8, x8, #0x40
10000238c: f9400042    	ldr	x2, [x2]
100002390: a9000550    	stp	x16, x1, [x10]
100002394: a9014540    	stp	x0, x17, [x10, #0x10]
100002398: a9023542    	stp	x2, x13, [x10, #0x20]
10000239c: f900194e    	str	x14, [x10, #0x30]
1000023a0: b900394f    	str	w15, [x10, #0x38]
1000023a4: 9101014a    	add	x10, x10, #0x40
1000023a8: eb09011f    	cmp	x8, x9
1000023ac: 540001e0    	b.eq	0x1000023e8 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0xbc>
1000023b0: f9400110    	ldr	x16, [x8]
1000023b4: a942b90d    	ldp	x13, x14, [x8, #0x28]
1000023b8: b940390f    	ldr	w15, [x8, #0x38]
1000023bc: b5fffe10    	cbnz	x16, 0x10000237c <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x50>
1000023c0: f9400171    	ldr	x17, [x11]
1000023c4: f82c0223    	ldadd	x12, x3, [x17]
1000023c8: 91002162    	add	x2, x11, #0x8
1000023cc: aa0e03e0    	mov	x0, x14
1000023d0: aa0d03e1    	mov	x1, x13
1000023d4: 52800030    	mov	w16, #0x1               ; =1
1000023d8: b6fffd83    	tbz	x3, #0x3f, 0x100002388 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x5c>
1000023dc: 91010108    	add	x8, x8, #0x40
1000023e0: f9000668    	str	x8, [x19, #0x8]
1000023e4: d4200020    	brk	#0x1
1000023e8: f9000668    	str	x8, [x19, #0x8]
1000023ec: eb160148    	subs	x8, x10, x22
1000023f0: 54000823    	b.lo	0x1000024f4 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x1c8>
1000023f4: d346fd18    	lsr	x24, x8, #6
1000023f8: a900e3f6    	stp	x22, x24, [sp, #0x8]
1000023fc: f9000ff5    	str	x21, [sp, #0x18]
100002400: aa1303e0    	mov	x0, x19
100002404: 9400127f    	bl	0x100006e00 <<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>>::forget_allocation_drop_remaining>
100002408: aa1503e0    	mov	x0, x21
10000240c: aa1503e1    	mov	x1, x21
100002410: 94000957    	bl	0x10000496c <alloc::vec::in_place_collect::needs_realloc::<tokens::token_stream::token::Token, tokens::token_stream::token::Token>>
100002414: 360004c0    	tbz	w0, #0x0, 0x1000024ac <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x180>
100002418: d379fea8    	lsr	x8, x21, #57
10000241c: b5000848    	cbnz	x8, 0x100002524 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x1f8>
100002420: d37ae6b7    	lsl	x23, x21, #6
100002424: b4000415    	cbz	x21, 0x1000024a4 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x178>
100002428: aa1603e1    	mov	x1, x22
10000242c: 52800102    	mov	w2, #0x8                ; =8
100002430: aa1703e3    	mov	x3, x23
100002434: aa1703e4    	mov	x4, x23
100002438: 97fffe2f    	bl	0x100001cf4 <<std::alloc::System as core::alloc::global::GlobalAlloc>::realloc>
10000243c: aa0003f6    	mov	x22, x0
100002440: b4000340    	cbz	x0, 0x1000024a8 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x17c>
100002444: d0000308    	adrp	x8, 0x100064000 <dyld_stub_binder+0x100064000>
100002448: 39764108    	ldrb	w8, [x8, #0xd90]
10000244c: 340002e8    	cbz	w8, 0x1000024a8 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x17c>
100002450: d0000308    	adrp	x8, 0x100064000 <dyld_stub_binder+0x100064000>
100002454: 91362108    	add	x8, x8, #0xd88
100002458: f8370108    	ldadd	x23, x8, [x8]
10000245c: cb1703e8    	neg	x8, x23
100002460: d0000309    	adrp	x9, 0x100064000 <dyld_stub_binder+0x100064000>
100002464: 9135c129    	add	x9, x9, #0xd70
100002468: f8280128    	ldadd	x8, x8, [x9]
10000246c: d0000308    	adrp	x8, 0x100064000 <dyld_stub_binder+0x100064000>
100002470: 91360108    	add	x8, x8, #0xd80
100002474: 5280002a    	mov	w10, #0x1               ; =1
100002478: f82a0108    	ldadd	x10, x8, [x8]
10000247c: d0000308    	adrp	x8, 0x100064000 <dyld_stub_binder+0x100064000>
100002480: 91366108    	add	x8, x8, #0xd98
100002484: f8370108    	ldadd	x23, x8, [x8]
100002488: f8370128    	ldadd	x23, x8, [x9]
10000248c: ab170108    	adds	x8, x8, x23
100002490: 54000602    	b.hs	0x100002550 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x224>
100002494: d0000309    	adrp	x9, 0x100064000 <dyld_stub_binder+0x100064000>
100002498: 9135e129    	add	x9, x9, #0xd78
10000249c: f8286128    	ldumax	x8, x8, [x9]
1000024a0: 14000002    	b	0x1000024a8 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x17c>
1000024a4: 52800116    	mov	w22, #0x8               ; =8
1000024a8: b40004d6    	cbz	x22, 0x100002540 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x214>
1000024ac: eb15031f    	cmp	x24, x21
1000024b0: 540002c8    	b.hi	0x100002508 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x1dc>
1000024b4: a9005a95    	stp	x21, x22, [x20]
1000024b8: f9000a98    	str	x24, [x20, #0x10]
1000024bc: aa1303e0    	mov	x0, x19
1000024c0: a9457bfd    	ldp	x29, x30, [sp, #0x50]
1000024c4: a9444ff4    	ldp	x20, x19, [sp, #0x40]
1000024c8: a94357f6    	ldp	x22, x21, [sp, #0x30]
1000024cc: a9425ff8    	ldp	x24, x23, [sp, #0x20]
1000024d0: 910183ff    	add	sp, sp, #0x60
1000024d4: 14001283    	b	0x100006ee0 <<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token> as core::ops::drop::Drop>::drop>
1000024d8: b0000220    	adrp	x0, 0x100047000 <_anon.39fb678c2aae451e69c32fd226da1a5f.30+0xcb>
1000024dc: 91030400    	add	x0, x0, #0xc1
1000024e0: d00002e3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000024e4: 91012063    	add	x3, x3, #0x48
1000024e8: 52802ea1    	mov	w1, #0x175              ; =373
1000024ec: 52800002    	mov	w2, #0x0                ; =0
1000024f0: 9400fcf2    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
1000024f4: 90000220    	adrp	x0, 0x100046000 <_anon.e1ed9a3023bc574f8c6e0db28b9f6bb5.23+0x9f2>
1000024f8: 91314000    	add	x0, x0, #0xc50
1000024fc: d00002e3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100002500: 9109c063    	add	x3, x3, #0x270
100002504: 14000005    	b	0x100002518 <<alloc::vec::Vec<tokens::token_stream::token::Token> as alloc::vec::spec_from_iter::SpecFromIter<tokens::token_stream::token::Token, core::iter::adapters::map::Map<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token>, token_ingress_probe::retain::{closure#0}>>>::from_iter+0x1ec>
100002508: b0000220    	adrp	x0, 0x100047000 <_anon.39fb678c2aae451e69c32fd226da1a5f.30+0xcb>
10000250c: 9105ec00    	add	x0, x0, #0x17b
100002510: d00002e3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100002514: 9101e063    	add	x3, x3, #0x78
100002518: 52803261    	mov	w1, #0x193              ; =403
10000251c: 52800002    	mov	w2, #0x0                ; =0
100002520: 9400fce6    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
100002524: 90000220    	adrp	x0, 0x100046000 <_anon.e1ed9a3023bc574f8c6e0db28b9f6bb5.23+0x9f2>
100002528: 91346400    	add	x0, x0, #0xd19
10000252c: d00002e3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100002530: 91018063    	add	x3, x3, #0x60
100002534: 52804661    	mov	w1, #0x233              ; =563
100002538: 52800002    	mov	w2, #0x0                ; =0
10000253c: 9400fcdf    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
100002540: 52800100    	mov	w0, #0x8                ; =8
100002544: aa1703e1    	mov	x1, x23
100002548: 9400fc32    	bl	0x100041610 <alloc::alloc::handle_alloc_error>
10000254c: d4200020    	brk	#0x1
100002550: d00002e0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100002554: 9117a000    	add	x0, x0, #0x5e8
100002558: 9400fd9b    	bl	0x100041bc4 <core::panicking::panic_const::panic_const_add_overflow>
10000255c: aa0003f4    	mov	x20, x0
100002560: 910023e0    	add	x0, sp, #0x8
100002564: 97fff917    	bl	0x1000009c0 <core::ptr::drop_glue::<alloc::vec::in_place_drop::InPlaceDstDataSrcBufDrop<tokens::token_stream::token::Token, tokens::token_stream::token::Token>>>
100002568: aa1303e0    	mov	x0, x19
10000256c: 9400125d    	bl	0x100006ee0 <<alloc::vec::into_iter::IntoIter<tokens::token_stream::token::Token> as core::ops::drop::Drop>::drop>
100002570: aa1403e0    	mov	x0, x20
100002574: 9400fe03    	bl	0x100041d80 <dyld_stub_binder+0x100041d80>
100002578: 9400fcbc    	bl	0x100041868 <core::panicking::panic_in_cleanup>

0000000100004b1c <<source_files_to_tokens::lexer::Lexer>::tokenize>:
100004b1c: a9ba6ffc    	stp	x28, x27, [sp, #-0x60]!
100004b20: a90167fa    	stp	x26, x25, [sp, #0x10]
100004b24: a9025ff8    	stp	x24, x23, [sp, #0x20]
100004b28: a90357f6    	stp	x22, x21, [sp, #0x30]
100004b2c: a9044ff4    	stp	x20, x19, [sp, #0x40]
100004b30: a9057bfd    	stp	x29, x30, [sp, #0x50]
100004b34: 910143fd    	add	x29, sp, #0x50
100004b38: d10683ff    	sub	sp, sp, #0x1a0
100004b3c: aa0003f5    	mov	x21, x0
100004b40: aa0803fc    	mov	x28, x8
100004b44: f9400408    	ldr	x8, [x0, #0x8]
100004b48: d342fd08    	lsr	x8, x8, #2
100004b4c: 52800209    	mov	w9, #0x10               ; =16
100004b50: f100411f    	cmp	x8, #0x10
100004b54: 9a898114    	csel	x20, x8, x9, hi
100004b58: d10343b3    	sub	x19, x29, #0xd0
100004b5c: d10343a8    	sub	x8, x29, #0xd0
100004b60: aa1403e0    	mov	x0, x20
100004b64: 52800001    	mov	w1, #0x0                ; =0
100004b68: 52800102    	mov	w2, #0x8                ; =8
100004b6c: 52800803    	mov	w3, #0x40               ; =64
100004b70: 9400ac8f    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100004b74: a97303a8    	ldp	x8, x0, [x29, #-0xd0]
100004b78: b500ab28    	cbnz	x8, 0x1000060dc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15c0>
100004b7c: eb00029f    	cmp	x20, x0
100004b80: 5400ab28    	b.hi	0x1000060e4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15c8>
100004b84: 91002269    	add	x9, x19, #0x8
100004b88: 91004268    	add	x8, x19, #0x10
100004b8c: a90c27e8    	stp	x8, x9, [sp, #0xc0]
100004b90: f85403a8    	ldur	x8, [x29, #-0xc0]
100004b94: 12800036    	mov	w22, #-0x2              ; =-2
100004b98: a931a3a0    	stp	x0, x8, [x29, #-0xe8]
100004b9c: 52800054    	mov	w20, #0x2               ; =2
100004ba0: f81283bf    	stur	xzr, [x29, #-0xd8]
100004ba4: 52800078    	mov	w24, #0x3               ; =3
100004ba8: d284c013    	mov	x19, #0x2600            ; =9728
100004bac: f2c00033    	movk	x19, #0x1, lsl #32
100004bb0: 14000009    	b	0x100004bd4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb8>
100004bb4: f85203a8    	ldur	x8, [x29, #-0xe0]
100004bb8: 8b1a1908    	add	x8, x8, x26, lsl #6
100004bbc: ad7a87a0    	ldp	q0, q1, [x29, #-0xb0]
100004bc0: ad798ba3    	ldp	q3, q2, [x29, #-0xd0]
100004bc4: ad010500    	stp	q0, q1, [x8, #0x20]
100004bc8: ad000903    	stp	q3, q2, [x8]
100004bcc: 91000748    	add	x8, x26, #0x1
100004bd0: f81283a8    	stur	x8, [x29, #-0xd8]
100004bd4: f94016a8    	ldr	x8, [x21, #0x28]
100004bd8: f81103a8    	stur	x8, [x29, #-0xf0]
100004bdc: b94032a1    	ldr	w1, [x21, #0x30]
100004be0: b90032b6    	str	w22, [x21, #0x30]
100004be4: 3100083f    	cmn	w1, #0x2
100004be8: 54000081    	b.ne	0x100004bf8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdc>
100004bec: 910042a0    	add	x0, x21, #0x10
100004bf0: 94000675    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004bf4: f81103a0    	stur	x0, [x29, #-0xf0]
100004bf8: 3100043f    	cmn	w1, #0x1
100004bfc: 54000320    	b.eq	0x100004c60 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x144>
100004c00: 7101c83f    	cmp	w1, #0x72
100004c04: 54000341    	b.ne	0x100004c6c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x150>
100004c08: f85103a8    	ldur	x8, [x29, #-0xf0]
100004c0c: b100051f    	cmn	x8, #0x1
100004c10: 5400aaa0    	b.eq	0x100006164 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1648>
100004c14: 91000508    	add	x8, x8, #0x1
100004c18: f94006a9    	ldr	x9, [x21, #0x8]
100004c1c: eb09011f    	cmp	x8, x9
100004c20: 540002a2    	b.hs	0x100004c74 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x158>
100004c24: f94002aa    	ldr	x10, [x21]
100004c28: 3868694b    	ldrb	w11, [x10, x8]
100004c2c: 7100897f    	cmp	w11, #0x22
100004c30: 54001a20    	b.eq	0x100004f74 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x458>
100004c34: 71008d7f    	cmp	w11, #0x23
100004c38: 540001e1    	b.ne	0x100004c74 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x158>
100004c3c: 3868694b    	ldrb	w11, [x10, x8]
100004c40: 71008d7f    	cmp	w11, #0x23
100004c44: 540028e1    	b.ne	0x100005160 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x644>
100004c48: 91000508    	add	x8, x8, #0x1
100004c4c: eb08013f    	cmp	x9, x8
100004c50: 54ffff61    	b.ne	0x100004c3c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x120>
100004c54: d2800017    	mov	x23, #0x0               ; =0
100004c58: aa0903e8    	mov	x8, x9
100004c5c: 14000143    	b	0x100005168 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x64c>
100004c60: b2401f39    	orr	x25, x25, #0xff
100004c64: 92800008    	mov	x8, #-0x1               ; =-1
100004c68: 1400006f    	b	0x100004e24 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x308>
100004c6c: 52800057    	mov	w23, #0x2               ; =2
100004c70: 14000002    	b	0x100004c78 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15c>
100004c74: d2800017    	mov	x23, #0x0               ; =0
100004c78: 7100bc3f    	cmp	w1, #0x2f
100004c7c: 540006c8    	b.hi	0x100004d54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x238>
100004c80: 2a0103e8    	mov	w8, w1
100004c84: 9ac82669    	lsr	x9, x19, x8
100004c88: 36000129    	tbz	w9, #0x0, 0x100004cac <<source_files_to_tokens::lexer::Lexer>::tokenize+0x190>
100004c8c: 7102003f    	cmp	w1, #0x80
100004c90: 54000062    	b.hs	0x100004c9c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x180>
100004c94: 52800028    	mov	w8, #0x1                ; =1
100004c98: 1400003e    	b	0x100004d90 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x274>
100004c9c: 7120003f    	cmp	w1, #0x800
100004ca0: 54000742    	b.hs	0x100004d88 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x26c>
100004ca4: 52800048    	mov	w8, #0x2                ; =2
100004ca8: 1400003a    	b	0x100004d90 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x274>
100004cac: f100bd1f    	cmp	x8, #0x2f
100004cb0: 54000521    	b.ne	0x100004d54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x238>
100004cb4: b94032a8    	ldr	w8, [x21, #0x30]
100004cb8: 3100091f    	cmn	w8, #0x2
100004cbc: 54000101    	b.ne	0x100004cdc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1c0>
100004cc0: 910042a0    	add	x0, x21, #0x10
100004cc4: aa0103fa    	mov	x26, x1
100004cc8: 9400063f    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004ccc: aa0103e8    	mov	x8, x1
100004cd0: aa1a03e1    	mov	x1, x26
100004cd4: f90016a0    	str	x0, [x21, #0x28]
100004cd8: b90032a8    	str	w8, [x21, #0x30]
100004cdc: b94032a8    	ldr	w8, [x21, #0x30]
100004ce0: 7100a91f    	cmp	w8, #0x2a
100004ce4: 54001540    	b.eq	0x100004f8c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x470>
100004ce8: 7100bd1f    	cmp	w8, #0x2f
100004cec: 54000341    	b.ne	0x100004d54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x238>
100004cf0: f85103a8    	ldur	x8, [x29, #-0xf0]
100004cf4: b100051f    	cmn	x8, #0x1
100004cf8: 5400a540    	b.eq	0x1000061a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1684>
100004cfc: b90032b6    	str	w22, [x21, #0x30]
100004d00: b100091f    	cmn	x8, #0x2
100004d04: 5400a480    	b.eq	0x100006194 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1678>
100004d08: 9100091b    	add	x27, x8, #0x2
100004d0c: b94032a1    	ldr	w1, [x21, #0x30]
100004d10: b90032b6    	str	w22, [x21, #0x30]
100004d14: 3100083f    	cmn	w1, #0x2
100004d18: 54000061    	b.ne	0x100004d24 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x208>
100004d1c: 910042a0    	add	x0, x21, #0x10
100004d20: 94000629    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004d24: 3100043f    	cmn	w1, #0x1
100004d28: 7a4a1824    	ccmp	w1, #0xa, #0x4, ne
100004d2c: 54001ce0    	b.eq	0x1000050c8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x5ac>
100004d30: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100004d34: 9a983708    	cinc	x8, x24, hs
100004d38: 7120003f    	cmp	w1, #0x800
100004d3c: 9a883288    	csel	x8, x20, x8, lo
100004d40: 7102003f    	cmp	w1, #0x80
100004d44: 9a9f2508    	csinc	x8, x8, xzr, hs
100004d48: ab1b011b    	adds	x27, x8, x27
100004d4c: 54fffe03    	b.lo	0x100004d0c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1f0>
100004d50: 140004ff    	b	0x10000614c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1630>
100004d54: 37000dd7    	tbnz	w23, #0x0, 0x100004f0c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x3f0>
100004d58: 529ffbe8    	mov	w8, #0xffdf             ; =65503
100004d5c: 72a003e8    	movk	w8, #0x1f, lsl #16
100004d60: 0a080028    	and	w8, w1, w8
100004d64: 51010508    	sub	w8, w8, #0x41
100004d68: 7100691f    	cmp	w8, #0x1a
100004d6c: 52800be8    	mov	w8, #0x5f               ; =95
100004d70: 7a482024    	ccmp	w1, w8, #0x4, hs
100004d74: 54001681    	b.ne	0x100005044 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x528>
100004d78: 7102003f    	cmp	w1, #0x80
100004d7c: 54001fe2    	b.hs	0x100005178 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x65c>
100004d80: 52800028    	mov	w8, #0x1                ; =1
100004d84: 140001fe    	b	0x10000557c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa60>
100004d88: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100004d8c: 9a983708    	cinc	x8, x24, hs
100004d90: f85103a9    	ldur	x9, [x29, #-0xf0]
100004d94: ab09011b    	adds	x27, x8, x9
100004d98: 54009e02    	b.hs	0x100006158 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x163c>
100004d9c: b94032a8    	ldr	w8, [x21, #0x30]
100004da0: 3100091f    	cmn	w8, #0x2
100004da4: 540000a1    	b.ne	0x100004db8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x29c>
100004da8: 910042a0    	add	x0, x21, #0x10
100004dac: 94000606    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004db0: f90016a0    	str	x0, [x21, #0x28]
100004db4: b90032a1    	str	w1, [x21, #0x30]
100004db8: b94032a8    	ldr	w8, [x21, #0x30]
100004dbc: 7100811f    	cmp	w8, #0x20
100004dc0: 540001c8    	b.hi	0x100004df8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2dc>
100004dc4: 9ac82669    	lsr	x9, x19, x8
100004dc8: 36000189    	tbz	w9, #0x0, 0x100004df8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2dc>
100004dcc: f94016a9    	ldr	x9, [x21, #0x28]
100004dd0: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
100004dd4: 9a98370a    	cinc	x10, x24, hs
100004dd8: 7120011f    	cmp	w8, #0x800
100004ddc: 9a8a328a    	csel	x10, x20, x10, lo
100004de0: 7102011f    	cmp	w8, #0x80
100004de4: 9a9f2548    	csinc	x8, x10, xzr, hs
100004de8: ab09011b    	adds	x27, x8, x9
100004dec: 54009702    	b.hs	0x1000060cc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b0>
100004df0: b90032b6    	str	w22, [x21, #0x30]
100004df4: 17ffffea    	b	0x100004d9c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x280>
100004df8: 52800008    	mov	w8, #0x0                ; =0
100004dfc: 5280000a    	mov	w10, #0x0               ; =0
100004e00: d2800009    	mov	x9, #0x0                ; =0
100004e04: 528000eb    	mov	w11, #0x7               ; =7
100004e08: 12181d4a    	and	w10, w10, #0xff00
100004e0c: 2a080148    	orr	w8, w10, w8
100004e10: 12001d6a    	and	w10, w11, #0xff
100004e14: 2a0a0108    	orr	w8, w8, w10
100004e18: aa098119    	orr	x25, x8, x9, lsl #32
100004e1c: 92800008    	mov	x8, #-0x1               ; =-1
100004e20: f85103b7    	ldur	x23, [x29, #-0xf0]
100004e24: 12001f2b    	and	w11, w25, #0xff
100004e28: b100051f    	cmn	x8, #0x1
100004e2c: a94f27ea    	ldp	x10, x9, [sp, #0xf0]
100004e30: 9a8a0129    	csel	x9, x9, x10, eq
100004e34: f9007fe9    	str	x9, [sp, #0xf8]
100004e38: 54008bc1    	b.ne	0x100005fb0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1494>
100004e3c: 7103fd7f    	cmp	w11, #0xff
100004e40: 54008ee0    	b.eq	0x10000601c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1500>
100004e44: a94006a0    	ldp	x0, x1, [x21]
100004e48: eb01037f    	cmp	x27, x1
100004e4c: fa579360    	ccmp	x27, x23, #0x0, ls
100004e50: 54000082    	b.hs	0x100004e60 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x344>
100004e54: d280000a    	mov	x10, #0x0               ; =0
100004e58: b500026a    	cbnz	x10, 0x100004ea4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x388>
100004e5c: 1400047e    	b	0x100006054 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1538>
100004e60: eb0102ff    	cmp	x23, x1
100004e64: 54000140    	b.eq	0x100004e8c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x370>
100004e68: b4000097    	cbz	x23, 0x100004e78 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x35c>
100004e6c: 38f76808    	ldrsb	w8, [x0, x23]
100004e70: 3101051f    	cmn	w8, #0x41
100004e74: 54ffff0d    	b.le	0x100004e54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x338>
100004e78: eb01037f    	cmp	x27, x1
100004e7c: 54000080    	b.eq	0x100004e8c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x370>
100004e80: 38fb6808    	ldrsb	w8, [x0, x27]
100004e84: 3101011f    	cmn	w8, #0x40
100004e88: 54fffe6b    	b.lt	0x100004e54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x338>
100004e8c: eb170369    	subs	x9, x27, x23
100004e90: 54008ee3    	b.lo	0x10000606c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1550>
100004e94: eb01037f    	cmp	x27, x1
100004e98: 54008ea8    	b.hi	0x10000606c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1550>
100004e9c: 8b17000a    	add	x10, x0, x23
100004ea0: b4008daa    	cbz	x10, 0x100006054 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1538>
100004ea4: 7100117f    	cmp	w11, #0x4
100004ea8: 54000121    	b.ne	0x100004ecc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x3b0>
100004eac: d10343a8    	sub	x8, x29, #0xd0
100004eb0: aa0a03e0    	mov	x0, x10
100004eb4: aa0903e1    	mov	x1, x9
100004eb8: 940006bd    	bl	0x1000069ac <source_files_to_tokens::lexer::strings::decode::decode_string_literal>
100004ebc: d10223a8    	sub	x8, x29, #0x88
100004ec0: d10343a0    	sub	x0, x29, #0xd0
100004ec4: 94001797    	bl	0x10000ad20 <<tokens::token_stream::token_text::TokenText>::owned_bytes>
100004ec8: 14000003    	b	0x100004ed4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x3b8>
100004ecc: a93827aa    	stp	x10, x9, [x29, #-0x80]
100004ed0: f81783bf    	stur	xzr, [x29, #-0x88]
100004ed4: b81683b9    	stur	w25, [x29, #-0x98]
100004ed8: 3cd783a0    	ldur	q0, [x29, #-0x88]
100004edc: 3cd883a1    	ldur	q1, [x29, #-0x78]
100004ee0: ad3987a0    	stp	q0, q1, [x29, #-0xd0]
100004ee4: f85983a8    	ldur	x8, [x29, #-0x68]
100004ee8: a9355fa8    	stp	x8, x23, [x29, #-0xb0]
100004eec: f81603bb    	stur	x27, [x29, #-0xa0]
100004ef0: f85283ba    	ldur	x26, [x29, #-0xd8]
100004ef4: f85183a8    	ldur	x8, [x29, #-0xe8]
100004ef8: eb08035f    	cmp	x26, x8
100004efc: 54ffe5c1    	b.ne	0x100004bb4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x98>
100004f00: d103a3a0    	sub	x0, x29, #0xe8
100004f04: 9400069e    	bl	0x10000697c <<alloc::raw_vec::RawVec<tokens::token_stream::token::Token>>::grow_one>
100004f08: 17ffff2b    	b	0x100004bb4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x98>
100004f0c: d10343a8    	sub	x8, x29, #0xd0
100004f10: 52800600    	mov	w0, #0x30               ; =48
100004f14: 52800001    	mov	w1, #0x0                ; =0
100004f18: 52800022    	mov	w2, #0x1                ; =1
100004f1c: 52800023    	mov	w3, #0x1                ; =1
100004f20: 9400aba3    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100004f24: b85303a8    	ldur	w8, [x29, #-0xd0]
100004f28: 37008c88    	tbnz	w8, #0x0, 0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x159c>
100004f2c: f85383a8    	ldur	x8, [x29, #-0xc8]
100004f30: f100bd1f    	cmp	x8, #0x2f
100004f34: 54008aa9    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
100004f38: f85403b7    	ldur	x23, [x29, #-0xc0]
100004f3c: 9000022a    	adrp	x10, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100004f40: 91179d4a    	add	x10, x10, #0x5e7
100004f44: eb0a02e9    	subs	x9, x23, x10
100004f48: da898529    	cneg	x9, x9, ls
100004f4c: f100bd3f    	cmp	x9, #0x2f
100004f50: 54008a69    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
100004f54: ad400540    	ldp	q0, q1, [x10]
100004f58: ad0006e0    	stp	q0, q1, [x23]
100004f5c: 3dc00940    	ldr	q0, [x10, #0x20]
100004f60: 3d800ae0    	str	q0, [x23, #0x20]
100004f64: f9007bf9    	str	x25, [sp, #0xf0]
100004f68: f85103b9    	ldur	x25, [x29, #-0xf0]
100004f6c: 5280061b    	mov	w27, #0x30              ; =48
100004f70: 17ffffad    	b	0x100004e24 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x308>
100004f74: f85103a9    	ldur	x9, [x29, #-0xf0]
100004f78: 91000939    	add	x25, x9, #0x2
100004f7c: eb08033f    	cmp	x25, x8
100004f80: 54008f83    	b.lo	0x100006170 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1654>
100004f84: 52800037    	mov	w23, #0x1               ; =1
100004f88: 17ffff3c    	b	0x100004c78 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15c>
100004f8c: b90032b6    	str	w22, [x21, #0x30]
100004f90: 5280003a    	mov	w26, #0x1               ; =1
100004f94: 14000003    	b	0x100004fa0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x484>
100004f98: 7100bd1f    	cmp	w8, #0x2f
100004f9c: 540003e0    	b.eq	0x100005018 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4fc>
100004fa0: f94016b7    	ldr	x23, [x21, #0x28]
100004fa4: b94032b9    	ldr	w25, [x21, #0x30]
100004fa8: b90032b6    	str	w22, [x21, #0x30]
100004fac: 31000b3f    	cmn	w25, #0x2
100004fb0: 540000a1    	b.ne	0x100004fc4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4a8>
100004fb4: 910042a0    	add	x0, x21, #0x10
100004fb8: 94000583    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004fbc: aa0003f7    	mov	x23, x0
100004fc0: aa0103f9    	mov	x25, x1
100004fc4: 3100073f    	cmn	w25, #0x1
100004fc8: 540008a0    	b.eq	0x1000050dc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x5c0>
100004fcc: b94032a8    	ldr	w8, [x21, #0x30]
100004fd0: 3100091f    	cmn	w8, #0x2
100004fd4: 540000a1    	b.ne	0x100004fe8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x4cc>
100004fd8: 910042a0    	add	x0, x21, #0x10
100004fdc: 9400057a    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100004fe0: f90016a0    	str	x0, [x21, #0x28]
100004fe4: b90032a1    	str	w1, [x21, #0x30]
100004fe8: b94032a8    	ldr	w8, [x21, #0x30]
100004fec: 7100ab3f    	cmp	w25, #0x2a
100004ff0: 54fffd40    	b.eq	0x100004f98 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x47c>
100004ff4: 7100bf3f    	cmp	w25, #0x2f
100004ff8: 54fffd41    	b.ne	0x100004fa0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x484>
100004ffc: 7100a91f    	cmp	w8, #0x2a
100005000: 54fffd01    	b.ne	0x100004fa0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x484>
100005004: b90032b6    	str	w22, [x21, #0x30]
100005008: b100075f    	cmn	x26, #0x1
10000500c: 54008d00    	b.eq	0x1000061ac <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1690>
100005010: 9100075a    	add	x26, x26, #0x1
100005014: 17ffffe3    	b	0x100004fa0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x484>
100005018: b90032b6    	str	w22, [x21, #0x30]
10000501c: f100075a    	subs	x26, x26, #0x1
100005020: 54fffc01    	b.ne	0x100004fa0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x484>
100005024: b1000eff    	cmn	x23, #0x3
100005028: 540091c8    	b.hi	0x100006260 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1744>
10000502c: 91000afb    	add	x27, x23, #0x2
100005030: d2802108    	mov	x8, #0x108              ; =264
100005034: f9404bf9    	ldr	x25, [sp, #0x90]
100005038: b3407d19    	bfxil	x25, x8, #0, #32
10000503c: 92800008    	mov	x8, #-0x1               ; =-1
100005040: 14000042    	b	0x100005148 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x62c>
100005044: 5100c028    	sub	w8, w1, #0x30
100005048: 7100291f    	cmp	w8, #0xa
10000504c: 540009e2    	b.hs	0x100005188 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x66c>
100005050: 7100b83f    	cmp	w1, #0x2e
100005054: 54002881    	b.ne	0x100005564 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa48>
100005058: f85103a8    	ldur	x8, [x29, #-0xf0]
10000505c: b100051f    	cmn	x8, #0x1
100005060: 54008b20    	b.eq	0x1000061c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16a8>
100005064: 91000508    	add	x8, x8, #0x1
100005068: f81303a8    	stur	x8, [x29, #-0xd0]
10000506c: d10343a1    	sub	x1, x29, #0xd0
100005070: aa1503e0    	mov	x0, x21
100005074: 52800142    	mov	w2, #0xa                ; =10
100005078: 94000512    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
10000507c: b94032a8    	ldr	w8, [x21, #0x30]
100005080: 3100091f    	cmn	w8, #0x2
100005084: 540000a1    	b.ne	0x100005098 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x57c>
100005088: 910042a0    	add	x0, x21, #0x10
10000508c: 9400054e    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005090: f90016a0    	str	x0, [x21, #0x28]
100005094: b90032a1    	str	w1, [x21, #0x30]
100005098: b94032a9    	ldr	w9, [x21, #0x30]
10000509c: 321b0128    	orr	w8, w9, #0x20
1000050a0: 7101951f    	cmp	w8, #0x65
1000050a4: 54003501    	b.ne	0x100005744 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc28>
1000050a8: f94016a8    	ldr	x8, [x21, #0x28]
1000050ac: b90032b6    	str	w22, [x21, #0x30]
1000050b0: 3100053f    	cmn	w9, #0x1
1000050b4: 540056a0    	b.eq	0x100005b88 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x106c>
1000050b8: 7102013f    	cmp	w9, #0x80
1000050bc: 54004462    	b.hs	0x100005948 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe2c>
1000050c0: 52800029    	mov	w9, #0x1                ; =1
1000050c4: 140002ae    	b	0x100005b7c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1060>
1000050c8: 52800008    	mov	w8, #0x0                ; =0
1000050cc: 5280000a    	mov	w10, #0x0               ; =0
1000050d0: d2800009    	mov	x9, #0x0                ; =0
1000050d4: 5280010b    	mov	w11, #0x8               ; =8
1000050d8: 17ffff4c    	b	0x100004e08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
1000050dc: f94006a8    	ldr	x8, [x21, #0x8]
1000050e0: f90057e8    	str	x8, [sp, #0xa8]
1000050e4: d10343a8    	sub	x8, x29, #0xd0
1000050e8: 52800340    	mov	w0, #0x1a               ; =26
1000050ec: 52800001    	mov	w1, #0x0                ; =0
1000050f0: 52800022    	mov	w2, #0x1                ; =1
1000050f4: 52800023    	mov	w3, #0x1                ; =1
1000050f8: 9400ab2d    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
1000050fc: b85303a8    	ldur	w8, [x29, #-0xd0]
100005100: 37007dc8    	tbnz	w8, #0x0, 0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x159c>
100005104: f85383a8    	ldur	x8, [x29, #-0xc8]
100005108: f100651f    	cmp	x8, #0x19
10000510c: 54007be9    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
100005110: f85403a9    	ldur	x9, [x29, #-0xc0]
100005114: f000020b    	adrp	x11, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005118: 9100816b    	add	x11, x11, #0x20
10000511c: eb0b012a    	subs	x10, x9, x11
100005120: da8a854a    	cneg	x10, x10, ls
100005124: f100655f    	cmp	x10, #0x19
100005128: 54007ba9    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
10000512c: 3dc00160    	ldr	q0, [x11]
100005130: 3d800120    	str	q0, [x9]
100005134: 3cc0a160    	ldur	q0, [x11, #0xa]
100005138: 3c80a120    	stur	q0, [x9, #0xa]
10000513c: f85103b9    	ldur	x25, [x29, #-0xf0]
100005140: 5280035b    	mov	w27, #0x1a              ; =26
100005144: f81103a9    	stur	x9, [x29, #-0xf0]
100005148: b100051f    	cmn	x8, #0x1
10000514c: f9004bf9    	str	x25, [sp, #0x90]
100005150: 54004980    	b.eq	0x100005a80 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf64>
100005154: f94057e9    	ldr	x9, [sp, #0xa8]
100005158: f9007be9    	str	x9, [sp, #0xf0]
10000515c: 17ffff31    	b	0x100004e20 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x304>
100005160: 7100897f    	cmp	w11, #0x22
100005164: 1a9f17f7    	cset	w23, eq
100005168: b100051f    	cmn	x8, #0x1
10000516c: 54008080    	b.eq	0x10000617c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1660>
100005170: 91000519    	add	x25, x8, #0x1
100005174: 17fffec1    	b	0x100004c78 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15c>
100005178: 7120003f    	cmp	w1, #0x800
10000517c: 54001fc2    	b.hs	0x100005574 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa58>
100005180: 52800048    	mov	w8, #0x2                ; =2
100005184: 140000fe    	b	0x10000557c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa60>
100005188: 7100883f    	cmp	w1, #0x22
10000518c: 540009a0    	b.eq	0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
100005190: 7100b83f    	cmp	w1, #0x2e
100005194: 54000221    	b.ne	0x1000051d8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x6bc>
100005198: b94032a8    	ldr	w8, [x21, #0x30]
10000519c: 3100091f    	cmn	w8, #0x2
1000051a0: 54000101    	b.ne	0x1000051c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x6a4>
1000051a4: 910042a0    	add	x0, x21, #0x10
1000051a8: aa0103f7    	mov	x23, x1
1000051ac: 94000506    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000051b0: aa0103e8    	mov	x8, x1
1000051b4: aa1703e1    	mov	x1, x23
1000051b8: f90016a0    	str	x0, [x21, #0x28]
1000051bc: b90032a8    	str	w8, [x21, #0x30]
1000051c0: b94032a8    	ldr	w8, [x21, #0x30]
1000051c4: 5100c108    	sub	w8, w8, #0x30
1000051c8: 7100291f    	cmp	w8, #0xa
1000051cc: 54fff423    	b.lo	0x100005050 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x534>
1000051d0: 7100883f    	cmp	w1, #0x22
1000051d4: 54000760    	b.eq	0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
1000051d8: 7102003f    	cmp	w1, #0x80
1000051dc: 54002762    	b.hs	0x1000056c8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xbac>
1000051e0: f9002bfc    	str	x28, [sp, #0x50]
1000051e4: f94006a9    	ldr	x9, [x21, #0x8]
1000051e8: f85103a8    	ldur	x8, [x29, #-0xf0]
1000051ec: f9004fe9    	str	x9, [sp, #0x98]
1000051f0: eb080128    	subs	x8, x9, x8
1000051f4: f90047e8    	str	x8, [sp, #0x88]
1000051f8: 54007963    	b.lo	0x100006124 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1608>
1000051fc: f94002aa    	ldr	x10, [x21]
100005200: 1a9f17e8    	cset	w8, eq
100005204: b900bfe8    	str	w8, [sp, #0xbc]
100005208: f00002dc    	adrp	x28, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000520c: 912ee39c    	add	x28, x28, #0xbb8
100005210: 52807b19    	mov	w25, #0x3d8             ; =984
100005214: f90043ea    	str	x10, [sp, #0x80]
100005218: a97f5f82    	ldp	x2, x23, [x28, #-0x10]
10000521c: b940bfe8    	ldr	w8, [sp, #0xbc]
100005220: f9400389    	ldr	x9, [x28]
100005224: f90077e9    	str	x9, [sp, #0xe8]
100005228: b4000157    	cbz	x23, 0x100005250 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x734>
10000522c: f85103a8    	ldur	x8, [x29, #-0xf0]
100005230: f9404fe9    	ldr	x9, [sp, #0x98]
100005234: eb08013f    	cmp	x9, x8
100005238: 54000200    	b.eq	0x100005278 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x75c>
10000523c: 39400048    	ldrb	w8, [x2]
100005240: f85103a9    	ldur	x9, [x29, #-0xf0]
100005244: 38696949    	ldrb	w9, [x10, x9]
100005248: 6b09011f    	cmp	w8, w9
10000524c: 1a9f17e8    	cset	w8, eq
100005250: 34000148    	cbz	w8, 0x100005278 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x75c>
100005254: f85103a8    	ldur	x8, [x29, #-0xf0]
100005258: 8b080140    	add	x0, x10, x8
10000525c: f94047e1    	ldr	x1, [sp, #0x88]
100005260: f90053e2    	str	x2, [sp, #0xa0]
100005264: aa1703e3    	mov	x3, x23
100005268: 9400a6fe    	bl	0x10002ee60 <<[u8]>::starts_with>
10000526c: f94043ea    	ldr	x10, [sp, #0x80]
100005270: f94053e9    	ldr	x9, [sp, #0xa0]
100005274: 37000080    	tbnz	w0, #0x0, 0x100005284 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x768>
100005278: 52801fe8    	mov	w8, #0xff               ; =255
10000527c: f90077e8    	str	x8, [sp, #0xe8]
100005280: a94da7f7    	ldp	x23, x9, [sp, #0xd8]
100005284: f94077e8    	ldr	x8, [sp, #0xe8]
100005288: 12001d08    	and	w8, w8, #0xff
10000528c: 7103fd1f    	cmp	w8, #0xff
100005290: 9a89035a    	csel	x26, x26, x9, eq
100005294: 9a97037b    	csel	x27, x27, x23, eq
100005298: 54003601    	b.ne	0x100005958 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe3c>
10000529c: 9100639c    	add	x28, x28, #0x18
1000052a0: d1006339    	sub	x25, x25, #0x18
1000052a4: a90da7f7    	stp	x23, x9, [sp, #0xd8]
1000052a8: b100633f    	cmn	x25, #0x18
1000052ac: 54fffb61    	b.ne	0x100005218 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x6fc>
1000052b0: 52801fe8    	mov	w8, #0xff               ; =255
1000052b4: 140001ab    	b	0x100005960 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe44>
1000052b8: 7100883f    	cmp	w1, #0x22
1000052bc: 540042e0    	b.eq	0x100005b18 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xffc>
1000052c0: f94016bb    	ldr	x27, [x21, #0x28]
1000052c4: b94032a1    	ldr	w1, [x21, #0x30]
1000052c8: b90032b6    	str	w22, [x21, #0x30]
1000052cc: 3100083f    	cmn	w1, #0x2
1000052d0: 54000081    	b.ne	0x1000052e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7c4>
1000052d4: 910042a0    	add	x0, x21, #0x10
1000052d8: 940004bb    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000052dc: aa0003fb    	mov	x27, x0
1000052e0: 7100843f    	cmp	w1, #0x21
1000052e4: 540008ad    	b.le	0x1000053f8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x8dc>
1000052e8: 7101703f    	cmp	w1, #0x5c
1000052ec: 54fffe61    	b.ne	0x1000052b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x79c>
1000052f0: f94016a0    	ldr	x0, [x21, #0x28]
1000052f4: b94032a1    	ldr	w1, [x21, #0x30]
1000052f8: b90032b6    	str	w22, [x21, #0x30]
1000052fc: 3100083f    	cmn	w1, #0x2
100005300: 54000061    	b.ne	0x10000530c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7f0>
100005304: 910042a0    	add	x0, x21, #0x10
100005308: 940004af    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
10000530c: 71016c3f    	cmp	w1, #0x5b
100005310: 5400082d    	b.le	0x100005414 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x8f8>
100005314: 51017028    	sub	w8, w1, #0x5c
100005318: 7100711f    	cmp	w8, #0x1c
10000531c: 540060a8    	b.hi	0x100005f30 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1414>
100005320: 52800029    	mov	w9, #0x1                ; =1
100005324: 72a02889    	movk	w9, #0x144, lsl #16
100005328: 1ac82529    	lsr	w9, w9, w8
10000532c: 3707fca9    	tbnz	w9, #0x0, 0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
100005330: 7100711f    	cmp	w8, #0x1c
100005334: 54005981    	b.ne	0x100005e64 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1348>
100005338: f94016bb    	ldr	x27, [x21, #0x28]
10000533c: b94032a1    	ldr	w1, [x21, #0x30]
100005340: b90032b6    	str	w22, [x21, #0x30]
100005344: 3100083f    	cmn	w1, #0x2
100005348: 54000081    	b.ne	0x100005358 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x83c>
10000534c: 910042a0    	add	x0, x21, #0x10
100005350: 9400049d    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005354: aa0003fb    	mov	x27, x0
100005358: 3100043f    	cmn	w1, #0x1
10000535c: 54000660    	b.eq	0x100005428 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x90c>
100005360: 5100c028    	sub	w8, w1, #0x30
100005364: 7100291f    	cmp	w8, #0xa
100005368: 1a9f27e8    	cset	w8, lo
10000536c: 71011c3f    	cmp	w1, #0x47
100005370: 1a9f27e9    	cset	w9, lo
100005374: 7101003f    	cmp	w1, #0x40
100005378: 1a888128    	csel	w8, w9, w8, hi
10000537c: 71019c3f    	cmp	w1, #0x67
100005380: 1a9f27e9    	cset	w9, lo
100005384: 7101803f    	cmp	w1, #0x60
100005388: 1a888128    	csel	w8, w9, w8, hi
10000538c: 34000848    	cbz	w8, 0x100005494 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x978>
100005390: f94016bb    	ldr	x27, [x21, #0x28]
100005394: b94032a1    	ldr	w1, [x21, #0x30]
100005398: b90032b6    	str	w22, [x21, #0x30]
10000539c: 3100083f    	cmn	w1, #0x2
1000053a0: 54000081    	b.ne	0x1000053b0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x894>
1000053a4: 910042a0    	add	x0, x21, #0x10
1000053a8: 94000487    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000053ac: aa0003fb    	mov	x27, x0
1000053b0: 3100043f    	cmn	w1, #0x1
1000053b4: 540003a0    	b.eq	0x100005428 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x90c>
1000053b8: 5100c028    	sub	w8, w1, #0x30
1000053bc: 7100291f    	cmp	w8, #0xa
1000053c0: 1a9f27e8    	cset	w8, lo
1000053c4: 71011c3f    	cmp	w1, #0x47
1000053c8: 1a9f27e9    	cset	w9, lo
1000053cc: 7101003f    	cmp	w1, #0x40
1000053d0: 1a888128    	csel	w8, w9, w8, hi
1000053d4: 71019c3f    	cmp	w1, #0x67
1000053d8: 1a9f27e9    	cset	w9, lo
1000053dc: 7101803f    	cmp	w1, #0x60
1000053e0: 1a888128    	csel	w8, w9, w8, hi
1000053e4: 36000588    	tbz	w8, #0x0, 0x100005494 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x978>
1000053e8: 92800008    	mov	x8, #-0x1               ; =-1
1000053ec: a9442ffb    	ldp	x27, x11, [sp, #0x40]
1000053f0: f9401fe9    	ldr	x9, [sp, #0x38]
1000053f4: 1400004c    	b	0x100005524 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa08>
1000053f8: 3100043f    	cmn	w1, #0x1
1000053fc: 540034c0    	b.eq	0x100005a94 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf78>
100005400: 7100283f    	cmp	w1, #0xa
100005404: 54001a40    	b.eq	0x10000574c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc30>
100005408: 7100343f    	cmp	w1, #0xd
10000540c: 54fff5a1    	b.ne	0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
100005410: 140000cf    	b	0x10000574c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc30>
100005414: 7100883f    	cmp	w1, #0x22
100005418: 54fff540    	b.eq	0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
10000541c: 7100c03f    	cmp	w1, #0x30
100005420: 54fff500    	b.eq	0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
100005424: 14000288    	b	0x100005e44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1328>
100005428: f94006a8    	ldr	x8, [x21, #0x8]
10000542c: f9003be8    	str	x8, [sp, #0x70]
100005430: d10343a8    	sub	x8, x29, #0xd0
100005434: 528002e0    	mov	w0, #0x17               ; =23
100005438: 52800001    	mov	w1, #0x0                ; =0
10000543c: 52800022    	mov	w2, #0x1                ; =1
100005440: 52800023    	mov	w3, #0x1                ; =1
100005444: 9400aa5a    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005448: f85303a8    	ldur	x8, [x29, #-0xd0]
10000544c: b5006628    	cbnz	x8, 0x100006110 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
100005450: f85383a8    	ldur	x8, [x29, #-0xc8]
100005454: f100591f    	cmp	x8, #0x16
100005458: 54006189    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
10000545c: f85403ab    	ldur	x11, [x29, #-0xc0]
100005460: f000020a    	adrp	x10, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005464: 91014d4a    	add	x10, x10, #0x53
100005468: eb0a0169    	subs	x9, x11, x10
10000546c: da898529    	cneg	x9, x9, ls
100005470: f100593f    	cmp	x9, #0x16
100005474: 54006149    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
100005478: 3dc00140    	ldr	q0, [x10]
10000547c: 3d800160    	str	q0, [x11]
100005480: f840f149    	ldur	x9, [x10, #0xf]
100005484: f800f169    	stur	x9, [x11, #0xf]
100005488: f85103bb    	ldur	x27, [x29, #-0xf0]
10000548c: 528002e9    	mov	w9, #0x17               ; =23
100005490: 14000025    	b	0x100005524 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa08>
100005494: 7102003f    	cmp	w1, #0x80
100005498: 54000062    	b.hs	0x1000054a4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x988>
10000549c: 52800028    	mov	w8, #0x1                ; =1
1000054a0: 14000007    	b	0x1000054bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x9a0>
1000054a4: 7120003f    	cmp	w1, #0x800
1000054a8: 54000062    	b.hs	0x1000054b4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x998>
1000054ac: 52800048    	mov	w8, #0x2                ; =2
1000054b0: 14000003    	b	0x1000054bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x9a0>
1000054b4: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
1000054b8: 9a983708    	cinc	x8, x24, hs
1000054bc: ab1b0108    	adds	x8, x8, x27
1000054c0: f9003be8    	str	x8, [sp, #0x70]
1000054c4: 54006a42    	b.hs	0x10000620c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16f0>
1000054c8: d10343a8    	sub	x8, x29, #0xd0
1000054cc: 52800300    	mov	w0, #0x18               ; =24
1000054d0: 52800001    	mov	w1, #0x0                ; =0
1000054d4: 52800022    	mov	w2, #0x1                ; =1
1000054d8: 52800023    	mov	w3, #0x1                ; =1
1000054dc: 9400aa34    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
1000054e0: b85303a8    	ldur	w8, [x29, #-0xd0]
1000054e4: 37006168    	tbnz	w8, #0x0, 0x100006110 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15f4>
1000054e8: f85383a8    	ldur	x8, [x29, #-0xc8]
1000054ec: f1005d1f    	cmp	x8, #0x17
1000054f0: 54005cc9    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
1000054f4: f85403ab    	ldur	x11, [x29, #-0xc0]
1000054f8: f000020a    	adrp	x10, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
1000054fc: 9100ed4a    	add	x10, x10, #0x3b
100005500: eb0a0169    	subs	x9, x11, x10
100005504: da898529    	cneg	x9, x9, ls
100005508: f1005d3f    	cmp	x9, #0x17
10000550c: 54005c89    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
100005510: 3dc00140    	ldr	q0, [x10]
100005514: 3d800160    	str	q0, [x11]
100005518: f9400949    	ldr	x9, [x10, #0x10]
10000551c: f9000969    	str	x9, [x11, #0x10]
100005520: 52800309    	mov	w9, #0x18               ; =24
100005524: b100051f    	cmn	x8, #0x1
100005528: a9042ffb    	stp	x27, x11, [sp, #0x40]
10000552c: f9001fe9    	str	x9, [sp, #0x38]
100005530: 54000080    	b.eq	0x100005540 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa24>
100005534: a90227eb    	stp	x11, x9, [sp, #0x20]
100005538: f9403be9    	ldr	x9, [sp, #0x70]
10000553c: a9016fe9    	stp	x9, x27, [sp, #0x10]
100005540: b100051f    	cmn	x8, #0x1
100005544: 54ffebe0    	b.eq	0x1000052c0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x7a4>
100005548: f94017ea    	ldr	x10, [sp, #0x28]
10000554c: a9412fe9    	ldp	x9, x11, [sp, #0x10]
100005550: a9342faa    	stp	x10, x11, [x29, #-0xc0]
100005554: f81503a9    	stur	x9, [x29, #-0xb0]
100005558: f94013e9    	ldr	x9, [sp, #0x20]
10000555c: f81383a9    	stur	x9, [x29, #-0xc8]
100005560: 14000173    	b	0x100005b2c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1010>
100005564: 7102003f    	cmp	w1, #0x80
100005568: 54000a82    	b.hs	0x1000056b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb9c>
10000556c: 52800028    	mov	w8, #0x1                ; =1
100005570: 1400007d    	b	0x100005764 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc48>
100005574: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005578: 9a983708    	cinc	x8, x24, hs
10000557c: f85103a9    	ldur	x9, [x29, #-0xf0]
100005580: ab09011b    	adds	x27, x8, x9
100005584: 54006022    	b.hs	0x100006188 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x166c>
100005588: b94032a8    	ldr	w8, [x21, #0x30]
10000558c: 3100091f    	cmn	w8, #0x2
100005590: 540000a1    	b.ne	0x1000055a4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa88>
100005594: 910042a0    	add	x0, x21, #0x10
100005598: 9400040b    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
10000559c: f90016a0    	str	x0, [x21, #0x28]
1000055a0: b90032a1    	str	w1, [x21, #0x30]
1000055a4: b94032a9    	ldr	w9, [x21, #0x30]
1000055a8: f94016a8    	ldr	x8, [x21, #0x28]
1000055ac: 71017d3f    	cmp	w9, #0x5f
1000055b0: 540001e0    	b.eq	0x1000055ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0xad0>
1000055b4: 3100053f    	cmn	w9, #0x1
1000055b8: 540002e0    	b.eq	0x100005614 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xaf8>
1000055bc: 5100c12a    	sub	w10, w9, #0x30
1000055c0: 7100295f    	cmp	w10, #0xa
1000055c4: 1a9f27ea    	cset	w10, lo
1000055c8: 71016d3f    	cmp	w9, #0x5b
1000055cc: 1a9f27eb    	cset	w11, lo
1000055d0: 7101013f    	cmp	w9, #0x40
1000055d4: 1a8a816a    	csel	w10, w11, w10, hi
1000055d8: 7101ed3f    	cmp	w9, #0x7b
1000055dc: 1a9f27eb    	cset	w11, lo
1000055e0: 7101813f    	cmp	w9, #0x60
1000055e4: 1a8a816a    	csel	w10, w11, w10, hi
1000055e8: 3400016a    	cbz	w10, 0x100005614 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xaf8>
1000055ec: 7140413f    	cmp	w9, #0x10, lsl #12      ; =0x10000
1000055f0: 9a98370a    	cinc	x10, x24, hs
1000055f4: 7120013f    	cmp	w9, #0x800
1000055f8: 9a8a328a    	csel	x10, x20, x10, lo
1000055fc: 7102013f    	cmp	w9, #0x80
100005600: 9a9f2549    	csinc	x9, x10, xzr, hs
100005604: ab08013b    	adds	x27, x9, x8
100005608: 540059c2    	b.hs	0x100006140 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1624>
10000560c: b90032b6    	str	w22, [x21, #0x30]
100005610: 17ffffde    	b	0x100005588 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xa6c>
100005614: a94006a0    	ldp	x0, x1, [x21]
100005618: eb01037f    	cmp	x27, x1
10000561c: f85103a9    	ldur	x9, [x29, #-0xf0]
100005620: fa499360    	ccmp	x27, x9, #0x0, ls
100005624: 54000082    	b.hs	0x100005634 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb18>
100005628: d2800009    	mov	x9, #0x0                ; =0
10000562c: b5000289    	cbnz	x9, 0x10000567c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb60>
100005630: 140002b4    	b	0x100006100 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15e4>
100005634: eb01013f    	cmp	x9, x1
100005638: 54000140    	b.eq	0x100005660 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb44>
10000563c: b4000089    	cbz	x9, 0x10000564c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb30>
100005640: 38e96808    	ldrsb	w8, [x0, x9]
100005644: 3101051f    	cmn	w8, #0x41
100005648: 54ffff0d    	b.le	0x100005628 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb0c>
10000564c: eb01037f    	cmp	x27, x1
100005650: 54000080    	b.eq	0x100005660 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb44>
100005654: 38fb6808    	ldrsb	w8, [x0, x27]
100005658: 3101011f    	cmn	w8, #0x40
10000565c: 54fffe6b    	b.lt	0x100005628 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xb0c>
100005660: eb090368    	subs	x8, x27, x9
100005664: 54005043    	b.lo	0x10000606c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1550>
100005668: eb01037f    	cmp	x27, x1
10000566c: 54005008    	b.hi	0x10000606c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1550>
100005670: f85103a9    	ldur	x9, [x29, #-0xf0]
100005674: 8b090009    	add	x9, x0, x9
100005678: b4005449    	cbz	x9, 0x100006100 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15e4>
10000567c: aa0903e0    	mov	x0, x9
100005680: aa0803e1    	mov	x1, x8
100005684: 94001474    	bl	0x10000a854 <<tokens::token_stream::token_kind::keyword_kind::KeywordKind>::from_lexeme>
100005688: 52800008    	mov	w8, #0x0                ; =0
10000568c: d2800009    	mov	x9, #0x0                ; =0
100005690: 12001c0a    	and	w10, w0, #0xff
100005694: 528000ab    	mov	w11, #0x5               ; =5
100005698: 33181c0b    	bfi	w11, w0, #8, #8
10000569c: 7103fd5f    	cmp	w10, #0xff
1000056a0: 1a8b014a    	csel	w10, w10, w11, eq
1000056a4: 52801f4b    	mov	w11, #0xfa              ; =250
1000056a8: 6a2a017f    	bics	wzr, w11, w10
1000056ac: 1a8a028b    	csel	w11, w20, w10, eq
1000056b0: 1218196a    	and	w10, w11, #0x7f00
1000056b4: 17fffdd5    	b	0x100004e08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
1000056b8: 7120003f    	cmp	w1, #0x800
1000056bc: 54000502    	b.hs	0x10000575c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc40>
1000056c0: 52800048    	mov	w8, #0x2                ; =2
1000056c4: 14000028    	b	0x100005764 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc48>
1000056c8: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
1000056cc: 9a983708    	cinc	x8, x24, hs
1000056d0: 7120003f    	cmp	w1, #0x800
1000056d4: 9a883288    	csel	x8, x20, x8, lo
1000056d8: f85103a9    	ldur	x9, [x29, #-0xf0]
1000056dc: ab090108    	adds	x8, x8, x9
1000056e0: f9007be8    	str	x8, [sp, #0xf0]
1000056e4: 540057c2    	b.hs	0x1000061dc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16c0>
1000056e8: d10343a8    	sub	x8, x29, #0xd0
1000056ec: 52800600    	mov	w0, #0x30               ; =48
1000056f0: 52800001    	mov	w1, #0x0                ; =0
1000056f4: 52800022    	mov	w2, #0x1                ; =1
1000056f8: 52800023    	mov	w3, #0x1                ; =1
1000056fc: 9400a9ac    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005700: f85303a8    	ldur	x8, [x29, #-0xd0]
100005704: b5004da8    	cbnz	x8, 0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x159c>
100005708: f85383a8    	ldur	x8, [x29, #-0xc8]
10000570c: f100bd1f    	cmp	x8, #0x2f
100005710: f000020a    	adrp	x10, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005714: 91179d4a    	add	x10, x10, #0x5e7
100005718: 54004b89    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
10000571c: f85403b7    	ldur	x23, [x29, #-0xc0]
100005720: eb0a02e9    	subs	x9, x23, x10
100005724: da898529    	cneg	x9, x9, ls
100005728: f100bd3f    	cmp	x9, #0x2f
10000572c: 54004b89    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
100005730: ad400540    	ldp	q0, q1, [x10]
100005734: ad0006e0    	stp	q0, q1, [x23]
100005738: 3dc00940    	ldr	q0, [x10, #0x20]
10000573c: 3d800ae0    	str	q0, [x23, #0x20]
100005740: 17fffe0a    	b	0x100004f68 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x44c>
100005744: 52800037    	mov	w23, #0x1               ; =1
100005748: 14000171    	b	0x100005d0c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11f0>
10000574c: 7102003f    	cmp	w1, #0x80
100005750: 54001dc2    	b.hs	0x100005b08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xfec>
100005754: 52800028    	mov	w8, #0x1                ; =1
100005758: 14000125    	b	0x100005bec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10d0>
10000575c: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005760: 9a983708    	cinc	x8, x24, hs
100005764: f85103a9    	ldur	x9, [x29, #-0xf0]
100005768: ab090117    	adds	x23, x8, x9
10000576c: 54005262    	b.hs	0x1000061b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x169c>
100005770: f81783b7    	stur	x23, [x29, #-0x88]
100005774: 7100c03f    	cmp	w1, #0x30
100005778: 540003a1    	b.ne	0x1000057ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcd0>
10000577c: b94032a8    	ldr	w8, [x21, #0x30]
100005780: 3100091f    	cmn	w8, #0x2
100005784: 540000a1    	b.ne	0x100005798 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xc7c>
100005788: 910042a0    	add	x0, x21, #0x10
10000578c: 9400038e    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005790: f90016a0    	str	x0, [x21, #0x28]
100005794: b90032a1    	str	w1, [x21, #0x30]
100005798: b94032a8    	ldr	w8, [x21, #0x30]
10000579c: 51010909    	sub	w9, w8, #0x42
1000057a0: 7100d93f    	cmp	w9, #0x36
1000057a4: 54000248    	b.hi	0x1000057ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcd0>
1000057a8: b20003ea    	mov	x10, #0x100000001       ; =4294967297
1000057ac: 9ac9254a    	lsr	x10, x10, x9
1000057b0: 37001d2a    	tbnz	w10, #0x0, 0x100005b54 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1038>
1000057b4: b21303ea    	mov	x10, #0x200000002000    ; =35184372097024
1000057b8: 9ac9254a    	lsr	x10, x10, x9
1000057bc: 37001bca    	tbnz	w10, #0x0, 0x100005b34 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1018>
1000057c0: b20a03ea    	mov	x10, #0x40000000400000  ; =18014398513676288
1000057c4: 9ac92549    	lsr	x9, x10, x9
1000057c8: 36000129    	tbz	w9, #0x0, 0x1000057ec <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcd0>
1000057cc: f94016a9    	ldr	x9, [x21, #0x28]
1000057d0: b90032b6    	str	w22, [x21, #0x30]
1000057d4: 3100051f    	cmn	w8, #0x1
1000057d8: 54003800    	b.eq	0x100005ed8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13bc>
1000057dc: 7102011f    	cmp	w8, #0x80
1000057e0: 54002ec2    	b.hs	0x100005db8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x129c>
1000057e4: 52800028    	mov	w8, #0x1                ; =1
1000057e8: 140001ba    	b	0x100005ed0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13b4>
1000057ec: d10223a1    	sub	x1, x29, #0x88
1000057f0: aa1503e0    	mov	x0, x21
1000057f4: 52800142    	mov	w2, #0xa                ; =10
1000057f8: 94000332    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
1000057fc: 52800017    	mov	w23, #0x0               ; =0
100005800: 52a00059    	mov	w25, #0x20000           ; =131072
100005804: 5280003b    	mov	w27, #0x1               ; =1
100005808: 340005bb    	cbz	w27, 0x1000058bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xda0>
10000580c: b94032a8    	ldr	w8, [x21, #0x30]
100005810: 3100091f    	cmn	w8, #0x2
100005814: 540000a1    	b.ne	0x100005828 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd0c>
100005818: 910042a0    	add	x0, x21, #0x10
10000581c: 9400036a    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005820: f90016a0    	str	x0, [x21, #0x28]
100005824: b90032a1    	str	w1, [x21, #0x30]
100005828: b94032a8    	ldr	w8, [x21, #0x30]
10000582c: 7100b91f    	cmp	w8, #0x2e
100005830: 54000461    	b.ne	0x1000058bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xda0>
100005834: 3dc006a0    	ldr	q0, [x21, #0x10]
100005838: a9426aa8    	ldp	x8, x26, [x21, #0x20]
10000583c: 3c9303a0    	stur	q0, [x29, #-0xd0]
100005840: a9346ba8    	stp	x8, x26, [x29, #-0xc0]
100005844: b81503b6    	stur	w22, [x29, #-0xb0]
100005848: d10343a0    	sub	x0, x29, #0xd0
10000584c: 9400035e    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005850: 5100c028    	sub	w8, w1, #0x30
100005854: 7100291f    	cmp	w8, #0xa
100005858: 540001a3    	b.lo	0x10000588c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd70>
10000585c: 7100b83f    	cmp	w1, #0x2e
100005860: 540002e0    	b.eq	0x1000058bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xda0>
100005864: 3100043f    	cmn	w1, #0x1
100005868: 54000120    	b.eq	0x10000588c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xd70>
10000586c: 529ffbe8    	mov	w8, #0xffdf             ; =65503
100005870: 72a003e8    	movk	w8, #0x1f, lsl #16
100005874: 0a080028    	and	w8, w1, w8
100005878: 51016d08    	sub	w8, w8, #0x5b
10000587c: 3100691f    	cmn	w8, #0x1a
100005880: 52800be8    	mov	w8, #0x5f               ; =95
100005884: 7a483024    	ccmp	w1, w8, #0x4, lo
100005888: 540001a0    	b.eq	0x1000058bc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xda0>
10000588c: b90032b6    	str	w22, [x21, #0x30]
100005890: b100075f    	cmn	x26, #0x1
100005894: 54004c80    	b.eq	0x100006224 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1708>
100005898: 91000748    	add	x8, x26, #0x1
10000589c: f81783a8    	stur	x8, [x29, #-0x88]
1000058a0: d10223a1    	sub	x1, x29, #0x88
1000058a4: aa1503e0    	mov	x0, x21
1000058a8: 52800142    	mov	w2, #0xa                ; =10
1000058ac: 94000305    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
1000058b0: 5280003a    	mov	w26, #0x1               ; =1
1000058b4: 3700009b    	tbnz	w27, #0x0, 0x1000058c4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xda8>
1000058b8: 14000016    	b	0x100005910 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdf4>
1000058bc: 5280001a    	mov	w26, #0x0               ; =0
1000058c0: 3600029b    	tbz	w27, #0x0, 0x100005910 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdf4>
1000058c4: b94032a8    	ldr	w8, [x21, #0x30]
1000058c8: 3100091f    	cmn	w8, #0x2
1000058cc: 540000a1    	b.ne	0x1000058e0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdc4>
1000058d0: 910042a0    	add	x0, x21, #0x10
1000058d4: 9400033c    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
1000058d8: f90016a0    	str	x0, [x21, #0x28]
1000058dc: b90032a1    	str	w1, [x21, #0x30]
1000058e0: b94032a9    	ldr	w9, [x21, #0x30]
1000058e4: 321b0128    	orr	w8, w9, #0x20
1000058e8: 7101951f    	cmp	w8, #0x65
1000058ec: 54000121    	b.ne	0x100005910 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xdf4>
1000058f0: f94016a8    	ldr	x8, [x21, #0x28]
1000058f4: b90032b6    	str	w22, [x21, #0x30]
1000058f8: 3100053f    	cmn	w9, #0x1
1000058fc: 54002300    	b.eq	0x100005d5c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1240>
100005900: 7102013f    	cmp	w9, #0x80
100005904: 54001682    	b.hs	0x100005bd4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10b8>
100005908: 52800029    	mov	w9, #0x1                ; =1
10000590c: 14000111    	b	0x100005d50 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1234>
100005910: d10223a1    	sub	x1, x29, #0x88
100005914: aa1503e0    	mov	x0, x21
100005918: 94000289    	bl	0x10000633c <<source_files_to_tokens::lexer::Lexer>::consume_literal_suffix>
10000591c: 3600009a    	tbz	w26, #0x0, 0x10000592c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xe10>
100005920: 52800008    	mov	w8, #0x0                ; =0
100005924: 52800029    	mov	w9, #0x1                ; =1
100005928: 1400013d    	b	0x100005e1c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1300>
10000592c: 7100001f    	cmp	w0, #0x0
100005930: 52802008    	mov	w8, #0x100              ; =256
100005934: 1a9f1108    	csel	w8, w8, wzr, ne
100005938: 2a170329    	orr	w9, w25, w23
10000593c: 2a080128    	orr	w8, w9, w8
100005940: 53185d0a    	lsl	w10, w8, #8
100005944: 1400013b    	b	0x100005e30 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1314>
100005948: 7120013f    	cmp	w9, #0x800
10000594c: 54001142    	b.hs	0x100005b74 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1058>
100005950: 52800049    	mov	w9, #0x2                ; =2
100005954: 1400008a    	b	0x100005b7c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1060>
100005958: f9006bfb    	str	x27, [sp, #0xd0]
10000595c: f9005bfa    	str	x26, [sp, #0xb0]
100005960: 7103fd1f    	cmp	w8, #0xff
100005964: f9402bfc    	ldr	x28, [sp, #0x50]
100005968: 54000401    	b.ne	0x1000059e8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xecc>
10000596c: f85103a8    	ldur	x8, [x29, #-0xf0]
100005970: b100051f    	cmn	x8, #0x1
100005974: 540045e0    	b.eq	0x100006230 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1714>
100005978: d10343a8    	sub	x8, x29, #0xd0
10000597c: 52800600    	mov	w0, #0x30               ; =48
100005980: 52800001    	mov	w1, #0x0                ; =0
100005984: 52800022    	mov	w2, #0x1                ; =1
100005988: 52800023    	mov	w3, #0x1                ; =1
10000598c: 9400a908    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005990: b85303a8    	ldur	w8, [x29, #-0xd0]
100005994: 37003928    	tbnz	w8, #0x0, 0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x159c>
100005998: f85383a8    	ldur	x8, [x29, #-0xc8]
10000599c: f100bd1f    	cmp	x8, #0x2f
1000059a0: f000020b    	adrp	x11, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
1000059a4: 91179d6b    	add	x11, x11, #0x5e7
1000059a8: 54003709    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
1000059ac: f85403a9    	ldur	x9, [x29, #-0xc0]
1000059b0: eb0b012a    	subs	x10, x9, x11
1000059b4: da8a854a    	cneg	x10, x10, ls
1000059b8: f100bd5f    	cmp	x10, #0x2f
1000059bc: 54003709    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
1000059c0: f85103b9    	ldur	x25, [x29, #-0xf0]
1000059c4: 9100072a    	add	x10, x25, #0x1
1000059c8: f90007ea    	str	x10, [sp, #0x8]
1000059cc: ad400560    	ldp	q0, q1, [x11]
1000059d0: ad000520    	stp	q0, q1, [x9]
1000059d4: 3dc00960    	ldr	q0, [x11, #0x20]
1000059d8: 3d800920    	str	q0, [x9, #0x20]
1000059dc: 5280061b    	mov	w27, #0x30              ; =48
1000059e0: f81103a9    	stur	x9, [x29, #-0xf0]
1000059e4: 14000022    	b	0x100005a6c <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf50>
1000059e8: aa0803f9    	mov	x25, x8
1000059ec: f9406be8    	ldr	x8, [sp, #0xd0]
1000059f0: b7f843e8    	tbnz	x8, #0x3f, 0x10000626c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1750>
1000059f4: 52800037    	mov	w23, #0x1               ; =1
1000059f8: 14000002    	b	0x100005a00 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xee4>
1000059fc: 910006f7    	add	x23, x23, #0x1
100005a00: f9406be1    	ldr	x1, [sp, #0xd0]
100005a04: f100803f    	cmp	x1, #0x20
100005a08: 54000082    	b.hs	0x100005a18 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xefc>
100005a0c: f9405be0    	ldr	x0, [sp, #0xb0]
100005a10: 9400cfef    	bl	0x1000399cc <core::str::count::char_count_general_case>
100005a14: 14000003    	b	0x100005a20 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf04>
100005a18: f9405be0    	ldr	x0, [sp, #0xb0]
100005a1c: 9400ce28    	bl	0x1000392bc <core::str::count::do_count_chars>
100005a20: eb0002ff    	cmp	x23, x0
100005a24: 54000102    	b.hs	0x100005a44 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf28>
100005a28: b94032a8    	ldr	w8, [x21, #0x30]
100005a2c: b90032b6    	str	w22, [x21, #0x30]
100005a30: 3100091f    	cmn	w8, #0x2
100005a34: 54fffe41    	b.ne	0x1000059fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xee0>
100005a38: 910042a0    	add	x0, x21, #0x10
100005a3c: 940002e2    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005a40: 17ffffef    	b	0x1000059fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0xee0>
100005a44: f9406be8    	ldr	x8, [sp, #0xd0]
100005a48: f85103a9    	ldur	x9, [x29, #-0xf0]
100005a4c: ab09011b    	adds	x27, x8, x9
100005a50: 54003f62    	b.hs	0x10000623c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1720>
100005a54: 2a1903e8    	mov	w8, w25
100005a58: f9401be9    	ldr	x9, [sp, #0x30]
100005a5c: 92607d29    	and	x9, x9, #0xffffffff00000000
100005a60: aa082128    	orr	x8, x9, x8, lsl #8
100005a64: b27f0519    	orr	x25, x8, #0x6
100005a68: 92800008    	mov	x8, #-0x1               ; =-1
100005a6c: b100051f    	cmn	x8, #0x1
100005a70: f9001bf9    	str	x25, [sp, #0x30]
100005a74: 54000060    	b.eq	0x100005a80 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xf64>
100005a78: f94007e9    	ldr	x9, [sp, #0x8]
100005a7c: 17fffdb7    	b	0x100005158 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x63c>
100005a80: d360ff29    	lsr	x9, x25, #32
100005a84: 12103f28    	and	w8, w25, #0xffff0000
100005a88: aa1903ea    	mov	x10, x25
100005a8c: aa1903eb    	mov	x11, x25
100005a90: 17fffcde    	b	0x100004e08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
100005a94: f94006b7    	ldr	x23, [x21, #0x8]
100005a98: d10343a8    	sub	x8, x29, #0xd0
100005a9c: 52800360    	mov	w0, #0x1b               ; =27
100005aa0: 52800001    	mov	w1, #0x0                ; =0
100005aa4: 52800022    	mov	w2, #0x1                ; =1
100005aa8: 52800023    	mov	w3, #0x1                ; =1
100005aac: 9400a8c0    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005ab0: b85303a8    	ldur	w8, [x29, #-0xd0]
100005ab4: 37003028    	tbnz	w8, #0x0, 0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x159c>
100005ab8: f85383a8    	ldur	x8, [x29, #-0xc8]
100005abc: f100691f    	cmp	x8, #0x1a
100005ac0: 54002e49    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
100005ac4: f85403a9    	ldur	x9, [x29, #-0xc0]
100005ac8: f000020b    	adrp	x11, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005acc: 9102116b    	add	x11, x11, #0x84
100005ad0: eb0b012a    	subs	x10, x9, x11
100005ad4: da8a854a    	cneg	x10, x10, ls
100005ad8: f100695f    	cmp	x10, #0x1a
100005adc: 54002e09    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
100005ae0: 3dc00160    	ldr	q0, [x11]
100005ae4: 3d800120    	str	q0, [x9]
100005ae8: 3cc0b160    	ldur	q0, [x11, #0xb]
100005aec: 3c80b120    	stur	q0, [x9, #0xb]
100005af0: a93327a8    	stp	x8, x9, [x29, #-0xd0]
100005af4: 52800368    	mov	w8, #0x1b               ; =27
100005af8: f81403a8    	stur	x8, [x29, #-0xc0]
100005afc: f85103a8    	ldur	x8, [x29, #-0xf0]
100005b00: a934dfa8    	stp	x8, x23, [x29, #-0xb8]
100005b04: 14000056    	b	0x100005c5c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1140>
100005b08: 7120003f    	cmp	w1, #0x800
100005b0c: 540006c2    	b.hs	0x100005be4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10c8>
100005b10: 52800048    	mov	w8, #0x2                ; =2
100005b14: 14000036    	b	0x100005bec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10d0>
100005b18: b100077f    	cmn	x27, #0x1
100005b1c: 54003960    	b.eq	0x100006248 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x172c>
100005b20: 91000768    	add	x8, x27, #0x1
100005b24: f81383a8    	stur	x8, [x29, #-0xc8]
100005b28: 92800008    	mov	x8, #-0x1               ; =-1
100005b2c: f81303a8    	stur	x8, [x29, #-0xd0]
100005b30: 1400004b    	b	0x100005c5c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1140>
100005b34: f94016a9    	ldr	x9, [x21, #0x28]
100005b38: b90032b6    	str	w22, [x21, #0x30]
100005b3c: 3100051f    	cmn	w8, #0x1
100005b40: 54001b20    	b.eq	0x100005ea4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1388>
100005b44: 7102011f    	cmp	w8, #0x80
100005b48: 54001302    	b.hs	0x100005da8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x128c>
100005b4c: 52800028    	mov	w8, #0x1                ; =1
100005b50: 140000d3    	b	0x100005e9c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1380>
100005b54: f94016a9    	ldr	x9, [x21, #0x28]
100005b58: b90032b6    	str	w22, [x21, #0x30]
100005b5c: 3100051f    	cmn	w8, #0x1
100005b60: 54001d60    	b.eq	0x100005f0c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13f0>
100005b64: 7102011f    	cmp	w8, #0x80
100005b68: 54001302    	b.hs	0x100005dc8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12ac>
100005b6c: 52800028    	mov	w8, #0x1                ; =1
100005b70: 140000e5    	b	0x100005f04 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13e8>
100005b74: 7140413f    	cmp	w9, #0x10, lsl #12      ; =0x10000
100005b78: 9a983709    	cinc	x9, x24, hs
100005b7c: ab080128    	adds	x8, x9, x8
100005b80: 54003282    	b.hs	0x1000061d0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16b4>
100005b84: f81303a8    	stur	x8, [x29, #-0xd0]
100005b88: 910042a0    	add	x0, x21, #0x10
100005b8c: 9400028e    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005b90: f90016a0    	str	x0, [x21, #0x28]
100005b94: b90032a1    	str	w1, [x21, #0x30]
100005b98: 7100b43f    	cmp	w1, #0x2d
100005b9c: 54000060    	b.eq	0x100005ba8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x108c>
100005ba0: 7100ac3f    	cmp	w1, #0x2b
100005ba4: 54000a21    	b.ne	0x100005ce8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11cc>
100005ba8: b90032b6    	str	w22, [x21, #0x30]
100005bac: 3100043f    	cmn	w1, #0x1
100005bb0: 540009c0    	b.eq	0x100005ce8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11cc>
100005bb4: 7102003f    	cmp	w1, #0x80
100005bb8: 54000062    	b.hs	0x100005bc4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x10a8>
100005bbc: 52800028    	mov	w8, #0x1                ; =1
100005bc0: 14000047    	b	0x100005cdc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11c0>
100005bc4: 7120003f    	cmp	w1, #0x800
100005bc8: 54000862    	b.hs	0x100005cd4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11b8>
100005bcc: 52800048    	mov	w8, #0x2                ; =2
100005bd0: 14000043    	b	0x100005cdc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11c0>
100005bd4: 7120013f    	cmp	w9, #0x800
100005bd8: 54000b82    	b.hs	0x100005d48 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x122c>
100005bdc: 52800049    	mov	w9, #0x2                ; =2
100005be0: 1400005c    	b	0x100005d50 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1234>
100005be4: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005be8: 9a983708    	cinc	x8, x24, hs
100005bec: ab1b0117    	adds	x23, x8, x27
100005bf0: 54002fc2    	b.hs	0x1000061e8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16cc>
100005bf4: d10343a8    	sub	x8, x29, #0xd0
100005bf8: 52800600    	mov	w0, #0x30               ; =48
100005bfc: 52800001    	mov	w1, #0x0                ; =0
100005c00: 52800022    	mov	w2, #0x1                ; =1
100005c04: 52800023    	mov	w3, #0x1                ; =1
100005c08: 9400a869    	bl	0x10002fdac <<alloc::raw_vec::RawVecInner>::try_allocate_in>
100005c0c: b85303a8    	ldur	w8, [x29, #-0xd0]
100005c10: 37002548    	tbnz	w8, #0x0, 0x1000060b8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x159c>
100005c14: f85383a8    	ldur	x8, [x29, #-0xc8]
100005c18: f100bd1f    	cmp	x8, #0x2f
100005c1c: 54002369    	b.ls	0x100006088 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x156c>
100005c20: f85403a9    	ldur	x9, [x29, #-0xc0]
100005c24: f000020b    	adrp	x11, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005c28: 91179d6b    	add	x11, x11, #0x5e7
100005c2c: eb0b012a    	subs	x10, x9, x11
100005c30: da8a854a    	cneg	x10, x10, ls
100005c34: f100bd5f    	cmp	x10, #0x2f
100005c38: 54002329    	b.ls	0x10000609c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1580>
100005c3c: ad400560    	ldp	q0, q1, [x11]
100005c40: ad000520    	stp	q0, q1, [x9]
100005c44: 3dc00960    	ldr	q0, [x11, #0x20]
100005c48: 3d800920    	str	q0, [x9, #0x20]
100005c4c: a93327a8    	stp	x8, x9, [x29, #-0xd0]
100005c50: 52800608    	mov	w8, #0x30               ; =48
100005c54: a9346fa8    	stp	x8, x27, [x29, #-0xc0]
100005c58: f81503b7    	stur	x23, [x29, #-0xb0]
100005c5c: a97327a8    	ldp	x8, x9, [x29, #-0xd0]
100005c60: b100051f    	cmn	x8, #0x1
100005c64: 540000c0    	b.eq	0x100005c7c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1160>
100005c68: f85403aa    	ldur	x10, [x29, #-0xc0]
100005c6c: 296933ab    	ldp	w11, w12, [x29, #-0xb8]
100005c70: f85503ad    	ldur	x13, [x29, #-0xb0]
100005c74: f9002fed    	str	x13, [sp, #0x58]
100005c78: 14000003    	b	0x100005c84 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1168>
100005c7c: f94037ea    	ldr	x10, [sp, #0x68]
100005c80: 294c2fec    	ldp	w12, w11, [sp, #0x60]
100005c84: b100051f    	cmn	x8, #0x1
100005c88: f85103ad    	ldur	x13, [x29, #-0xf0]
100005c8c: 9a8901b7    	csel	x23, x13, x9, eq
100005c90: f90037ea    	str	x10, [sp, #0x68]
100005c94: 9a8a013b    	csel	x27, x9, x10, eq
100005c98: 52800089    	mov	w9, #0x4                ; =4
100005c9c: 290c2fec    	stp	w12, w11, [sp, #0x60]
100005ca0: 1a8b012a    	csel	w10, w9, w11, eq
100005ca4: b9407fe9    	ldr	w9, [sp, #0x7c]
100005ca8: 1a8c0129    	csel	w9, w9, w12, eq
100005cac: b9007fe9    	str	w9, [sp, #0x7c]
100005cb0: 540000a0    	b.eq	0x100005cc4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x11a8>
100005cb4: aa098159    	orr	x25, x10, x9, lsl #32
100005cb8: f9402fe9    	ldr	x9, [sp, #0x58]
100005cbc: f9007be9    	str	x9, [sp, #0xf0]
100005cc0: 17fffc59    	b	0x100004e24 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x308>
100005cc4: 12103d48    	and	w8, w10, #0xffff0000
100005cc8: aa0a03eb    	mov	x11, x10
100005ccc: f81103b7    	stur	x23, [x29, #-0xf0]
100005cd0: 17fffc4e    	b	0x100004e08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
100005cd4: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005cd8: 9a983708    	cinc	x8, x24, hs
100005cdc: ab000108    	adds	x8, x8, x0
100005ce0: 540028a2    	b.hs	0x1000061f4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16d8>
100005ce4: f81303a8    	stur	x8, [x29, #-0xd0]
100005ce8: d10343a1    	sub	x1, x29, #0xd0
100005cec: aa1503e0    	mov	x0, x21
100005cf0: 52800142    	mov	w2, #0xa                ; =10
100005cf4: 940001f3    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005cf8: 320083e8    	mov	w8, #0x10001            ; =65537
100005cfc: 11040108    	add	w8, w8, #0x100
100005d00: 7100001f    	cmp	w0, #0x0
100005d04: 52802029    	mov	w9, #0x101              ; =257
100005d08: 1a881137    	csel	w23, w9, w8, ne
100005d0c: d10343a1    	sub	x1, x29, #0xd0
100005d10: aa1503e0    	mov	x0, x21
100005d14: 9400018a    	bl	0x10000633c <<source_files_to_tokens::lexer::Lexer>::consume_literal_suffix>
100005d18: d2800009    	mov	x9, #0x0                ; =0
100005d1c: 7100001f    	cmp	w0, #0x0
100005d20: 52a02008    	mov	w8, #0x1000000          ; =16777216
100005d24: 1a9f1108    	csel	w8, w8, wzr, ne
100005d28: f85303bb    	ldur	x27, [x29, #-0xd0]
100005d2c: 320083ea    	mov	w10, #0x10001           ; =65537
100005d30: 1103fd4a    	add	w10, w10, #0xff
100005d34: 0a0a02ea    	and	w10, w23, w10
100005d38: 121002eb    	and	w11, w23, #0x10000
100005d3c: 2a0b0108    	orr	w8, w8, w11
100005d40: 5280002b    	mov	w11, #0x1               ; =1
100005d44: 17fffc31    	b	0x100004e08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
100005d48: 7140413f    	cmp	w9, #0x10, lsl #12      ; =0x10000
100005d4c: 9a983709    	cinc	x9, x24, hs
100005d50: ab080128    	adds	x8, x9, x8
100005d54: 54002622    	b.hs	0x100006218 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16fc>
100005d58: f81783a8    	stur	x8, [x29, #-0x88]
100005d5c: 910042a0    	add	x0, x21, #0x10
100005d60: 94000219    	bl	0x1000065c4 <<core::str::iter::CharIndices as core::iter::traits::iterator::Iterator>::next>
100005d64: f90016a0    	str	x0, [x21, #0x28]
100005d68: b90032a1    	str	w1, [x21, #0x30]
100005d6c: 7100b43f    	cmp	w1, #0x2d
100005d70: 54000060    	b.eq	0x100005d7c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1260>
100005d74: 7100ac3f    	cmp	w1, #0x2b
100005d78: 540003a1    	b.ne	0x100005dec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12d0>
100005d7c: b90032b6    	str	w22, [x21, #0x30]
100005d80: 3100043f    	cmn	w1, #0x1
100005d84: 54000340    	b.eq	0x100005dec <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12d0>
100005d88: 7102003f    	cmp	w1, #0x80
100005d8c: 54000062    	b.hs	0x100005d98 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x127c>
100005d90: 52800028    	mov	w8, #0x1                ; =1
100005d94: 14000013    	b	0x100005de0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12c4>
100005d98: 7120003f    	cmp	w1, #0x800
100005d9c: 540001e2    	b.hs	0x100005dd8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12bc>
100005da0: 52800048    	mov	w8, #0x2                ; =2
100005da4: 1400000f    	b	0x100005de0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x12c4>
100005da8: 7120011f    	cmp	w8, #0x800
100005dac: 54000742    	b.hs	0x100005e94 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1378>
100005db0: 52800048    	mov	w8, #0x2                ; =2
100005db4: 1400003a    	b	0x100005e9c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1380>
100005db8: 7120011f    	cmp	w8, #0x800
100005dbc: 54000862    	b.hs	0x100005ec8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13ac>
100005dc0: 52800048    	mov	w8, #0x2                ; =2
100005dc4: 14000043    	b	0x100005ed0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13b4>
100005dc8: 7120011f    	cmp	w8, #0x800
100005dcc: 54000982    	b.hs	0x100005efc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13e0>
100005dd0: 52800048    	mov	w8, #0x2                ; =2
100005dd4: 1400004c    	b	0x100005f04 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x13e8>
100005dd8: 7140403f    	cmp	w1, #0x10, lsl #12      ; =0x10000
100005ddc: 9a983708    	cinc	x8, x24, hs
100005de0: ab000108    	adds	x8, x8, x0
100005de4: 54002382    	b.hs	0x100006254 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1738>
100005de8: f81783a8    	stur	x8, [x29, #-0x88]
100005dec: d10223a1    	sub	x1, x29, #0x88
100005df0: aa1503e0    	mov	x0, x21
100005df4: 52800142    	mov	w2, #0xa                ; =10
100005df8: 940001b2    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005dfc: aa0003f7    	mov	x23, x0
100005e00: d10223a1    	sub	x1, x29, #0x88
100005e04: aa1503e0    	mov	x0, x21
100005e08: 9400014d    	bl	0x10000633c <<source_files_to_tokens::lexer::Lexer>::consume_literal_suffix>
100005e0c: 710002ff    	cmp	w23, #0x0
100005e10: 320083e8    	mov	w8, #0x10001            ; =65537
100005e14: 1a9f0509    	csinc	w9, w8, wzr, eq
100005e18: 52802008    	mov	w8, #0x100              ; =256
100005e1c: 7100001f    	cmp	w0, #0x0
100005e20: 52a0200a    	mov	w10, #0x1000000         ; =16777216
100005e24: 1a9f114a    	csel	w10, w10, wzr, ne
100005e28: 2a080128    	orr	w8, w9, w8
100005e2c: 2a0a010a    	orr	w10, w8, w10
100005e30: d2800009    	mov	x9, #0x0                ; =0
100005e34: f85783bb    	ldur	x27, [x29, #-0x88]
100005e38: 12102548    	and	w8, w10, #0x3ff0000
100005e3c: aa0a03eb    	mov	x11, x10
100005e40: 17fffbf2    	b	0x100004e08 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x2ec>
100005e44: 3100043f    	cmn	w1, #0x1
100005e48: 54000741    	b.ne	0x100005f30 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1414>
100005e4c: f94006a3    	ldr	x3, [x21, #0x8]
100005e50: f0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005e54: 9101a800    	add	x0, x0, #0x6a
100005e58: 52800341    	mov	w1, #0x1a               ; =26
100005e5c: f85103bb    	ldur	x27, [x29, #-0xf0]
100005e60: 14000009    	b	0x100005e84 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1368>
100005e64: 7100651f    	cmp	w8, #0x19
100005e68: 54000641    	b.ne	0x100005f30 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1414>
100005e6c: b100041f    	cmn	x0, #0x1
100005e70: 540020c0    	b.eq	0x100006288 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x176c>
100005e74: 91000403    	add	x3, x0, #0x1
100005e78: f0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100005e7c: 91179c00    	add	x0, x0, #0x5e7
100005e80: 52800601    	mov	w1, #0x30               ; =48
100005e84: d10343a8    	sub	x8, x29, #0xd0
100005e88: aa1b03e2    	mov	x2, x27
100005e8c: 94000236    	bl	0x100006764 <<source_files_to_tokens::lexer::lex_error::LexError>::new::<&str>>
100005e90: 17ffff73    	b	0x100005c5c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1140>
100005e94: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
100005e98: 9a983708    	cinc	x8, x24, hs
100005e9c: ab090117    	adds	x23, x8, x9
100005ea0: 54001b02    	b.hs	0x100006200 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16e4>
100005ea4: f81783b7    	stur	x23, [x29, #-0x88]
100005ea8: d10223a1    	sub	x1, x29, #0x88
100005eac: aa1503e0    	mov	x0, x21
100005eb0: 52800102    	mov	w2, #0x8                ; =8
100005eb4: 94000183    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005eb8: 5280001b    	mov	w27, #0x0               ; =0
100005ebc: 52000017    	eor	w23, w0, #0x1
100005ec0: 52a00039    	mov	w25, #0x10000           ; =65536
100005ec4: 17fffe51    	b	0x100005808 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcec>
100005ec8: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
100005ecc: 9a983708    	cinc	x8, x24, hs
100005ed0: ab090117    	adds	x23, x8, x9
100005ed4: 54001962    	b.hs	0x100006200 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16e4>
100005ed8: f81783b7    	stur	x23, [x29, #-0x88]
100005edc: d10223a1    	sub	x1, x29, #0x88
100005ee0: aa1503e0    	mov	x0, x21
100005ee4: 52800202    	mov	w2, #0x10               ; =16
100005ee8: 94000176    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005eec: 5280001b    	mov	w27, #0x0               ; =0
100005ef0: 52000017    	eor	w23, w0, #0x1
100005ef4: 52a00079    	mov	w25, #0x30000           ; =196608
100005ef8: 17fffe44    	b	0x100005808 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcec>
100005efc: 7140411f    	cmp	w8, #0x10, lsl #12      ; =0x10000
100005f00: 9a983708    	cinc	x8, x24, hs
100005f04: ab090117    	adds	x23, x8, x9
100005f08: 540017c2    	b.hs	0x100006200 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x16e4>
100005f0c: f81783b7    	stur	x23, [x29, #-0x88]
100005f10: d10223a1    	sub	x1, x29, #0x88
100005f14: aa1503e0    	mov	x0, x21
100005f18: 52800042    	mov	w2, #0x2                ; =2
100005f1c: 94000169    	bl	0x1000064c0 <<source_files_to_tokens::lexer::Lexer>::consume_digits_with_underscores>
100005f20: 5280001b    	mov	w27, #0x0               ; =0
100005f24: 52800019    	mov	w25, #0x0               ; =0
100005f28: 52000017    	eor	w23, w0, #0x1
100005f2c: 17fffe37    	b	0x100005808 <<source_files_to_tokens::lexer::Lexer>::tokenize+0xcec>
100005f30: aa0003f7    	mov	x23, x0
100005f34: aa0103f9    	mov	x25, x1
100005f38: b81a43a1    	stur	w1, [x29, #-0x5c]
100005f3c: d10173a8    	sub	x8, x29, #0x5c
100005f40: f81303a8    	stur	x8, [x29, #-0xd0]
100005f44: f00001a8    	adrp	x8, 0x10003c000 <<i32 as core::fmt::Debug>::fmt+0x48>
100005f48: 9132f108    	add	x8, x8, #0xcbc
100005f4c: f81383a8    	stur	x8, [x29, #-0xc8]
100005f50: d10223a8    	sub	x8, x29, #0x88
100005f54: d10343a1    	sub	x1, x29, #0xd0
100005f58: f00001e0    	adrp	x0, 0x100044000 <GCC_except_table1121+0x1c>
100005f5c: 912f3c00    	add	x0, x0, #0xbcf
100005f60: 9400c1fc    	bl	0x100036750 <alloc::fmt::format::format_inner>
100005f64: 7102033f    	cmp	w25, #0x80
100005f68: aa1703e8    	mov	x8, x23
100005f6c: 54000062    	b.hs	0x100005f78 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x145c>
100005f70: 52800029    	mov	w9, #0x1                ; =1
100005f74: 14000007    	b	0x100005f90 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1474>
100005f78: 7120033f    	cmp	w25, #0x800
100005f7c: 54000062    	b.hs	0x100005f88 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x146c>
100005f80: 52800049    	mov	w9, #0x2                ; =2
100005f84: 14000003    	b	0x100005f90 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1474>
100005f88: 7140433f    	cmp	w25, #0x10, lsl #12     ; =0x10000
100005f8c: 9a983709    	cinc	x9, x24, hs
100005f90: ab080128    	adds	x8, x9, x8
100005f94: 54001802    	b.hs	0x100006294 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1778>
100005f98: 3cd783a0    	ldur	q0, [x29, #-0x88]
100005f9c: 3c9303a0    	stur	q0, [x29, #-0xd0]
100005fa0: f85883a9    	ldur	x9, [x29, #-0x78]
100005fa4: a9346fa9    	stp	x9, x27, [x29, #-0xc0]
100005fa8: f81503a8    	stur	x8, [x29, #-0xb0]
100005fac: 17ffff2c    	b	0x100005c5c <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1140>
100005fb0: 53087f29    	lsr	w9, w25, #8
100005fb4: d360ff2a    	lsr	x10, x25, #32
100005fb8: a9005f88    	stp	x8, x23, [x28]
100005fbc: f9000b9b    	str	x27, [x28, #0x10]
100005fc0: 3900638b    	strb	w11, [x28, #0x18]
100005fc4: 53187f28    	lsr	w8, w25, #24
100005fc8: 39006f88    	strb	w8, [x28, #0x1b]
100005fcc: 78019389    	sturh	w9, [x28, #0x19]
100005fd0: b9001f8a    	str	w10, [x28, #0x1c]
100005fd4: f9407fe8    	ldr	x8, [sp, #0xf8]
100005fd8: f9001388    	str	x8, [x28, #0x20]
100005fdc: f85283a8    	ldur	x8, [x29, #-0xd8]
100005fe0: b4000148    	cbz	x8, 0x100006008 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x14ec>
100005fe4: f85203b4    	ldur	x20, [x29, #-0xe0]
100005fe8: 52800029    	mov	w9, #0x1                ; =1
100005fec: cb080135    	sub	x21, x9, x8
100005ff0: aa1403e0    	mov	x0, x20
100005ff4: 94000217    	bl	0x100006850 <core::ptr::drop_glue::<tokens::token_stream::token::Token>>
100005ff8: 91010294    	add	x20, x20, #0x40
100005ffc: 910006b5    	add	x21, x21, #0x1
100006000: f10006bf    	cmp	x21, #0x1
100006004: 54ffff61    	b.ne	0x100005ff0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x14d4>
100006008: d103a3a0    	sub	x0, x29, #0xe8
10000600c: 52800101    	mov	w1, #0x8                ; =8
100006010: 52800802    	mov	w2, #0x40               ; =64
100006014: 9400a69c    	bl	0x10002fa84 <<alloc::raw_vec::RawVecInner>::deallocate>
100006018: 14000007    	b	0x100006034 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1518>
10000601c: 3cd183a0    	ldur	q0, [x29, #-0xe8]
100006020: 3c808380    	stur	q0, [x28, #0x8]
100006024: f85283a8    	ldur	x8, [x29, #-0xd8]
100006028: f9000f88    	str	x8, [x28, #0x18]
10000602c: 92800008    	mov	x8, #-0x1               ; =-1
100006030: f9000388    	str	x8, [x28]
100006034: 910683ff    	add	sp, sp, #0x1a0
100006038: a9457bfd    	ldp	x29, x30, [sp, #0x50]
10000603c: a9444ff4    	ldp	x20, x19, [sp, #0x40]
100006040: a94357f6    	ldp	x22, x21, [sp, #0x30]
100006044: a9425ff8    	ldp	x24, x23, [sp, #0x20]
100006048: a94167fa    	ldp	x26, x25, [sp, #0x10]
10000604c: a8c66ffc    	ldp	x28, x27, [sp], #0x60
100006050: d65f03c0    	ret
100006054: aa1703e2    	mov	x2, x23
100006058: d00002c4    	adrp	x4, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000605c: 9121e084    	add	x4, x4, #0x878
100006060: aa1b03e3    	mov	x3, x27
100006064: 9400ed9d    	bl	0x1000416d8 <core::str::slice_error_fail>
100006068: 1400008e    	b	0x1000062a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1784>
10000606c: d0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100006070: 91143000    	add	x0, x0, #0x50c
100006074: d00002c3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006078: 913fe063    	add	x3, x3, #0xff8
10000607c: 528036e1    	mov	w1, #0x1b7              ; =439
100006080: 52800002    	mov	w2, #0x0                ; =0
100006084: 9400ee0d    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
100006088: d0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
10000608c: 911cc800    	add	x0, x0, #0x732
100006090: f00002c3    	adrp	x3, 0x100061000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.12+0xa18>
100006094: 9100a063    	add	x3, x3, #0x28
100006098: 14000017    	b	0x1000060f4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15d8>
10000609c: d0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
1000060a0: 91185c00    	add	x0, x0, #0x617
1000060a4: f00002c3    	adrp	x3, 0x100061000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.12+0xa18>
1000060a8: 91004063    	add	x3, x3, #0x10
1000060ac: 528046e1    	mov	w1, #0x237              ; =567
1000060b0: 52800002    	mov	w2, #0x0                ; =0
1000060b4: 9400ee01    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
1000060b8: a94c27e8    	ldp	x8, x9, [sp, #0xc0]
1000060bc: f9400120    	ldr	x0, [x9]
1000060c0: f9400101    	ldr	x1, [x8]
1000060c4: 9400ed59    	bl	0x100041628 <alloc::raw_vec::handle_error>
1000060c8: 14000076    	b	0x1000062a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1784>
1000060cc: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000060d0: 91230000    	add	x0, x0, #0x8c0
1000060d4: 9400eebc    	bl	0x100041bc4 <core::panicking::panic_const::panic_const_add_overflow>
1000060d8: 14000072    	b	0x1000062a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1784>
1000060dc: f85403a1    	ldur	x1, [x29, #-0xc0]
1000060e0: 9400ed52    	bl	0x100041628 <alloc::raw_vec::handle_error>
1000060e4: d0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
1000060e8: 91068c00    	add	x0, x0, #0x1a3
1000060ec: d00002c3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000060f0: 913f2063    	add	x3, x3, #0xfc8
1000060f4: 52803761    	mov	w1, #0x1bb              ; =443
1000060f8: 52800002    	mov	w2, #0x0                ; =0
1000060fc: 9400edef    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
100006100: d00002c4    	adrp	x4, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006104: 91284084    	add	x4, x4, #0xa10
100006108: f85103a2    	ldur	x2, [x29, #-0xf0]
10000610c: 17ffffd5    	b	0x100006060 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1544>
100006110: a94c27e8    	ldp	x8, x9, [sp, #0xc0]
100006114: f9400120    	ldr	x0, [x9]
100006118: f9400101    	ldr	x1, [x8]
10000611c: 9400ed43    	bl	0x100041628 <alloc::raw_vec::handle_error>
100006120: 14000060    	b	0x1000062a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1784>
100006124: d00002c3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006128: 91236063    	add	x3, x3, #0x8d8
10000612c: f85103a0    	ldur	x0, [x29, #-0xf0]
100006130: f9404fe1    	ldr	x1, [sp, #0x98]
100006134: aa0103e2    	mov	x2, x1
100006138: 9400ee60    	bl	0x100041ab8 <core::slice::index::slice_index_fail>
10000613c: 14000059    	b	0x1000062a0 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1784>
100006140: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006144: 9127e000    	add	x0, x0, #0x9f8
100006148: 17ffffe3    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
10000614c: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006150: 91254000    	add	x0, x0, #0x950
100006154: 17ffffe0    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006158: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000615c: 9122a000    	add	x0, x0, #0x8a8
100006160: 17ffffdd    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006164: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006168: 91266000    	add	x0, x0, #0x998
10000616c: 17ffffda    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006170: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006174: 91272000    	add	x0, x0, #0x9c8
100006178: 17ffffd7    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
10000617c: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006180: 9126c000    	add	x0, x0, #0x9b0
100006184: 17ffffd4    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006188: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000618c: 91278000    	add	x0, x0, #0x9e0
100006190: 17ffffd1    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006194: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006198: 9124e000    	add	x0, x0, #0x938
10000619c: 17ffffce    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061a0: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061a4: 91248000    	add	x0, x0, #0x920
1000061a8: 17ffffcb    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061ac: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061b0: 9125a000    	add	x0, x0, #0x968
1000061b4: 17ffffc8    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061b8: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061bc: 9128a000    	add	x0, x0, #0xa28
1000061c0: 17ffffc5    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061c4: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061c8: 912a8000    	add	x0, x0, #0xaa0
1000061cc: 17ffffc2    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061d0: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061d4: 912ae000    	add	x0, x0, #0xab8
1000061d8: 17ffffbf    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061dc: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061e0: 91224000    	add	x0, x0, #0x890
1000061e4: 17ffffbc    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061e8: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061ec: 912d8000    	add	x0, x0, #0xb60
1000061f0: 17ffffb9    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
1000061f4: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
1000061f8: 912b4000    	add	x0, x0, #0xad0
1000061fc: 17ffffb6    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006200: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006204: 912a2000    	add	x0, x0, #0xa88
100006208: 17ffffb3    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
10000620c: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006210: 912cc000    	add	x0, x0, #0xb30
100006214: 17ffffb0    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006218: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000621c: 91296000    	add	x0, x0, #0xa58
100006220: 17ffffad    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006224: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006228: 91290000    	add	x0, x0, #0xa40
10000622c: 17ffffaa    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006230: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006234: 91242000    	add	x0, x0, #0x908
100006238: 17ffffa7    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
10000623c: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006240: 9123c000    	add	x0, x0, #0x8f0
100006244: 17ffffa4    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006248: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000624c: 912d2000    	add	x0, x0, #0xb48
100006250: 17ffffa1    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006254: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006258: 9129c000    	add	x0, x0, #0xa70
10000625c: 17ffff9e    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006260: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006264: 91260000    	add	x0, x0, #0x980
100006268: 17ffff9b    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
10000626c: d0000200    	adrp	x0, 0x100048000 <_anon.4f08c176c92f8821926a9b2c4a8a346e.52+0x6fa>
100006270: 910fd400    	add	x0, x0, #0x3f5
100006274: d00002c3    	adrp	x3, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006278: 913ec063    	add	x3, x3, #0xfb0
10000627c: 528045e1    	mov	w1, #0x22f              ; =559
100006280: 52800002    	mov	w2, #0x0                ; =0
100006284: 9400ed8d    	bl	0x1000418b8 <core::panicking::panic_nounwind_fmt>
100006288: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
10000628c: 912de000    	add	x0, x0, #0xb78
100006290: 17ffff91    	b	0x1000060d4 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x15b8>
100006294: d00002c0    	adrp	x0, 0x100060000 <_anon.39fb678c2aae451e69c32fd226da1a5f.1>
100006298: 912e4000    	add	x0, x0, #0xb90
10000629c: 9400ee4a    	bl	0x100041bc4 <core::panicking::panic_const::panic_const_add_overflow>
1000062a0: d4200020    	brk	#0x1
1000062a4: aa0003f3    	mov	x19, x0
1000062a8: d10223a0    	sub	x0, x29, #0x88
1000062ac: 97fff9db    	bl	0x100004a18 <core::ptr::drop_glue::<alloc::string::String>>
1000062b0: 1400001e    	b	0x100006328 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x180c>
1000062b4: 9400ed6d    	bl	0x100041868 <core::panicking::panic_in_cleanup>
1000062b8: 1400001b    	b	0x100006324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1808>
1000062bc: 1400001a    	b	0x100006324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1808>
1000062c0: 14000019    	b	0x100006324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1808>
1000062c4: aa0003f3    	mov	x19, x0
1000062c8: d10343a0    	sub	x0, x29, #0xd0
1000062cc: 97fff9e2    	bl	0x100004a54 <core::ptr::drop_glue::<tokens::token_stream::token::Token>>
1000062d0: 14000016    	b	0x100006328 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x180c>
1000062d4: 9400ed65    	bl	0x100041868 <core::panicking::panic_in_cleanup>
1000062d8: aa0003f3    	mov	x19, x0
1000062dc: b4000115    	cbz	x21, 0x1000062fc <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17e0>
1000062e0: 91010294    	add	x20, x20, #0x40
1000062e4: cb1503f5    	neg	x21, x21
1000062e8: aa1403e0    	mov	x0, x20
1000062ec: 94000159    	bl	0x100006850 <core::ptr::drop_glue::<tokens::token_stream::token::Token>>
1000062f0: 91010294    	add	x20, x20, #0x40
1000062f4: f10006b5    	subs	x21, x21, #0x1
1000062f8: 54ffff81    	b.ne	0x1000062e8 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x17cc>
1000062fc: d103a3a0    	sub	x0, x29, #0xe8
100006300: 52800101    	mov	w1, #0x8                ; =8
100006304: 52800802    	mov	w2, #0x40               ; =64
100006308: 9400a5df    	bl	0x10002fa84 <<alloc::raw_vec::RawVecInner>::deallocate>
10000630c: 14000009    	b	0x100006330 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1814>
100006310: 9400ed56    	bl	0x100041868 <core::panicking::panic_in_cleanup>
100006314: 9400ed55    	bl	0x100041868 <core::panicking::panic_in_cleanup>
100006318: 14000003    	b	0x100006324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1808>
10000631c: 14000002    	b	0x100006324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1808>
100006320: 14000001    	b	0x100006324 <<source_files_to_tokens::lexer::Lexer>::tokenize+0x1808>
100006324: aa0003f3    	mov	x19, x0
100006328: d103a3a0    	sub	x0, x29, #0xe8
10000632c: 97fff992    	bl	0x100004974 <core::ptr::drop_glue::<alloc::vec::Vec<tokens::token_stream::token::Token>>>
100006330: aa1303e0    	mov	x0, x19
100006334: 9400ee93    	bl	0x100041d80 <dyld_stub_binder+0x100041d80>
100006338: 9400ed4c    	bl	0x100041868 <core::panicking::panic_in_cleanup>
