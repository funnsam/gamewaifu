SOURCES := $(shell find . -name '*.s')
BINS := $(SOURCES:%.s=%.bin)

all: $(BINS)

%.bin: %.s tests.inc
	rgbasm $*.s -o $@ -P tests.inc
	rgblink $@ -o $@ -l link.ld

clean:
	- rm *.bin
