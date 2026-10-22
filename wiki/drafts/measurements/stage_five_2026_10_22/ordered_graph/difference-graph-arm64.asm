
build/stage-five/reuse/omega-final-candidate:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000104346ff4 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap>:
104346ff4:     	sub	sp, sp, #0x80
104346ff8:     	stp	x28, x27, [sp, #0x20]
104346ffc:     	stp	x26, x25, [sp, #0x30]
104347000:     	stp	x24, x23, [sp, #0x40]
104347004:     	stp	x22, x21, [sp, #0x50]
104347008:     	stp	x20, x19, [sp, #0x60]
10434700c:     	stp	x29, x30, [sp, #0x70]
104347010:     	add	x29, sp, #0x70
104347014:     	mov	x22, x2
104347018:     	mov	x20, x1
10434701c:     	mov	x21, x0
104347020:     	mov	x0, x1
104347024:     	mov	x1, x2
104347028:     	bl	0x104347e88 <__RNvXs4_NtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_valuesNtB5_7OperandNtNtCs4aqKKZZmviT_4core3cmp9PartialEq2eq>
10434702c:     	tbz	w0, #0x0, 0x104347038 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x44>
104347030:     	mov	w0, #0x1                ; =1
104347034:     	b	0x104347254 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x260>
104347038:     	ldr	x19, [x21, #0x10]
10434703c:     	cbz	x19, 0x104347250 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x25c>
104347040:     	mov	x24, #0x0               ; =0
104347044:     	mov	x26, #0x0               ; =0
104347048:     	mov	x23, #0x0               ; =0
10434704c:     	ldr	x27, [x21, #0x8]
104347050:     	lsl	x28, x19, #3
104347054:     	mov	x25, x28
104347058:     	ldr	x0, [x27, x23, lsl #3]
10434705c:     	mov	x1, x20
104347060:     	bl	0x104347e88 <__RNvXs4_NtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_valuesNtB5_7OperandNtNtCs4aqKKZZmviT_4core3cmp9PartialEq2eq>
104347064:     	tbnz	w0, #0x0, 0x104347080 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x8c>
104347068:     	add	x23, x23, #0x1
10434706c:     	add	x26, x26, #0x10
104347070:     	add	x24, x24, #0x18
104347074:     	subs	x25, x25, #0x8
104347078:     	b.ne	0x104347058 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x64>
10434707c:     	b	0x104347250 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x25c>
104347080:     	cmp	x23, x19
104347084:     	b.hs	0x104347274 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x280>
104347088:     	mov	x25, #0x0               ; =0
10434708c:     	mov	x20, #0x0               ; =0
104347090:     	ldr	x0, [x27, x20, lsl #3]
104347094:     	mov	x1, x22
104347098:     	bl	0x104347e88 <__RNvXs4_NtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_valuesNtB5_7OperandNtNtCs4aqKKZZmviT_4core3cmp9PartialEq2eq>
10434709c:     	tbnz	w0, #0x0, 0x1043470b4 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0xc0>
1043470a0:     	add	x20, x20, #0x1
1043470a4:     	add	x25, x25, #0x10
1043470a8:     	subs	x28, x28, #0x8
1043470ac:     	b.ne	0x104347090 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x9c>
1043470b0:     	b	0x104347250 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x25c>
1043470b4:     	cmp	x20, x19
1043470b8:     	b.hs	0x104347274 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x280>
1043470bc:     	ldr	x1, [x21, #0x40]
1043470c0:     	cmp	x23, x1
1043470c4:     	b.hs	0x1043472b0 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x2bc>
1043470c8:     	ldr	x23, [x21, #0x38]
1043470cc:     	add	x22, x23, x24
1043470d0:     	ldr	x8, [x22]
1043470d4:     	cmn	x8, #0x1
1043470d8:     	b.ne	0x104347218 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x224>
1043470dc:     	adrp	x8, 0x106423000 <_heap_main+0x14c0>
1043470e0:     	add	x8, x8, #0x4e0
1043470e4:     	ldrb	w8, [x8]
1043470e8:     	cmp	w8, #0x1
1043470ec:     	b.eq	0x104347110 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x11c>
1043470f0:     	cmp	w8, #0x2
1043470f4:     	b.ne	0x104347298 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x2a4>
1043470f8:     	adrp	x0, 0x104de0000 <_anon.9932812392abb88d7bcf8d21aa97c884.34+0x77>
1043470fc:     	add	x0, x0, #0x9ee
104347100:     	mov	w1, #0x19               ; =25
104347104:     	mov	w2, #0x1                ; =1
104347108:     	mov	x3, #0x0                ; =0
10434710c:     	bl	0x1048cd420 <__RNvCsfjMFHT8xq90_13phase_profile3add>
104347110:     	add	x8, sp, #0x8
104347114:     	mov	x0, x19
104347118:     	mov	w1, #0x1                ; =1
10434711c:     	mov	w2, #0x8                ; =8
104347120:     	mov	w3, #0x10               ; =16
104347124:     	bl	0x10497e704 <__RNvMs5_NtCscU4bHatjcek_5alloc7raw_vecNtB5_11RawVecInner15try_allocate_inCs2qEGCSZWdXs_5gimli>
104347128:     	ldp	x8, x0, [sp, #0x8]
10434712c:     	cbnz	x8, 0x104347290 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x29c>
104347130:     	ldr	x8, [sp, #0x18]
104347134:     	stp	x0, x8, [sp, #0x8]
104347138:     	str	x19, [sp, #0x18]
10434713c:     	add	x9, x8, x26
104347140:     	mov	w8, #0x1                ; =1
104347144:     	stp	x8, xzr, [x9]
104347148:     	ldp	x9, x10, [x21, #0x20]
10434714c:     	mov	w11, #0x18              ; =24
104347150:     	madd	x12, x10, x11, x9
104347154:     	adrp	x2, 0x1063ea000 <_anon.a84f219c0843d5f34a49721bab61bdb9.6+0x20>
104347158:     	add	x2, x2, #0x750
10434715c:     	adrp	x10, 0x1063ea000 <_anon.a84f219c0843d5f34a49721bab61bdb9.6+0x20>
104347160:     	add	x10, x10, #0x768
104347164:     	b	0x10434716c <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x178>
104347168:     	tbz	w13, #0x0, 0x1043471ec <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x1f8>
10434716c:     	cmp	x8, x19
104347170:     	cinc	x8, x8, lo
104347174:     	b.hs	0x1043471ec <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x1f8>
104347178:     	mov	w13, #0x0               ; =0
10434717c:     	mov	x14, x9
104347180:     	ldp	x15, x1, [sp, #0x10]
104347184:     	mov	x17, x14
104347188:     	cmp	x14, x12
10434718c:     	csel	x14, xzr, x11, eq
104347190:     	b.eq	0x104347168 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x174>
104347194:     	ldr	x0, [x17]
104347198:     	cmp	x0, x1
10434719c:     	b.hs	0x1043472a8 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x2b4>
1043471a0:     	add	x14, x17, x14
1043471a4:     	add	x16, x15, x0, lsl #4
1043471a8:     	ldp	x3, x16, [x16]
1043471ac:     	ldr	x0, [x17, #0x10]
1043471b0:     	adds	x16, x16, x0
1043471b4:     	cset	w0, vs
1043471b8:     	cbz	x3, 0x104347184 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x190>
1043471bc:     	tbnz	w0, #0x0, 0x104347184 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x190>
1043471c0:     	ldr	x0, [x17, #0x8]
1043471c4:     	cmp	x0, x1
1043471c8:     	b.hs	0x1043472a4 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x2b0>
1043471cc:     	add	x17, x15, x0, lsl #4
1043471d0:     	ldp	x0, x3, [x17]
1043471d4:     	cmp	x0, #0x0
1043471d8:     	ccmp	x3, x16, #0x8, ne
1043471dc:     	b.ge	0x104347184 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x190>
1043471e0:     	mov	w13, #0x1               ; =1
1043471e4:     	stp	x13, x16, [x17]
1043471e8:     	b	0x104347180 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x18c>
1043471ec:     	ldr	x8, [x22]
1043471f0:     	cmn	x8, #0x1
1043471f4:     	b.eq	0x104347208 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x214>
1043471f8:     	add	x0, x23, x24
1043471fc:     	mov	w1, #0x8                ; =8
104347200:     	mov	w2, #0x10               ; =16
104347204:     	bl	0x10497e3dc <__RNvMs3_NtCscU4bHatjcek_5alloc7raw_vecNtB5_11RawVecInner10deallocateCs2qEGCSZWdXs_5gimli>
104347208:     	ldur	q0, [sp, #0x8]
10434720c:     	ldr	x8, [sp, #0x18]
104347210:     	str	x8, [x22, #0x10]
104347214:     	str	q0, [x22]
104347218:     	ldr	x8, [x22]
10434721c:     	cmn	x8, #0x1
104347220:     	b.eq	0x104347250 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x25c>
104347224:     	add	x8, x23, x24
104347228:     	ldr	x1, [x8, #0x10]
10434722c:     	cmp	x20, x1
104347230:     	b.hs	0x1043472c0 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x2cc>
104347234:     	ldr	x8, [x8, #0x8]
104347238:     	add	x8, x8, x25
10434723c:     	ldr	w9, [x8]
104347240:     	ldr	x8, [x8, #0x8]
104347244:     	cmn	x8, #0x1
104347248:     	csel	w0, wzr, w9, le
10434724c:     	b	0x104347254 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x260>
104347250:     	mov	w0, #0x0                ; =0
104347254:     	ldp	x29, x30, [sp, #0x70]
104347258:     	ldp	x20, x19, [sp, #0x60]
10434725c:     	ldp	x22, x21, [sp, #0x50]
104347260:     	ldp	x24, x23, [sp, #0x40]
104347264:     	ldp	x26, x25, [sp, #0x30]
104347268:     	ldp	x28, x27, [sp, #0x20]
10434726c:     	add	sp, sp, #0x80
104347270:     	ret
104347274:     	adrp	x0, 0x104de0000 <_anon.9932812392abb88d7bcf8d21aa97c884.34+0x77>
104347278:     	add	x0, x0, #0xcf7
10434727c:     	adrp	x3, 0x1063ea000 <_anon.a84f219c0843d5f34a49721bab61bdb9.6+0x20>
104347280:     	add	x3, x3, #0x6d8
104347284:     	mov	w1, #0x1bb              ; =443
104347288:     	mov	w2, #0x0                ; =0
10434728c:     	bl	0x1049dc838 <__RNvNtCs4aqKKZZmviT_4core9panicking18panic_nounwind_fmt>
104347290:     	ldr	x1, [sp, #0x18]
104347294:     	bl	0x1049dc4a8 <__RNvNtCscU4bHatjcek_5alloc7raw_vec12handle_error>
104347298:     	bl	0x1049d5850 <__RNvCsfjMFHT8xq90_13phase_profile10initialize>
10434729c:     	cbnz	w0, 0x1043470f8 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x104>
1043472a0:     	b	0x104347110 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x11c>
1043472a4:     	mov	x2, x10
1043472a8:     	bl	0x1049dc800 <__RNvNtCs4aqKKZZmviT_4core9panicking18panic_bounds_check>
1043472ac:     	brk	#0x1
1043472b0:     	adrp	x2, 0x1063ea000 <_anon.a84f219c0843d5f34a49721bab61bdb9.6+0x20>
1043472b4:     	add	x2, x2, #0x738
1043472b8:     	mov	x0, x23
1043472bc:     	bl	0x1049dc800 <__RNvNtCs4aqKKZZmviT_4core9panicking18panic_bounds_check>
1043472c0:     	adrp	x2, 0x1063ea000 <_anon.a84f219c0843d5f34a49721bab61bdb9.6+0x20>
1043472c4:     	add	x2, x2, #0x708
1043472c8:     	mov	x0, x20
1043472cc:     	bl	0x1049dc800 <__RNvNtCs4aqKKZZmviT_4core9panicking18panic_bounds_check>
1043472d0:     	mov	x19, x0
1043472d4:     	ldur	q0, [sp, #0x8]
1043472d8:     	ldr	x8, [sp, #0x18]
1043472dc:     	str	x8, [x22, #0x10]
1043472e0:     	str	q0, [x22]
1043472e4:     	mov	x0, x19
1043472e8:     	bl	0x1049dce24 <dyld_stub_binder+0x1049dce24>
1043472ec:     	mov	x19, x0
1043472f0:     	mov	w8, #0x1                ; =1
1043472f4:     	tbz	w8, #0x0, 0x104347308 <__RNvMNtNtNtNtNtCs9qwBaD1DgLm_10validation15proof_contracts18arithmetic_domains14ordered_values10congruence16difference_graphNtB2_15DifferenceGraph15nonnegative_gap+0x314>
1043472f8:     	add	x0, sp, #0x8
1043472fc:     	mov	w1, #0x8                ; =8
104347300:     	mov	w2, #0x10               ; =16
104347304:     	bl	0x10497e3dc <__RNvMs3_NtCscU4bHatjcek_5alloc7raw_vecNtB5_11RawVecInner10deallocateCs2qEGCSZWdXs_5gimli>
104347308:     	mov	x0, x19
10434730c:     	bl	0x1049dce24 <dyld_stub_binder+0x1049dce24>
104347310:     	bl	0x1049dc7e8 <__RNvNtCs4aqKKZZmviT_4core9panicking16panic_in_cleanup>
