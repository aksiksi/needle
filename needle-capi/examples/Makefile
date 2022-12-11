INCFLAGS = -I..
CFLAGS = -Wall -Werror
LDFLAGS = -L../target/debug
LIBS = -lneedle_capi

.PHONY: all

all: analyzer comparator full

analyzer: analyzer.c
	gcc analyzer.c $(CFLAGS) $(INCFLAGS) $(LDFLAGS) $(LIBS) -o analyzer.out

comparator: comparator.c
	gcc comparator.c $(CFLAGS) $(INCFLAGS) $(LDFLAGS) $(LIBS) -o comparator.out

full: full.c
	gcc full.c $(CFLAGS) $(INCFLAGS) $(LDFLAGS) $(LIBS) -o full.out
