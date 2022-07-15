INCFLAGS = -I..
LDFLAGS = -L../target/debug
LIBS = -lneedle_capi

.PHONY: all

all: analyzer comparator

analyzer: analyzer.c
	gcc analyzer.c $(INCFLAGS) $(LDFLAGS) $(LIBS) -o analyzer.out

comparator: comparator.c
	gcc comparator.c $(INCFLAGS) $(LDFLAGS) $(LIBS) -o comparator.out
