PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
SYSTEMD_USER_DIR ?= $(PREFIX)/lib/systemd/user
CARGO ?= cargo

.PHONY: all build release test clean install uninstall

all: build

build:
	$(CARGO) build

release:
	$(CARGO) build --release

test:
	$(CARGO) test

clean:
	$(CARGO) clean

install: release
	install -Dm755 target/release/rest-time-linux $(DESTDIR)$(BINDIR)/rest-time-linux
	install -Dm644 resources/rest-time.desktop $(DESTDIR)$(DATADIR)/applications/rest-time.desktop
	install -Dm644 resources/rest-time.service $(DESTDIR)$(SYSTEMD_USER_DIR)/rest-time.service
	install -Dm644 resources/icons/rest-time-active.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/rest-time-active.svg
	install -Dm644 resources/icons/rest-time-paused.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/rest-time-paused.svg
	install -Dm644 resources/icons/rest-time-break.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/rest-time-break.svg

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/rest-time-linux
	rm -f $(DESTDIR)$(DATADIR)/applications/rest-time.desktop
	rm -f $(DESTDIR)$(SYSTEMD_USER_DIR)/rest-time.service
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/rest-time-active.svg
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/rest-time-paused.svg
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/rest-time-break.svg
