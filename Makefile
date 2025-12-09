NAME := $(shell grep 'name =' Cargo.toml | head -n 1 | cut -d'"' -f2)
VERSION := $(shell grep '^version =' Cargo.toml | cut -d'"' -f2)
TARGET_ARCH ?= $(shell rustc -vV | sed -n 's/host: //p')
ARCH := $(shell echo "$(TARGET_ARCH)" | cut -d'-' -f1)
DBUS_NAME := org.shadowblip.InputPlumber
ALL_RS := $(shell find src -name '*.rs')
ALL_ROOTFS := $(shell find rootfs -type f)
PREFIX ?= /usr
CACHE_DIR := .cache
ENABLE_METRICS ?= 1

ifeq ($(ARCH),x86_64)
	ARCH_DEB = amd64
endif
ifeq ($(ARCH),aarch64)
	ARCH_DEB = arm64
endif

# Build variables
BUILD_TYPE ?= release
LOG_LEVEL ?= debug

# Docker image variables
IMAGE_NAME ?= inputplumber-builder
IMAGE_TAG ?= latest

# systemd-sysext variables 
SYSEXT_ID ?= _any
SYSEXT_VERSION_ID ?=

# Include any user defined settings
-include settings.mk

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
	install -D -m 755 target/$(TARGET_ARCH)/$(BUILD_TYPE)/$(NAME) \
		$(PREFIX)/bin/$(NAME)
	install -D -m 644 rootfs/usr/share/dbus-1/system.d/$(DBUS_NAME).conf \
		$(PREFIX)/share/dbus-1/system.d/$(DBUS_NAME).conf
	install -D -m 644 -t $(PREFIX)/share/polkit-1/actions/ \
		rootfs/usr/share/polkit-1/actions/*
	install -D -m 644 -t $(PREFIX)/share/polkit-1/rules.d/ \
		rootfs/usr/share/polkit-1/rules.d/*
	install -D -m 644 -t $(PREFIX)/lib/systemd/system/ \
		rootfs/usr/lib/systemd/system/*
	install -D -m 644 -t $(PREFIX)/lib/udev/hwdb.d/ \
		rootfs/usr/lib/udev/hwdb.d/*
	install -D -m 644 -t $(PREFIX)/lib/udev/rules.d/ \
		rootfs/usr/lib/udev/rules.d/*
	install -D -m 644 -t $(PREFIX)/share/$(NAME)/devices/ \
		rootfs/usr/share/$(NAME)/devices/*
	install -D -m 644 -t $(PREFIX)/share/$(NAME)/schema/ \
		rootfs/usr/share/$(NAME)/schema/*
	install -D -m 644 -t $(PREFIX)/share/$(NAME)/capability_maps/ \
		rootfs/usr/share/$(NAME)/capability_maps/*
	install -D -m 644 -t $(PREFIX)/share/$(NAME)/profiles/ \
		rootfs/usr/share/$(NAME)/profiles/*
		
	@echo ""
	@echo "Install completed. Enable service with:"
	@echo "  systemctl enable --now $(NAME)"

.PHONY: uninstall
uninstall: ## Uninstall inputplumber
	rm $(PREFIX)/bin/$(NAME)
	rm $(PREFIX)/share/dbus-1/system.d/$(DBUS_NAME).conf
	rm $(PREFIX)/share/polkit-1/actions/$(DBUS_NAME).policy
	rm $(PREFIX)/share/polkit-1/rules.d/$(DBUS_NAME).rules
	rm $(PREFIX)/lib/systemd/system/$(NAME).service
	rm $(PREFIX)/lib/systemd/system/$(NAME)-suspend.service
	rm $(PREFIX)/lib/udev/hwdb.d/59-inputplumber.hwdb
	rm $(PREFIX)/lib/udev/hwdb.d/60-inputplumber-autostart.hwdb
	rm $(PREFIX)/lib/udev/rules.d/90-inputplumber-autostart.rules
	rm $(PREFIX)/lib/udev/rules.d/99-inputplumber-device-setup.rules
	rm -rf $(PREFIX)/share/$(NAME)/devices/
	rm -rf $(PREFIX)/share/$(NAME)/schema/
	rm -rf $(PREFIX)/share/$(NAME)/capability_maps/
	rm -rf $(PREFIX)/share/$(NAME)/profiles/

##@ Development

.PHONY: build ## Build (Default: BUILD_TYPE=release)
build: target/$(TARGET_ARCH)/$(BUILD_TYPE)/$(NAME)

.PHONY: debug
debug: target/$(TARGET_ARCH)/debug/$(NAME)  ## Build debug build
target/$(TARGET_ARCH)/debug/$(NAME): $(ALL_RS) Cargo.lock
	cargo build --target $(TARGET_ARCH)

.PHONY: release
release: target/$(TARGET_ARCH)/release/$(NAME) ## Build release build
target/$(TARGET_ARCH)/release/$(NAME): $(ALL_RS) Cargo.lock
	cargo build --release --target $(TARGET_ARCH)

.PHONY: all
all: build debug ## Build release and debug builds

.PHONY: run
run: debug ## Build and run
	sudo LOG_LEVEL=$(LOG_LEVEL) ENABLE_METRICS=$(ENABLE_METRICS) ./target/$(TARGET_ARCH)/debug/$(NAME)

.PHONY: remote-debug
remote-debug: ## Start a remote debugging server on a remote device and stop the inputplumber service
	ssh -tt $(SSH_USER)@$(SSH_HOST) sudo systemctl stop inputplumber
	ssh -tt $(SSH_USER)@$(SSH_HOST) sudo lldb-server platform --listen '*:1234' --server

.PHONY: clean
clean: ## Remove build artifacts
	rm -rf target dist .cache

.PHONY: format
format: ## Run rustfmt on all source files
	rustfmt --edition 2021 $(ALL_RS)

.PHONY: test
test: test-autostart-rules ## Run all tests
	cargo clippy --all -- -D warnings
	cargo test -- --show-output

.PHONY: test-autostart-rules
test-autostart-rules: ## Test to ensure autostart rules are up-to-date
	RUST_BACKTRACE=0 cargo test config::config_test::check_autostart_rules -- --exact --show-output

.PHONY: test-polkit-usage
test-polkit-usage: ## Test to ensure polkit policy exists for all actions
	RUST_BACKTRACE=0 cargo test dbus::polkit_test::check_polkit_policies -- --exact --show-output

.PHONY: generate
generate: ## Generate schema definitions for configs
	cargo run --bin generate

.PHONY: setup
setup: /usr/share/dbus-1/system.d/$(DBUS_NAME).conf ## Install dbus policies
/usr/share/dbus-1/system.d/$(DBUS_NAME).conf:
	sudo cp $(PWD)/rootfs/usr/share/dbus-1/system.d/$(DBUS_NAME).conf \
		/usr/share/dbus-1/system.d/$(DBUS_NAME).conf
	sudo systemctl reload dbus

.PHONY: example
example:
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER='sudo -E' cargo run --example unified_gamepad

##@ Distribution

.PHONY: dist
dist: dist/$(NAME)-$(ARCH).tar.gz dist/$(NAME)_$(VERSION)-1_$(ARCH_DEB).deb dist/$(NAME)-$(VERSION)-1.$(ARCH).rpm dist/$(NAME)-$(ARCH).raw ## Create all redistributable versions of the project

.PHONY: dist-archive
dist-archive: dist/$(NAME)-$(ARCH).tar.gz ## Build a redistributable archive of the project
dist/$(NAME)-$(ARCH).tar.gz: build $(ALL_ROOTFS)
	rm -rf $(CACHE_DIR)/$(NAME)
	mkdir -p $(CACHE_DIR)/$(NAME)
	$(MAKE) install BUILD_TYPE=$(BUILD_TYPE) PREFIX=$(CACHE_DIR)/$(NAME)/usr NO_RELOAD=true
	mkdir -p dist
	tar cvfz $@ -C $(CACHE_DIR) $(NAME)
	cd dist && sha256sum $(NAME)-$(ARCH).tar.gz > $(NAME)-$(ARCH).tar.gz.sha256.txt

.PHONY: dist-deb
dist-deb: dist/$(NAME)_$(VERSION)-1_$(ARCH_DEB).deb ## Build a redistributable deb package
dist/$(NAME)_$(VERSION)-1_$(ARCH_DEB).deb: target/$(TARGET_ARCH)/release/$(NAME)
	mkdir -p dist
	cargo install --version 3.3.0 cargo-deb
	cargo deb --target $(TARGET_ARCH)
	cp ./target/$(TARGET_ARCH)/debian/$(NAME)_$(VERSION)-1_$(ARCH_DEB).deb dist
	cd dist && sha256sum $(NAME)_$(VERSION)-1_$(ARCH_DEB).deb > $(NAME)_$(VERSION)-1_$(ARCH_DEB).deb.sha256.txt

.PHONY: dist-rpm
dist-rpm: dist/$(NAME)-$(VERSION)-1.$(ARCH).rpm ## Build a redistributable RPM package
dist/$(NAME)-$(VERSION)-1.$(ARCH).rpm: target/$(TARGET_ARCH)/release/$(NAME)
	mkdir -p dist
	cargo install --version 0.14.1 cargo-generate-rpm
	cargo generate-rpm --target $(TARGET_ARCH)
	cp ./target/$(TARGET_ARCH)/generate-rpm/$(NAME)-$(VERSION)-1.$(ARCH).rpm dist
	cd dist && sha256sum $(NAME)-$(VERSION)-1.$(ARCH).rpm > $(NAME)-$(VERSION)-1.$(ARCH).rpm.sha256.txt

.PHONY: dist-ext
dist-ext: dist/$(NAME)-$(ARCH).raw ## Create a systemd-sysext extension archive
dist/$(NAME)-$(ARCH).raw: dist/$(NAME)-$(ARCH).tar.gz $(CACHE_DIR)/libiio $(CACHE_DIR)/libserialport
	@echo "Building redistributable systemd extension"
	mkdir -p dist
	rm -rf dist/$(NAME)-$(ARCH).raw $(CACHE_DIR)/$(NAME)-$(ARCH).raw
	cp dist/$(NAME)-$(ARCH).tar.gz $(CACHE_DIR)
	cd $(CACHE_DIR) && tar xvfz $(NAME)-$(ARCH).tar.gz $(NAME)/usr
	mkdir -p $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d
	echo ID=$(SYSEXT_ID) > $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d/extension-release.$(NAME)
	echo EXTENSION_RELOAD_MANAGER=1 >> $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d/extension-release.$(NAME)
	if [ -n "$(SYSEXT_VERSION_ID)" ]; then echo VERSION_ID=$(SYSEXT_VERSION_ID) >> $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d/extension-release.$(NAME); fi

	# Install libserialport in the extension for libiio compatibility in SteamOS
	cp -r $(CACHE_DIR)/libserialport/usr/lib/libserialport* $(CACHE_DIR)/$(NAME)/usr/lib
	
	@# Install libiio in the extension for SteamOS compatibility
	cp -r $(CACHE_DIR)/libiio/usr/lib/libiio* $(CACHE_DIR)/$(NAME)/usr/lib

	@# Build the extension archive
	cd $(CACHE_DIR) && mksquashfs $(NAME) $(NAME)-$(ARCH).raw
	rm -rf $(CACHE_DIR)/$(NAME)
	mv $(CACHE_DIR)/$(NAME)-$(ARCH).raw $@
	cd dist && sha256sum $(NAME)-$(ARCH).raw > $(NAME)-$(ARCH).raw.sha256.txt

.PHONY: $(CACHE_DIR)/libiio
$(CACHE_DIR)/libiio:
	rm -rf $(CACHE_DIR)/libiio*
	mkdir -p $(CACHE_DIR)/libiio
ifeq ($(ARCH),x86_64)
	VERSION=$$(curl -s https://archlinuxarm.org/packages/aarch64/libiio | grep '<h1>libiio ' | cut -d'>' -f2 | cut -d'<' -f1 | cut -d' ' -f2) && \
	curl -L http://mirrors.mit.edu/archlinux/extra/os/x86_64/libiio-$${VERSION}-x86_64.pkg.tar.zst \
		-o $(CACHE_DIR)/libiio.tar.zst
	zstd -d $(CACHE_DIR)/libiio.tar.zst
	tar xvf $(CACHE_DIR)/libiio.tar -C $(CACHE_DIR)/libiio
endif
ifeq ($(ARCH),aarch64)
	VERSION=$$(curl -s https://archlinuxarm.org/packages/aarch64/libiio | grep '<h1>libiio ' | cut -d'>' -f2 | cut -d'<' -f1 | cut -d' ' -f2) && \
	curl -L http://mirror.archlinuxarm.org/aarch64/extra/libiio-$${VERSION}-aarch64.pkg.tar.xz \
		-o $(CACHE_DIR)/libiio.tar.xz
	tar xvf $(CACHE_DIR)/libiio.tar.xz -C $(CACHE_DIR)/libiio
endif

.PHONY: $(CACHE_DIR)/libserialport
$(CACHE_DIR)/libserialport:
	rm -rf $(CACHE_DIR)/libserialport*
	mkdir -p $(CACHE_DIR)/libserialport
ifeq ($(ARCH),x86_64)
	curl -L http://mirrors.mit.edu/archlinux/extra/os/x86_64/libserialport-0.1.2-1-x86_64.pkg.tar.zst \
	  -o $(CACHE_DIR)/libserialport.tar.zst
	zstd -d $(CACHE_DIR)/libserialport.tar.zst
	tar xvf $(CACHE_DIR)/libserialport.tar -C $(CACHE_DIR)/libserialport
endif
ifeq ($(ARCH),aarch64)
	curl -L http://mirror.archlinuxarm.org/aarch64/extra/libserialport-0.1.2-1-aarch64.pkg.tar.xz \
		-o $(CACHE_DIR)/libserialport.tar.xz
	tar xvf $(CACHE_DIR)/libserialport.tar.xz -C $(CACHE_DIR)/libserialport
endif

.PHONY: update-pkgbuild-hash
update-pkgbuild-hash: dist/$(NAME)-$(ARCH).tar.gz ## Update the PKGBUILD hash
	sed -i "s#^sha256sums=.*#sha256sums=('$$(cat dist/$(NAME)-$(ARCH).tar.gz.sha256.txt | cut -d' ' -f1)')#g" \
		pkg/archlinux/PKGBUILD

.PHONY: dbus-xml
dbus-xml: ## Generate DBus XML spec from running InputPlumber
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/Manager > ./bindings/dbus-xml/org.shadowblip.Input.Manager.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/CompositeDevice0 > ./bindings/dbus-xml/org.shadowblip.Input.CompositeDevice.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/devices/target/dbus0 > ./bindings/dbus-xml/org.shadowblip.Input.DBusDevice.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/devices/target/keyboard0 > ./bindings/dbus-xml/org.shadowblip.Input.Keyboard.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/devices/target/mouse0 > ./bindings/dbus-xml/org.shadowblip.Input.Mouse.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/devices/target/gamepad0 > ./bindings/dbus-xml/org.shadowblip.Input.Gamepad.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/devices/source/event0 > ./bindings/dbus-xml/org.shadowblip.Input.Source.EventDevice.xml
	busctl introspect org.shadowblip.InputPlumber \
		--xml-interface /org/shadowblip/InputPlumber/devices/source/hidraw0 > ./bindings/dbus-xml/org.shadowblip.Input.Source.HIDRawDevice.xml

XSL_TEMPLATE := ./docs/dbus2markdown.xsl
.PHONY: docs
docs: ## Generate markdown docs for DBus interfaces
	mkdir -p docs
	xsltproc --novalid -o docs/manager.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Manager.xml
	sed -i 's/DBus Interface API/Manager DBus Interface API/g' ./docs/manager.md
	xsltproc --novalid -o docs/composite_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.CompositeDevice.xml
	sed -i 's/DBus Interface API/CompositeDevice DBus Interface API/g' ./docs/composite_device.md
	xsltproc --novalid -o docs/target_dbus.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.DBusDevice.xml
	sed -i 's/DBus Interface API/DBusDevice DBus Interface API/g' ./docs/target_dbus.md
	xsltproc --novalid -o docs/target_keyboard.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Keyboard.xml
	sed -i 's/DBus Interface API/Keyboard DBus Interface API/g' ./docs/target_keyboard.md
	xsltproc --novalid -o docs/target_mouse.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Mouse.xml
	sed -i 's/DBus Interface API/Mouse DBus Interface API/g' ./docs/target_mouse.md
	xsltproc --novalid -o docs/target_gamepad.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Gamepad.xml
	sed -i 's/DBus Interface API/Gamepad DBus Interface API/g' ./docs/target_gamepad.md
	xsltproc --novalid -o docs/source_event_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Source.EventDevice.xml
	sed -i 's/DBus Interface API/Source EventDevice DBus Interface API/g' ./docs/source_event_device.md
	xsltproc --novalid -o docs/source_hidraw_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Source.HIDRawDevice.xml
	sed -i 's/DBus Interface API/Source HIDRaw DBus Interface API/g' ./docs/source_hidraw_device.md

# Refer to .releaserc.yaml for release configuration
.PHONY: sem-release 
sem-release: ## Publish a release with semantic release 
	npx semantic-release

# E.g. make in-docker TARGET=build
.PHONY: in-docker
in-docker:
	@# Run the given make target inside Docker
	docker build -t $(IMAGE_NAME):$(IMAGE_TAG) .
	mkdir -p $(CACHE_DIR)/home
	docker run --rm \
		-v "$(PWD):$(PWD)" \
		-v "$(PWD)/$(CACHE_DIR)/home:/home/build" \
		--workdir "$(PWD)" \
		-e HOME=/home/build \
		-e CARGO_HOME=/home/build/.cargo \
		-e ARCH=$(ARCH) \
		-e TARGET_ARCH=$(TARGET_ARCH) \
		-e BUILD_TYPE=$(BUILD_TYPE) \
		-e PKG_CONFIG_SYSROOT_DIR="/usr/$(ARCH)-linux-gnu" \
		--user $(shell id -u):$(shell id -g) \
		$(IMAGE_NAME):$(IMAGE_TAG) \
		make BUILD_TYPE=$(BUILD_TYPE) $(TARGET)

##@ Deployment

.PHONY: deploy
deploy: deploy-ext ## Build and deploy to a remote device

.PHONY: deploy-ext
deploy-ext: dist-ext ## Build and deploy systemd extension to a remote device
	ssh $(SSH_USER)@$(SSH_HOST) mkdir -p .var/lib/extensions
	scp dist/$(NAME)-$(ARCH).raw $(SSH_USER)@$(SSH_HOST):~/.var/lib/extensions
	ssh -t $(SSH_USER)@$(SSH_HOST) sudo systemd-sysext refresh
	ssh $(SSH_USER)@$(SSH_HOST) systemd-sysext status

