SECTION "code", ROM0

    total_tests 2

jp_pos:
    ld a, 0

    jp :+
    ld a, 1
:   ld [RESULT], a

jp_neg:
    jp .a
:   ld a, 0
    jp .b
.a
    jp :-
    ld a, 2
.b
    ld [RESULT], a

    done
