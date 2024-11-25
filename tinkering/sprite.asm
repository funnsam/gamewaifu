INCLUDE "hardware.inc"

SECTION "VBlank handler", ROM0[$0040]
    jp VBlank

SECTION "Header", ROM0[$0100]
    jp EntryPoint
    ;ds $150 - @, 0 ; Make room for the header

SECTION "Code", ROM0[$0150]
EntryPoint:
    ; Shut down audio circuitry
    xor a, a
    ld [rNR52], a

    ; Do not turn the LCD off outside of VBlank
WaitVBlank:
    ld a, [rLY]
    cp 144
    jp c, WaitVBlank

    ; Turn the LCD off
    xor a, a
    ld [rLCDC], a

    ; Copy the tile data
    ld de, Tiles
    ld hl, $8000
    ld bc, TilesEnd - Tiles
CopyTiles:
    ld a, [de]
    ld [hli], a
    inc de
    dec bc
    ld a, b
    or a, c
    jp nz, CopyTiles

    ; Copy the tilemap
    ld hl, $9800
    ld bc, 1024
CopyTilemap:
    xor a, a
    ld [hli], a
    dec bc
    ld a, b
    or a, c
    jp nz, CopyTilemap

    ; Clean OAM
    xor a, a
    ld b, 160
    ld hl, _OAMRAM
ClearOam:
    ld [hli], a
    dec b
    jp nz, ClearOam

    ; Set up OAM
    ld hl, _OAMRAM
    ld a, 8 + 16
    ld [hli], a
    ld a, 0 + 8
    ld [hli], a
    xor a, a
    ld [hli], a
    ld [hli], a

    ld a, 8 + 16
    ld [hli], a
    ld a, 1 + 8
    ld [hli], a
    xor a, a
    ld [hli], a
    ld a, $10
    ld [hli], a

    ; Set up tile map
    ld hl, $9833 + 32
    inc [hl]

    ; Set palettes
    ld a, $e4
    ld [rBGP], a
    ld a, $1b
    ld [rOBP0], a
    ld a, $b1
    ld [rOBP1], a

    ; Turn the LCD on
    ld a, LCDCF_ON | LCDCF_BGON | LCDCF_BG8000 | LCDCF_OBJON | LCDCF_OBJ8
    ld [rLCDC], a

    ; Turn on VBlank interrupt
    ld a, IEF_VBLANK
    ld [rIE], a
    ei

:   jp :-

VBlank:
    ld b, b

    ; incr sprite x, wrapping to x=-8 at x=160
    ld hl, _OAMRAM + OAMA_X
    ld de, sizeof_OAM_ATTRS
    ld a, 160 + 8
    cp a, [hl]
    jr z, :+
    inc [hl]
    add hl, de
    inc [hl]
    jr :++
:   xor a, a
    ld [hl], a
    add hl, de
    inc a
    ld [hl], a
:

    ; incr counter
    ld hl, $9833
    inc [hl]

    reti

SECTION "Tile data", ROM0
Tiles:
    INCBIN "tileset.bin"
TilesEnd:
