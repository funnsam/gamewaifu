SECTION "code", ROM0

    total_tests 2

jr_pos:
    ld a, 0

    jr :+
    ld a, 1
:   ld [RESULT], a

jr_neg:
    jr .a
:   ld a, 0
    jr .b
.a
    jr :-
    ld a, 2
.b
    ld [RESULT], a

    done
