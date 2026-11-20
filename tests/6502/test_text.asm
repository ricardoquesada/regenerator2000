* = $800

	.TEXT  "0-9, HOLA COMO TE VA ""CARLOS"".", 0
* = $820
	.TEXT  "0-9, hola como te va ""carlos"".", 0

* = $900
	.ENCODE
	.ENC "screen"
	.TEXT  "0-9, HOLA COMO TE VA ""CARLOS"".", 0
	.ENDENCODE

* = $920
	.ENCODE
	.ENC "screen"
	.TEXT  "0-9, hola como te va ""carlos"".", 0
	.ENDENCODE

	; PETSCII
	.byte range(256)

	; ASCII
	.byte range(256)

* = $1000
	lda $800
	lda $801
	lda $810
	lda $81f

.block
	lda #$00
.bend

cacho .block
	lda #$00
.bend
