INCFLAGS = -I..
LDFLAGS = -L../target/debug
LIBS = -lneedle_capi

.PHONY: all

all: analyzer

analyzer: analyzer.c
	gcc analyzer.c $(INCFLAGS) $(LDFLAGS) $(LIBS) -o analyzer.out
