def RESULT equ $ff01
def EXIT   equ $ff02
def AMOUNT equ $ff02

MACRO done
    xor a, a
    ld [EXIT], a
:   jr :-
ENDM

MACRO total_tests
    ld a, \1
    ld [AMOUNT], a
ENDM
