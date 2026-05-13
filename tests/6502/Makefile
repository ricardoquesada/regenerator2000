EXO=~/bin/exomizer
PUCRUNCH=~/bin/pucrunch
DALI=~/bin/dali

all:
	64tass -o test_text.prg test_text.asm
	64tass -o test_pointer.prg test_pointer.asm
	64tass -o test_petscii.prg test_petscii.asm
	64tass -o test_labels.prg test_labels.asm
	64tass -o test_arrows.prg test_arrows.asm
	64tass -o test_lohi_hilo.prg test_lohi_hilo.asm
	64tass -o test_immediate_mode.prg test_immediate_mode.asm
	64tass -i -o test_illegal_opcodes.prg test_illegal_opcodes.asm
	64tass -i -o test_brk.prg test_brk.asm
	64tass -i -o test_acum.prg test_acum.asm
	64tass -i -o test_addressing.prg test_addressing.asm

crunch:
	${EXO} sfx 0x2e00 -o moving_tubes_lxt_exo.prg -t 64 moving_tubes_lxt_2e00.prg
	${PUCRUNCH} -x0x2e00  moving_tubes_lxt_2e00.prg moving_tubes_lxt_pucrunch.prg
	${DALI} --sfx 11776 -o moving_tubes_lxt_dali.prg moving_tubes_lxt_2e00.prg
	${EXO} sfx 0x1100 -o mule_exo.prg -t 64 mule_1100.prg
	${PUCRUNCH} -x0x1100  mule_1100.prg mule_pucrunch.prg
	${DALI} --sfx 4352 -o mule_dali.prg mule_1100.prg

clean:
	rm -f test_text.prg test_pointer.prg test_petscii.prg test_labels.prg test_arrows.prg test_lohi_hilo.prg test_immediate_mode.prg test_illegal_opcodes.prg test_brk.prg test_acum.prg test_addressing.prg
