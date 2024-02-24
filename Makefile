NAME := $(shell grep 'name =' Cargo.toml | head -n 1 | cut -d'"' -f2)
VERSION := $(shell grep '^version =' Cargo.toml | cut -d'"' -f2)
ARCH := $(shell uname -m)
DBUS_NAME := org.shadowblip.InputPlumber
ALL_RS := $(shell find src -name '*.rs')
ALL_ROOTFS := $(shell find rootfs -type f)
PREFIX ?= /usr
CACHE_DIR := .cache

# Docker image variables
IMAGE_NAME ?= inputplumber-builder
IMAGE_TAG ?= latest

# systemd-sysext variables 
SYSEXT_ID ?= steamos
SYSEXT_VERSION_ID ?= 3.5.15

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
	install -D -m 755 target/release/$(NAME) \
		$(PREFIX)/bin/$(NAME)
	install -D -m 644 rootfs/usr/share/dbus-1/system.d/$(DBUS_NAME).conf \
		$(PREFIX)/share/dbus-1/system.d/$(DBUS_NAME).conf
	install -D -m 644 rootfs/usr/lib/systemd/system/$(NAME).service \
		$(PREFIX)/lib/systemd/system/$(NAME).service
	install -D -m 644 rootfs/usr/share/$(NAME)/devices/steam_deck.yaml \
		$(PREFIX)/share/$(NAME)/devices/steam_deck.yaml
	@echo ""
	@echo "Install completed. Enable service with:" 
	@echo "  systemctl enable --now $(NAME)"

.PHONY: uninstall
uninstall: ## Uninstall inputplumber
	rm $(PREFIX)/bin/$(NAME)
	rm $(PREFIX)/share/dbus-1/system.d/$(DBUS_NAME).conf
	rm $(PREFIX)/lib/systemd/system/$(NAME).service
	rm $(PREFIX)/share/$(NAME)/devices/steam_deck.yaml

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
	sudo cp $(PWD)/rootfs/usr/share/dbus-1/system.d/$(DBUS_NAME).conf \
		/usr/share/dbus-1/system.d/$(DBUS_NAME).conf
	sudo systemctl reload dbus

##@ Distribution

.PHONY: dist
dist: dist/$(NAME).tar.gz dist/$(NAME)-$(VERSION)-1.$(ARCH).rpm dist/$(NAME).raw ## Create all redistributable versions of the project

.PHONY: dist-archive
dist-archive: dist/$(NAME).tar.gz ## Build a redistributable archive of the project
dist/$(NAME).tar.gz: build $(ALL_ROOTFS)
	rm -rf $(CACHE_DIR)/$(NAME)
	mkdir -p $(CACHE_DIR)/$(NAME)
	$(MAKE) install PREFIX=$(CACHE_DIR)/$(NAME)/usr NO_RELOAD=true
	mkdir -p dist
	tar cvfz $@ -C $(CACHE_DIR) $(NAME)
	cd dist && sha256sum $(NAME).tar.gz > $(NAME).tar.gz.sha256.txt

.PHONY: dist-rpm
dist-rpm: dist/$(NAME)-$(VERSION)-1.$(ARCH).rpm ## Build a redistributable RPM package
dist/$(NAME)-$(VERSION)-1.$(ARCH).rpm: target/release/$(NAME)
	mkdir -p dist
	cargo install cargo-generate-rpm
	cargo generate-rpm
	cp ./target/generate-rpm/$(NAME)-$(VERSION)-1.$(ARCH).rpm dist
	cd dist && sha256sum $(NAME)-$(VERSION)-1.$(ARCH).rpm > $(NAME)-$(VERSION)-1.$(ARCH).rpm.sha256.txt

.PHONY: dist-ext
dist-ext: dist/$(NAME).raw ## Create a systemd-sysext extension archive
dist/$(NAME).raw: dist/$(NAME).tar.gz
	@echo "Building redistributable systemd extension"
	mkdir -p dist
	rm -rf dist/$(NAME).raw $(CACHE_DIR)/$(NAME).raw
	cp dist/$(NAME).tar.gz $(CACHE_DIR)
	cd $(CACHE_DIR) && tar xvfz $(NAME).tar.gz $(NAME)/usr
	mkdir -p $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d
	echo ID=$(SYSEXT_ID) > $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d/extension-release.$(NAME)
	echo VERSION_ID=$(SYSEXT_VERSION_ID) >> $(CACHE_DIR)/$(NAME)/usr/lib/extension-release.d/extension-release.$(NAME)

	@# Build the extension archive
	cd $(CACHE_DIR) && mksquashfs $(NAME) $(NAME).raw
	rm -rf $(CACHE_DIR)/$(NAME)
	mv $(CACHE_DIR)/$(NAME).raw $@
	cd dist && sha256sum $(NAME).raw > $(NAME).raw.sha256.txt

XSL_TEMPLATE := ./docs/dbus2markdown.xsl
.PHONY: docs
docs: ## Generate markdown docs for DBus interfaces
	mkdir -p docs
	xsltproc --novalid -o docs/manager.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Manager.xml
	sed -i 's/DBus Interface API/Manager DBus Interface API/g' ./docs/manager.md
	xsltproc --novalid -o docs/composite_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.CompositeDevice.xml
	sed -i 's/DBus Interface API/CompositeDevice DBus Interface API/g' ./docs/composite_device.md
	xsltproc --novalid -o docs/source_event_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Source.EventDevice.xml
	sed -i 's/DBus Interface API/Source EventDevice DBus Interface API/g' ./docs/source_event_device.md
	xsltproc --novalid -o docs/source_hidraw_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Source.HIDRawDevice.xml
	sed -i 's/DBus Interface API/Source HIDRaw DBus Interface API/g' ./docs/source_hidraw_device.md
	xsltproc --novalid -o docs/keyboard.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.Keyboard
	sed -i 's/DBus Interface API/Keyboard DBus Interface API/g' ./docs/keyboard.md
	xsltproc --novalid -o docs/target_dbus_device.md $(XSL_TEMPLATE) ./bindings/dbus-xml/org.shadowblip.Input.DBusDevice.xml
	sed -i 's/DBus Interface API/Target DBus Device Interface API/g' ./docs/target_dbus_device.md

# Refer to .releaserc.yaml for release configuration
.PHONY: sem-release 
sem-release: ## Publish a release with semantic release 
	npx semantic-release

# E.g. make in-docker TARGET=build
.PHONY: in-docker
in-docker:
	@# Run the given make target inside Docker
	docker build -t $(IMAGE_NAME):$(IMAGE_TAG) .
	docker run --rm \
		-v $(PWD):/src \
		--workdir /src \
		-e HOME=/home/build \
		--user $(shell id -u):$(shell id -g) \
		$(IMAGE_NAME):$(IMAGE_TAG) \
		make $(TARGET)

##@ Deployment

.PHONY: deploy
deploy: deploy-ext ## Build and deploy to a remote device

.PHONY: deploy-ext
deploy-ext: dist-ext ## Build and deploy systemd extension to a remote device
	ssh $(SSH_USER)@$(SSH_HOST) mkdir -p .var/lib/extensions
	scp dist/$(NAME).raw $(SSH_USER)@$(SSH_HOST):~/.var/lib/extensions
	ssh -t $(SSH_USER)@$(SSH_HOST) sudo systemd-sysext refresh
	ssh $(SSH_USER)@$(SSH_HOST) systemd-sysext status

