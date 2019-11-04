prefix ?= /usr/local
bindir = $(prefix)/bin

TARGET = debug
DEBUG ?= 0
VENDOR ?= 0

ifeq ($(DEBUG),0)
	TARGET = release
	ARGS += --release
endif

ifneq ($(VENDOR),0)
	ARGS += --frozen
endif

NAME = tensorman
BIN = target/$(TARGET)/$(NAME)

.PHONY: all clean install

all: $(BIN)

$(BIN): vendor-check
	cargo build $(ARGS)

install:
	install -Dm0755 $(BIN) $(DESTDIR)$(bindir)/$(NAME)

vendor:
	rm .cargo -rf
	mkdir -p .cargo
	cargo vendor | head -n -1 > .cargo/config
	echo 'directory = "vendor"' >> .cargo/config
	tar pcf vendor.tar vendor
	rm -rf vendor

vendor-check:
ifneq ($(VENDOR),0)
	rm vendor -rf && tar pxf vendor.tar
endif

