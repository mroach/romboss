# Cross-compiling for Windows needs: mingw64-gcc, mingw64-winpthreads-static

BIN_NAME = romboss
PREFIX = /opt/local

WINDOWS_TARGETS = x86_64-pc-windows-gnu
LINUX_TARGETS = x86_64-unknown-linux-gnu
TARGETS = $(LINUX_TARGETS) $(WINDOWS_TARGETS)
XC_TARGETS = $(addprefix xc-target/,$(TARGETS))

.PHONY: build
build:
	cargo build --release

.PHONY: install
install:
	install target/release/$(BIN_NAME) $(PREFIX)/bin/$(BIN_NAME)

all: build install

.PHONY: $(XC_TARGETS)
$(XC_TARGETS):
	cargo build --release --target=$(@F)

.PHONY: all-targets
all-targets: $(XC_TARGETS)

.PHONY: release-prep
release-prep:
	@mkdir -p dist/bin
	@echo "Copying Windows binaries for $(WINDOWS_TARGETS)"
	@for t in "$(WINDOWS_TARGETS)"; do \
		cp target/$$t/release/$(BIN_NAME).exe dist/bin/$(BIN_NAME)-$$t.exe; \
	done

	@echo "Copying Linux binaries for $(LINUX_TARGETS)"
	@for t in "$(LINUX_TARGETS)"; do \
		cp target/$$t/release/$(BIN_NAME) dist/bin/$(BIN_NAME)-$$t; \
	done
