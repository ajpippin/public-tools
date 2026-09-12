#= Makefile ====================================================================
# 
#   DESC:	Makefile for 
# 
#   AUTHOR:     %%author%%
# 
#   MODIFIED:   %%date%%  %%user%%  Original Version
# 
#   VERSION:    %%identity%%
# 
# ==============================================================================

-include ${HOME}/Library/abp/Makefile/common
default: all


#--[ Defaults ]-----------------------------------------------------------------

CC =gcc
CCC =g++
LD =gcc

MY_CFLAGS =-I${HOME}/Library/abp/Include
MY_LDFLAGS=
MY_LIBS=


#--[ Localization ]-------------------------------------------------------------

#--[ Programs and Flags ]-------------------------------------------------------

CC	=gcc
DEBUG	=-ggdb -DDEBUG
CFLAGS	=-O2 $(DEBUG) $(MORECFLAGS)

CCC	=g++
CCFLAGS	=$(CFLAGS)

LD	=$(CC)
LDFLAGS	=$(MORELDFLAGS)
LIBS	=$(MORELIBS)


#--[ Source ]-------------------------------------------------------------------

# -- Binary
#
NAM1	=task1
HDR1	=
SRC1	=$(HDR1:%.hh=%.cc)
OBJ1	=$(SRC1:%.cc=$(OBJDIR)/%.o)
TSK1	=$(BINDIR)/$(NAM1)

# -- Library
#
NAM2	=libtask2.a
HDR2	=
SRC2	=$(HDR2:%.hh=%.cc)
OBJ2	=$(SRC2:%.cc=$(OBJDIR)/%.o)
TSK2	=$(LIBDIR)/$(NAM2)


# -- All source code.
#
NAMS	=$(NAM1) $(NAM2)
HDRS	=$(HDR1) $(HDR2)
SRCS	=$(SRC1) $(SRC2)
OBJS	=$(OBJ1) $(OBJ2)
TSKS	=$(TSK1) $(TSK2)


#--[ Targets ]------------------------------------------------------------------

$(TSKS): Makefile
all: $(TSKS)
$(NAM1): $(TSK1)
$(NAM2): $(TSK2)


# -- Program
#
$(TSK1): $(OBJ1)
	$(LINK.cc) -o $@ $(OBJ1) $(LIBS)


# -- Library
#
$(TSK2): $(OBJ2)
	$(AR) $@ $(OBJ2)
	$(RANLIB) $@


#--[ Miscellaneous Targets ]----------------------------------------------------

.PHONY:	all clean tags depend help dirs install
.PHONY: install-bin install-lib

clean:
	$(RMFORCE) $(TASKS) $(OBJS) core tags Makefile.bak

tags:
	ctags $(HDRS) $(SRCS)

depend: 
	$(DEPEND) -- $(CFLAGS) -- $(SRCS) > /dev/null 2>&1 &&\
	    $(RMFORCE) Makefile.bak

help:
	@echo "Available targets:"
	@echo "  <default>   = all"
	@echo "  all         = $(TSKS)"
	@echo "  clean       = Clean up files."
	@echo "  tags        = Make a tags file."
	@echo "  depend      = Recreate makefile dependancies"
	@echo "  help        = This message."
	@echo "  dirs        = Make directories."
	@echo "  install     = Install into proper place."
	@echo "  install-bin = Install binaries --> $(INSTBING)"
	@echo "  install-lib = Install binaries --> $(INSTLIBG)"

# -- Installation
#
dirs:
	mkdir -p \
	$(BINDIR) \
	$(LIBDIR) \
	$(OBJDIR)

install: 
	mkdir -p $(INSTHDR) $(INSTLIB) $(INSTLIBG) $(INSTBIN) $(INSTBING)
	@echo "Nothing to install."

install-bin: $(INSTBING)/$(NAM1)

$(INSTBING)/$(NAM1): $(TSK1)
	$(INSTALL) -m 0755 -s $< `dirname $@`

install-lib: $(INSTLIBG)/$(NAM2)

#===============================================================================

# DO NOT DELETE THIS LINE -- make depend needs it ------------------------------
