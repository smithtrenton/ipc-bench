_RNvNtCs4WLemonnQlt_7harness7payload26check_response_and_advance:
.seh_proc _RNvNtCs4WLemonnQlt_7harness7payload26check_response_and_advance
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$144, %rsp
	.seh_stackalloc 144
	.seh_endprologue
	cmpq	%rdx, %r9
	sete	%al
	cmpq	$9, %r9
	setae	%dl
	testb	%dl, %al
	jne	.LBB141_1
	leaq	anon.c619f3efcd4644d86699a7891a7f797d.282(%rip), %rdx
	movl	$25, %r8d
	movb	$21, %cl
	.seh_startepilogue
	addq	$144, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	.seh_endepilogue
	jmp	_RINvMNtNtCsfQUUnEvlYZE_5alloc2io5errorNtNtNtCs8xEFJqa6dYS_4core2io5error5Error3newReECse0R8cuyqEa_3std
.LBB141_1:
	movq	(%rcx), %rdi
	movq	%rdi, 48(%rsp)
	movl	_tls_index(%rip), %eax
	movq	%gs:88, %rdx
	movq	(%rdx,%rax,8), %rax
	cmpb	$0, _RNvNCNKNvNtCs4WLemonnQlt_7harness7payload15FULL_VALIDATION0s_023___RUST_STD_INTERNAL_VAL@SECREL32(%rax)
	movl	$8, %r11d
	cmovneq	%r9, %r11
	testl	$1023, %edi
	cmoveq	%r9, %r11
	movq	$0, 40(%rsp)
	leal	1(%rdi), %eax
	movb	%al, 39(%rsp)
	cmpb	%al, (%r8)
	jne	.LBB141_2
	movl	$1, %esi
	movq	$-7, %r10
	movabsq	$367465021388636487, %rbx
	.p2align	4
.LBB141_7:
	movq	%r10, %rax
	mulq	%rbx
	movq	%rsi, 40(%rsp)
	cmpq	$7, %rsi
	jbe	.LBB141_9
	movl	%r10d, %eax
	subl	%edx, %eax
	shrl	%eax
	addl	%edx, %eax
	shrl	$7, %eax
	leal	(%rax,%rax,4), %eax
	movzbl	%al, %eax
	addl	%esi, %eax
	addb	$-8, %al
	jmp	.LBB141_10
	.p2align	4
.LBB141_9:
	movzbl	(%rcx,%rsi), %eax
.LBB141_10:
	movb	%al, 39(%rsp)
	cmpq	%rsi, %r9
	je	.LBB141_14
	cmpb	%al, (%r8,%rsi)
	jne	.LBB141_3
	incq	%rsi
	incq	%r10
	cmpq	%rsi, %r11
	jne	.LBB141_7
	movq	%r8, 56(%rsp)
	movq	%r9, 64(%rsp)
	leaq	56(%rsp), %rax
	#APP
	#NO_APP
	movq	%rcx, %rsi
	movq	%r8, %rdx
	movq	%r9, %r8
	movq	%r9, %rbx
	callq	memcpy
	movq	%rsi, 56(%rsp)
	movq	%rbx, 64(%rsp)
	leaq	56(%rsp), %rax
	#APP
	#NO_APP
	incq	%rdi
	movq	%rdi, (%rsi)
	xorl	%eax, %eax
	jmp	.LBB141_5
.LBB141_2:
	xorl	%esi, %esi
.LBB141_3:
	addq	%rsi, %r8
	leaq	48(%rsp), %rax
	movq	%rax, 56(%rsp)
	leaq	_RNvXsd_NtNtNtCs8xEFJqa6dYS_4core3fmt3num3impyNtB9_7Display3fmt(%rip), %rax
	movq	%rax, 64(%rsp)
	leaq	40(%rsp), %rax
	movq	%rax, 72(%rsp)
	leaq	_RNvXsi_NtNtNtCs8xEFJqa6dYS_4core3fmt3num3impjNtB9_7Display3fmt(%rip), %rax
	movq	%rax, 80(%rsp)
	leaq	39(%rsp), %rax
	movq	%rax, 88(%rsp)
	leaq	_RNvXNtNtNtCs8xEFJqa6dYS_4core3fmt3num3imphNtB6_7Display3fmt(%rip), %rax
	movq	%rax, 96(%rsp)
	movq	%r8, 104(%rsp)
	movq	%rax, 112(%rsp)
	leaq	anon.c619f3efcd4644d86699a7891a7f797d.281(%rip), %rdx
	leaq	120(%rsp), %rsi
	leaq	56(%rsp), %r8
	movq	%rsi, %rcx
	callq	_RNvNvNtCsfQUUnEvlYZE_5alloc3fmt6format12format_inner
	movb	$21, %cl
	movq	%rsi, %rdx
	callq	_RINvMNtNtCsfQUUnEvlYZE_5alloc2io5errorNtNtNtCs8xEFJqa6dYS_4core2io5error5Error3newNtNtB7_6string6StringECs4WLemonnQlt_7harness
	nop
.LBB141_5:
	.seh_startepilogue
	addq	$144, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	.seh_endepilogue
	retq
.LBB141_14:
	leaq	anon.c619f3efcd4644d86699a7891a7f797d.280(%rip), %r8
	movq	%r9, %rcx
	movq	%r9, %rdx
	callq	_RNvNtCs8xEFJqa6dYS_4core9panicking18panic_bounds_check
	ud2
	.seh_endproc
