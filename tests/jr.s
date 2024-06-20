SECTION "code", ROM0

jr_pos:
    jr :+
    ld a, 1
:   ld [0], a

jr_neg:
    jr .a
:   ld a, 0
    jr .b
.a
    jr :-
    ld a, 2
.b
    ld [0], a

done:
    halt
