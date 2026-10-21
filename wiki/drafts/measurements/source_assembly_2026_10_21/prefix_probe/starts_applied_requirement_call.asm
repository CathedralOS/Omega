000000010007f984 <tokens_to_syntax_trees::expressions::primary::starts_applied_requirement_call>:
10007f984: d10143ff    	sub	sp, sp, #0x50
10007f988: a9034ff4    	stp	x20, x19, [sp, #0x30]
10007f98c: a9047bfd    	stp	x29, x30, [sp, #0x40]
10007f990: 910103fd    	add	x29, sp, #0x40
10007f994: aa0003f3    	mov	x19, x0
10007f998: 910003e8    	mov	x8, sp
10007f99c: 94014df8    	bl	0x1000d317c <<tokens_to_syntax_trees::input::token_cursor::Input>::expect_token>
10007f9a0: f94003e8    	ldr	x8, [sp]
10007f9a4: b100051f    	cmn	x8, #0x1
10007f9a8: 54000140    	b.eq	0x10007f9d0 <tokens_to_syntax_trees::expressions::primary::starts_applied_requirement_call+0x4c>
10007f9ac: 910003e0    	mov	x0, sp
10007f9b0: 9403ef96    	bl	0x10017b808 <<alloc::vec::Vec<u8> as core::ops::drop::Drop>::drop>
10007f9b4: 910003e0    	mov	x0, sp
10007f9b8: 9403ef79    	bl	0x10017b79c <<alloc::raw_vec::RawVec<u8> as core::ops::drop::Drop>::drop>
10007f9bc: 52800000    	mov	w0, #0x0                ; =0
10007f9c0: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007f9c4: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007f9c8: 910143ff    	add	sp, sp, #0x50
10007f9cc: d65f03c0    	ret
10007f9d0: 3940abe8    	ldrb	w8, [sp, #0x2a]
10007f9d4: 3707ff48    	tbnz	w8, #0x0, 0x10007f9bc <tokens_to_syntax_trees::expressions::primary::starts_applied_requirement_call+0x38>
10007f9d8: f9400fe8    	ldr	x8, [sp, #0x18]
10007f9dc: b4ffff08    	cbz	x8, 0x10007f9bc <tokens_to_syntax_trees::expressions::primary::starts_applied_requirement_call+0x38>
10007f9e0: f9400be8    	ldr	x8, [sp, #0x10]
10007f9e4: 3940e109    	ldrb	w9, [x8, #0x38]
10007f9e8: 3940e50a    	ldrb	w10, [x8, #0x39]
10007f9ec: 7100193f    	cmp	w9, #0x6
10007f9f0: 7a570940    	ccmp	w10, #0x17, #0x0, eq
10007f9f4: 540000c0    	b.eq	0x10007fa0c <tokens_to_syntax_trees::expressions::primary::starts_applied_requirement_call+0x88>
10007f9f8: 3940e109    	ldrb	w9, [x8, #0x38]
10007f9fc: 3940e508    	ldrb	w8, [x8, #0x39]
10007fa00: 7100193f    	cmp	w9, #0x6
10007fa04: 7a470900    	ccmp	w8, #0x7, #0x0, eq
10007fa08: 54fffda1    	b.ne	0x10007f9bc <tokens_to_syntax_trees::expressions::primary::starts_applied_requirement_call+0x38>
10007fa0c: aa1303e0    	mov	x0, x19
10007fa10: a9447bfd    	ldp	x29, x30, [sp, #0x40]
10007fa14: a9434ff4    	ldp	x20, x19, [sp, #0x30]
10007fa18: 910143ff    	add	sp, sp, #0x50
10007fa1c: 17fff811    	b	0x10007da60 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call>
10007fa20: aa0003f3    	mov	x19, x0
10007fa24: 910003e0    	mov	x0, sp
10007fa28: 9403ef5d    	bl	0x10017b79c <<alloc::raw_vec::RawVec<u8> as core::ops::drop::Drop>::drop>
10007fa2c: aa1303e0    	mov	x0, x19
10007fa30: 94045d6a    	bl	0x100196fd8 <dyld_stub_binder+0x100196fd8>
10007fa34: 94045c0d    	bl	0x100196a68 <core::panicking::panic_in_cleanup>