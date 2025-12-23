; ZP FIELDS
f01 = $01
f41 = $41

; ZP ABSOLUTE ADDRESSES
a20 = $20

; ABSOLUTE ADDRESSES
a200F = $200F

* = $0800
                        .TEXT "HOLA COMO T"
                        EOR a20
                        LSR f41,X
                        BRK 
                        .BYTE $6C        ; Partial Screencode
                        .BYTE $6F        ; Partial Screencode
                        .BYTE $77        ; Partial Screencode
                        .BYTE $65        ; Partial Screencode
                        .BYTE $72        ; Partial Screencode
                        .BYTE $63        ; Partial Screencode
                        .BYTE $61        ; Partial Screencode
                        .BYTE $73        ; Partial Screencode
                        .BYTE $65        ; Partial Screencode
                        .TEXT "@HOLA CO"
                        .ENDENCODE 
                        ORA a200F
                        .BYTE $14        ; Invalid or partial instruction
                        ORA a20
                        ASL f01,X
                        BRK 
                        .BYTE $4C        ; Partial Screencode
                        .BYTE $4F        ; Partial Screencode
                        .BYTE $57        ; Partial Screencode
                        .BYTE $45        ; Partial Screencode
                        .BYTE $52        ; Partial Screencode
                        .BYTE $43        ; Partial Screencode
                        .BYTE $41        ; Partial Screencode
                        .BYTE $53        ; Partial Screencode
                        .BYTE $45        ; Partial Screencode
                        .TEXT "@"
                        .BYTE $48        ; Partial Screencode
                        .BYTE $4F        ; Partial Screencode
                        .BYTE $4C        ; Invalid or partial instruction
                        .BYTE $41        ; Invalid or partial instruction
