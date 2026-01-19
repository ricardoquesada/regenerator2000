* = $1000

	; lo/hi
	.for i := 0, i < 256, i += 1
		.byte	<($d000+i)
	.endfor
	.for i := 0, i < 256, i += 1
		.byte	>($d000+i)
	.endfor

	; hi/lo
	.for i := 0, i < 256, i += 1
		.byte	>($c000+i)
	.endfor
	.for i := 0, i < 256, i += 1
		.byte	<($c000+i)
	.endfor
