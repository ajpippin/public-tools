#= makefile ====================================================================
# 
#   DESC:	Makefile for 
# 
#   AUTHOR:     %%fullname%%
# 
#   MODIFIED:   %%date%%  %%user%%  Original Version
# 
# ==============================================================================

include ${HOME}/.abp/Makefile/common
default: all


#--[ Programs and Flags ]-------------------------------------------------------

JFLAGS	=


#--[ Source ]-------------------------------------------------------------------

NAME1	=FileOne
TASK1	=$(NAME1:%=%.class)
STUB1	=$(NAME1:%=%_Stub.class)
SKEL1	=$(NAME1:%=%_Skel.class)

NAME2	=FileOne
TASK2	=$(NAME2:%=%.class)
STUB2	=
SKEL2	=

# -- All source files.
#
NAMES	=$(NAME1) $(NAME2)
TASKS	=$(TASK1) $(TASK2)
STUBS	=$(STUB1) $(STUB2)
SKELS	=$(SKEL1) $(SKEL2)

SRCS	=$(NAMES:%=%.java)
OBJS	=$(TASKS) $(STUBS) $(SKELS)


#--[ Targets ]------------------------------------------------------------------

$(TSKS): Makefile

all: $(TSKS)
$(NAM1): $(TSK1)
$(NAM2): $(TSK2)


#--[ Install Targets ]----------------------------------------------------------

$(INSTJAVA)/%.class: %.class
	$(INSTALL) -m 0644 $< $(INSTJAVA)


#--[ Miscellaneous Targets ]----------------------------------------------------

.PHONY:	all clean install

clean:
	@echo -ne "Cleaning known files..."
	@$(MAKE) -s clean-quiet && echo "  [done]"

install: 
	@mkdir -p $(INSTJAVA)
	@echo "Nothing to install."


#===============================================================================
