* = $1000

l0:
	bne l0
	beq l0
	bmi l0
	jsr l0
	jmp l0
	jmp (l0)

	bne l1
	beq l1
	bmi l1
	jsr l1
	jmp l1

l1: 	jmp l1
	jmp (l1)


l2:	bne l2
	bne l2
	bne l2
	bne l1
	bne l0

l3:	bne l3
