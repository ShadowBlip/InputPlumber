NAME := $(shell grep 'name =' Cargo.toml | head -n 1 | cut -d'"' -f2)
DBUS_NAME := org.shadowblip.InputPlumber
ALL_RS := $(shell find src -name '*.rs')
PREFIX ?= /usr

##@ General

# The help target prints out all targets with their descriptions organized
# beneath their categories. The categories are represented by '##@' and the
# target descriptions by '##'. The awk commands is responsible for reading the
# entire set of makefiles included in this invocation, looking for lines of the
# file as xyz: ## something, and then pretty-format the target and help. Then,
# if there's a line with ##@ something, that gets pretty-printed as a category.
# More info on the usage of ANSI control characters for terminal formatting:
# https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_parameters
# More info on the awk command:
# http://linuxcommand.org/lc3_adv_awk.php

.PHONY: help
help: ## Display this help.
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m<target>\033[0m\n"} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

.PHONY: install
install: build ## Install inputplumber to the given prefix (default: PREFIX=/usr)
	install -D -m 755 target/release/$(NAME) \
		$(PREFIX)/bin/$(NAME)
	install -D -m 644 rootfs/usr/share/dbus-1/system.d/$(DBUS_NAME).conf \
		$(PREFIX)/share/dbus-1/system.d/$(DBUS_NAME).conf
	install -D -m 644 rootfs/usr/lib/systemd/system/$(NAME).service \
		$(PREFIX)/lib/systemd/system/$(NAME).service
	systemctl reload dbus

.PHONY: uninstall
uninstall: ## Uninstall inputplumber
	rm $(PREFIX)/bin/$(NAME)
	rm $(PREFIX)/share/dbus-1/system.d/$(DBUS_NAME).conf
	rm $(PREFIX)/lib/systemd/system/$(NAME).service

##@ Development

.PHONY: debug
debug: target/debug/$(NAME)  ## Build debug build
target/debug/$(NAME): $(ALL_RS) Cargo.lock
	cargo build

.PHONY: build
build: target/release/$(NAME) ## Build release build
target/release/$(NAME): $(ALL_RS) Cargo.lock
	cargo build --release

.PHONY: all
all: build debug ## Build release and debug builds

.PHONY: run
run: setup debug ## Build and run
	sudo ./target/debug/$(NAME)

.PHONY: clean
clean: ## Remove build artifacts
	rm -rf target

.PHONY: format
format: ## Run rustfmt on all source files
	rustfmt --edition 2021 $(ALL_RS)

.PHONY: test
test: ## Run all tests
	cargo test -- --show-output

.PHONY: setup
setup: /usr/share/dbus-1/system.d/$(DBUS_NAME).conf ## Install dbus policies
/usr/share/dbus-1/system.d/$(DBUS_NAME).conf:
	sudo ln $(PWD)/rootfs/usr/share/dbus-1/system.d/$(DBUS_NAME).conf \
		/usr/share/dbus-1/system.d/$(DBUS_NAME).conf
	sudo systemctl reload dbus
