* = $800

	.TEXT  "0-9, HOLA COMO TE VA", 0
* = $820
	.TEXT  "0-9, hola como te va", 0

* = $840
	.ENCODE
	.ENC "screen"
	.TEXT  "0-9, HOLA COMO TE VA", 0
	.ENDENCODE

* = $860
	.ENCODE
	.ENC "screen"
	.TEXT  "0-9, hola como te va", 0
	.ENDENCODE

	lda $800
	lda $801
	lda $810
	lda $81f
