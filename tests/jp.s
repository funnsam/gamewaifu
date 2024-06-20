SECTION "code", ROM0

jp_pos:
    jp :+
    ld a, 1
:   ld [0], a

jp_neg:
    jp .a
:   ld a, 0
    jp .b
.a
    jp :-
    ld a, 2
.b
    ld [0], a

done:
    halt
