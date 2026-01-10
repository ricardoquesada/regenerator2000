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

clean:
	rm -f test_text.prg test_pointer.prg test_petscii.prg test_labels.prg test_arrows.prg test_lohi_hilo.prg test_immediate_mode.prg test_illegal_opcodes.prg test_brk.prg test_acum.prg
