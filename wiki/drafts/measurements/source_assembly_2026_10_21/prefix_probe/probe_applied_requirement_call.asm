000000010007da60 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call>:
10007da60: a9bc6ffc    	stp	x28, x27, [sp, #-0x40]!
10007da64: a90157f6    	stp	x22, x21, [sp, #0x10]
10007da68: a9024ff4    	stp	x20, x19, [sp, #0x20]
10007da6c: a9037bfd    	stp	x29, x30, [sp, #0x30]
10007da70: 9100c3fd    	add	x29, sp, #0x30
10007da74: d14017e9    	sub	x9, sp, #0x5, lsl #12   ; =0x5000
10007da78: d14007ff    	sub	sp, sp, #0x1, lsl #12   ; =0x1000
10007da7c: f94003ff    	ldr	xzr, [sp]
10007da80: eb2963ff    	cmp	sp, x9
10007da84: 54ffffa1    	b.ne	0x10007da78 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x18>
10007da88: d10403ff    	sub	sp, sp, #0x100
10007da8c: aa0003f3    	mov	x19, x0
10007da90: f9400800    	ldr	x0, [x0, #0x10]
10007da94: 910003e8    	mov	x8, sp
10007da98: 940321c9    	bl	0x1001461bc <<syntax_trees::syntax_trees::SyntaxTrees>::new>
10007da9c: d102e3a8    	sub	x8, x29, #0xb8
10007daa0: 910003e0    	mov	x0, sp
10007daa4: aa1303e1    	mov	x1, x19
10007daa8: 52800022    	mov	w2, #0x1                ; =1
10007daac: 9400d4b9    	bl	0x1000b2d90 <tokens_to_syntax_trees::type_syntax::parse_type::parse_type_reference_handle_with_trailing_domain>
10007dab0: f85483a8    	ldur	x8, [x29, #-0xb8]
10007dab4: b100051f    	cmn	x8, #0x1
10007dab8: 540000c0    	b.eq	0x10007dad0 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x70>
10007dabc: d102e3a0    	sub	x0, x29, #0xb8
10007dac0: 9403f752    	bl	0x10017b808 <<alloc::vec::Vec<u8> as core::ops::drop::Drop>::drop>
10007dac4: d102e3a0    	sub	x0, x29, #0xb8
10007dac8: 9403f735    	bl	0x10017b79c <<alloc::raw_vec::RawVec<u8> as core::ops::drop::Drop>::drop>
10007dacc: 14000036    	b	0x10007dba4 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x144>
10007dad0: d10343b3    	sub	x19, x29, #0xd0
10007dad4: 29440a61    	ldp	w1, w2, [x19, #0x20]
10007dad8: a975d7b4    	ldp	x20, x21, [x29, #-0xa8]
10007dadc: f85683a8    	ldur	x8, [x29, #-0x98]
10007dae0: f81383a8    	stur	x8, [x29, #-0xc8]
10007dae4: 79408268    	ldrh	w8, [x19, #0x40]
10007dae8: 79002268    	strh	w8, [x19, #0x10]
10007daec: b8443268    	ldur	w8, [x19, #0x43]
10007daf0: b9000268    	str	w8, [x19]
10007daf4: 39411e68    	ldrb	w8, [x19, #0x47]
10007daf8: 39001268    	strb	w8, [x19, #0x4]
10007dafc: 5288ae08    	mov	w8, #0x4570             ; =17776
10007db00: 910003e9    	mov	x9, sp
10007db04: 39410a76    	ldrb	w22, [x19, #0x42]
10007db08: 8b080120    	add	x0, x9, x8
10007db0c: 9402f8b8    	bl	0x10013bdec <<syntax_trees::syntax_trees::type_system::types::TypeReferenceTable>::type_reference>
10007db10: f9400008    	ldr	x8, [x0]
10007db14: f100091f    	cmp	x8, #0x2
10007db18: 1a9f97e8    	cset	w8, hi
10007db1c: f10002bf    	cmp	x21, #0x0
10007db20: 1a9f17e9    	cset	w9, eq
10007db24: 2a0902c9    	orr	w9, w22, w9
10007db28: 2a090108    	orr	w8, w8, w9
10007db2c: 370003c8    	tbnz	w8, #0x0, 0x10007dba4 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x144>
10007db30: 3940e288    	ldrb	w8, [x20, #0x38]
10007db34: 3940e689    	ldrb	w9, [x20, #0x39]
10007db38: 7100191f    	cmp	w8, #0x6
10007db3c: 7a470920    	ccmp	w9, #0x7, #0x0, eq
10007db40: 54000321    	b.ne	0x10007dba4 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x144>
10007db44: a93b57b4    	stp	x20, x21, [x29, #-0x50]
10007db48: f85383a8    	ldur	x8, [x29, #-0xc8]
10007db4c: f81c03a8    	stur	x8, [x29, #-0x40]
10007db50: 79402268    	ldrh	w8, [x19, #0x10]
10007db54: 79013268    	strh	w8, [x19, #0x98]
10007db58: 39026a7f    	strb	wzr, [x19, #0x9a]
10007db5c: b9400268    	ldr	w8, [x19]
10007db60: b809b268    	stur	w8, [x19, #0x9b]
10007db64: 39401268    	ldrb	w8, [x19, #0x4]
10007db68: 39027e68    	strb	w8, [x19, #0x9f]
10007db6c: b0000a42    	adrp	x2, 0x1001c6000 <_anon.7ce1f5357e21a2f4e7acde0ebde5ce33.25+0x7a0>
10007db70: 91212442    	add	x2, x2, #0x849
10007db74: d102e3a8    	sub	x8, x29, #0xb8
10007db78: d10143a0    	sub	x0, x29, #0x50
10007db7c: 528000e1    	mov	w1, #0x7                ; =7
10007db80: 52800043    	mov	w3, #0x2                ; =2
10007db84: 9401584a    	bl	0x1000d3cac <<tokens_to_syntax_trees::input::token_cursor::Input>::take_punctuation>
10007db88: f85483a8    	ldur	x8, [x29, #-0xb8]
10007db8c: b100051f    	cmn	x8, #0x1
10007db90: 54000200    	b.eq	0x10007dbd0 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x170>
10007db94: d102e3a0    	sub	x0, x29, #0xb8
10007db98: 9403f71c    	bl	0x10017b808 <<alloc::vec::Vec<u8> as core::ops::drop::Drop>::drop>
10007db9c: d102e3a0    	sub	x0, x29, #0xb8
10007dba0: 9403f6ff    	bl	0x10017b79c <<alloc::raw_vec::RawVec<u8> as core::ops::drop::Drop>::drop>
10007dba4: 52800013    	mov	w19, #0x0               ; =0
10007dba8: 910003e0    	mov	x0, sp
10007dbac: 97fff009    	bl	0x100079bd0 <core::ptr::drop_glue::<syntax_trees::syntax_trees::SyntaxTrees>>
10007dbb0: aa1303e0    	mov	x0, x19
10007dbb4: 914017ff    	add	sp, sp, #0x5, lsl #12   ; =0x5000
10007dbb8: 910403ff    	add	sp, sp, #0x100
10007dbbc: a9437bfd    	ldp	x29, x30, [sp, #0x30]
10007dbc0: a9424ff4    	ldp	x20, x19, [sp, #0x20]
10007dbc4: a94157f6    	ldp	x22, x21, [sp, #0x10]
10007dbc8: a8c46ffc    	ldp	x28, x27, [sp], #0x40
10007dbcc: d65f03c0    	ret
10007dbd0: ad410660    	ldp	q0, q1, [x19, #0x20]
10007dbd4: ad3d87a0    	stp	q0, q1, [x29, #-0x50]
10007dbd8: d102e3a8    	sub	x8, x29, #0xb8
10007dbdc: d10143a0    	sub	x0, x29, #0x50
10007dbe0: 940157d6    	bl	0x1000d3b38 <<tokens_to_syntax_trees::input::token_cursor::Input>::take_identifier>
10007dbe4: f85483a8    	ldur	x8, [x29, #-0xb8]
10007dbe8: b100051f    	cmn	x8, #0x1
10007dbec: 54000200    	b.eq	0x10007dc2c <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x1cc>
10007dbf0: a97957b4    	ldp	x20, x21, [x29, #-0x70]
10007dbf4: 3941ea76    	ldrb	w22, [x19, #0x7a]
10007dbf8: d102e3a0    	sub	x0, x29, #0xb8
10007dbfc: 97ffdc3e    	bl	0x100074cf4 <core::ptr::drop_glue::<core::result::Result<(syntax_trees::syntax_trees::names::identifier::Identifier, tokens_to_syntax_trees::input::token_cursor::Input), tokens_to_syntax_trees::diagnostics::parse_error::ParseError>>>
10007dc00: 52800013    	mov	w19, #0x0               ; =0
10007dc04: 3707fd36    	tbnz	w22, #0x0, 0x10007dba8 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x148>
10007dc08: b4fffd15    	cbz	x21, 0x10007dba8 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x148>
10007dc0c: 3940e288    	ldrb	w8, [x20, #0x38]
10007dc10: 7100191f    	cmp	w8, #0x6
10007dc14: 54000121    	b.ne	0x10007dc38 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x1d8>
10007dc18: 3940e688    	ldrb	w8, [x20, #0x39]
10007dc1c: 7100591f    	cmp	w8, #0x16
10007dc20: 540000c1    	b.ne	0x10007dc38 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x1d8>
10007dc24: 52800033    	mov	w19, #0x1               ; =1
10007dc28: 17ffffe0    	b	0x10007dba8 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x148>
10007dc2c: d102e3a0    	sub	x0, x29, #0xb8
10007dc30: 97ffdc31    	bl	0x100074cf4 <core::ptr::drop_glue::<core::result::Result<(syntax_trees::syntax_trees::names::identifier::Identifier, tokens_to_syntax_trees::input::token_cursor::Input), tokens_to_syntax_trees::diagnostics::parse_error::ParseError>>>
10007dc34: 17ffffdc    	b	0x10007dba4 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x144>
10007dc38: 3940e288    	ldrb	w8, [x20, #0x38]
10007dc3c: 7100191f    	cmp	w8, #0x6
10007dc40: 3940e688    	ldrb	w8, [x20, #0x39]
10007dc44: 7a570900    	ccmp	w8, #0x17, #0x0, eq
10007dc48: 1a9f17f3    	cset	w19, eq
10007dc4c: 17ffffd7    	b	0x10007dba8 <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x148>
10007dc50: aa0003f3    	mov	x19, x0
10007dc54: d102e3a0    	sub	x0, x29, #0xb8
10007dc58: 9403f6d1    	bl	0x10017b79c <<alloc::raw_vec::RawVec<u8> as core::ops::drop::Drop>::drop>
10007dc5c: 14000008    	b	0x10007dc7c <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x21c>
10007dc60: 94046382    	bl	0x100196a68 <core::panicking::panic_in_cleanup>
10007dc64: aa0003f3    	mov	x19, x0
10007dc68: d102e3a0    	sub	x0, x29, #0xb8
10007dc6c: 9403f6cc    	bl	0x10017b79c <<alloc::raw_vec::RawVec<u8> as core::ops::drop::Drop>::drop>
10007dc70: 14000003    	b	0x10007dc7c <tokens_to_syntax_trees::expressions::primary::probe_applied_requirement_call+0x21c>
10007dc74: 9404637d    	bl	0x100196a68 <core::panicking::panic_in_cleanup>
10007dc78: aa0003f3    	mov	x19, x0
10007dc7c: 910003e0    	mov	x0, sp
10007dc80: 97ffefd4    	bl	0x100079bd0 <core::ptr::drop_glue::<syntax_trees::syntax_trees::SyntaxTrees>>
10007dc84: aa1303e0    	mov	x0, x19
10007dc88: 940464d4    	bl	0x100196fd8 <dyld_stub_binder+0x100196fd8>
10007dc8c: 94046377    	bl	0x100196a68 <core::panicking::panic_in_cleanup>
10007dc90: d503201f    	nop
10007dc94: d503201f    	nop
10007dc98: d503201f    	nop
10007dc9c: d503201f    	nop
10007dca0: d503201f    	nop
10007dca4: d503201f    	nop
10007dca8: d503201f    	nop
10007dcac: d503201f    	nop
10007dcb0: d503201f    	nop
10007dcb4: d503201f    	nop
10007dcb8: d503201f    	nop
10007dcbc: d503201f    	nop