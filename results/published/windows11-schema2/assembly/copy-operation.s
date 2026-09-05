_RNCNvNtCsjYHqGo6J17F_7support14copy_roundtrip18run_copy_roundtrips_0B5_:
.Lfunc_begin264:
.seh_proc _RNCNvNtCsjYHqGo6J17F_7support14copy_roundtrip18run_copy_roundtrips_0B5_
	.seh_handler __CxxFrameHandler3, @unwind, @except
	pushq	%rbp
	.seh_pushreg %rbp
	pushq	%r14
	.seh_pushreg %r14
	pushq	%rsi
	.seh_pushreg %rsi
	pushq	%rdi
	.seh_pushreg %rdi
	pushq	%rbx
	.seh_pushreg %rbx
	subq	$48, %rsp
	.seh_stackalloc 48
	leaq	48(%rsp), %rbp
	.seh_setframe %rbp, 48
	.seh_endprologue
	movq	$-2, -8(%rbp)
	movq	(%rcx), %r14
	movq	8(%rcx), %rdi
	movq	16(%r14), %r8
	movq	16(%rdi), %rdx
	cmpq	%rdx, %r8
	jne	.LBB460_1
	movq	%rcx, %rsi
	movq	8(%r14), %rcx
	movq	8(%rdi), %rdx
	callq	memcpy
	movq	%r14, -16(%rbp)
	leaq	-16(%rbp), %rax
	#APP
	#NO_APP
	movq	16(%rsi), %rbx
	movq	16(%rbx), %r8
	movq	16(%r14), %rdx
	cmpq	%rdx, %r8
	jne	.LBB460_5
	movq	8(%rbx), %rcx
	movq	8(%r14), %rdx
	callq	memcpy
	movq	%rbx, -16(%rbp)
	leaq	-16(%rbp), %rax
	#APP
	#NO_APP
	movq	16(%rbx), %rdx
	testq	%rdx, %rdx
	je	.LBB460_7
	movq	8(%rbx), %rcx
	callq	_RNvNtCs81C54RvG40j_7harness5fault18transform_response
	movq	16(%rbx), %r8
	movq	24(%rsi), %r14
	movq	16(%r14), %rcx
	cmpq	%r8, %rcx
	jne	.LBB460_10
.LBB460_11:
	movq	8(%r14), %rcx
	movq	8(%rbx), %rdx
	callq	memcpy
	movq	%r14, -16(%rbp)
	leaq	-16(%rbp), %rax
	#APP
	#NO_APP
	movq	32(%rsi), %rsi
	movq	16(%rsi), %r8
	movq	16(%r14), %rdx
	cmpq	%rdx, %r8
	jne	.LBB460_12
	movq	8(%rsi), %rcx
	movq	8(%r14), %rdx
	callq	memcpy
	movq	%rsi, -16(%rbp)
	leaq	-16(%rbp), %rax
	#APP
	#NO_APP
	movq	8(%rdi), %rcx
	movq	16(%rdi), %rdx
	movq	8(%rsi), %r8
	movq	16(%rsi), %r9
	callq	_RNvNtCs81C54RvG40j_7harness7payload26check_response_and_advance
	testq	%rax, %rax
	je	.LBB460_14
	movq	%rax, %rsi
	movq	%rax, -16(%rbp)
	callq	_RNvCs1njKG4L9aB3_7___rustc35___rust_no_alloc_shim_is_unstable_v2
	movl	$8, %ecx
	movl	$8, %edx
	callq	_RNvCs1njKG4L9aB3_7___rustc12___rust_alloc
	testq	%rax, %rax
	je	.LBB460_16
	movq	%rsi, (%rax)
	jmp	.LBB460_20
.LBB460_7:
	xorl	%r8d, %r8d
	movq	24(%rsi), %r14
	movq	16(%r14), %rcx
	cmpq	%r8, %rcx
	je	.LBB460_11
.LBB460_10:
	leaq	anon.038aa1c19b7fee9ecf3bcbc4ba39b7b7.621(%rip), %rax
	movq	%r8, %rdx
	movq	%rax, %r8
	callq	_RNvNvNtCs8xEFJqa6dYS_4core5slice20copy_from_slice_impl17len_mismatch_fail
	ud2
.LBB460_14:
	xorl	%eax, %eax
.LBB460_20:
	.seh_startepilogue
	addq	$48, %rsp
	popq	%rbx
	popq	%rdi
	popq	%rsi
	popq	%r14
	popq	%rbp
	.seh_endepilogue
	retq
.LBB460_1:
	leaq	anon.038aa1c19b7fee9ecf3bcbc4ba39b7b7.619(%rip), %rax
	movq	%r8, %rcx
	movq	%rax, %r8
	callq	_RNvNvNtCs8xEFJqa6dYS_4core5slice20copy_from_slice_impl17len_mismatch_fail
	ud2
.LBB460_5:
	leaq	anon.038aa1c19b7fee9ecf3bcbc4ba39b7b7.620(%rip), %rax
	movq	%r8, %rcx
	movq	%rax, %r8
	callq	_RNvNvNtCs8xEFJqa6dYS_4core5slice20copy_from_slice_impl17len_mismatch_fail
	ud2
.LBB460_12:
	leaq	anon.038aa1c19b7fee9ecf3bcbc4ba39b7b7.622(%rip), %rax
	movq	%r8, %rcx
	movq	%rax, %r8
	callq	_RNvNvNtCs8xEFJqa6dYS_4core5slice20copy_from_slice_impl17len_mismatch_fail
	ud2
.LBB460_16:
.Ltmp5998:
	movl	$8, %ecx
	movl	$8, %edx
	callq	_RNvNtCsfQUUnEvlYZE_5alloc5alloc18handle_alloc_error
	nop
.Ltmp5999:
	ud2
	.seh_handlerdata
	.long	$cppxdata$_RNCNvNtCsjYHqGo6J17F_7support14copy_roundtrip18run_copy_roundtrips_0B5_@IMGREL
	.section	.text,"xr",one_only,_RNCNvNtCsjYHqGo6J17F_7support14copy_roundtrip18run_copy_roundtrips_0B5_,unique,496
	.seh_endproc
