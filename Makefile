.PHONY: build-host sign-host run-host

build-host:
	cargo build -p pear-host

sign-host: build-host
	codesign --force --sign - --entitlements pear-host/host.entitlements target/debug/pear-host

run-host: sign-host
	./target/debug/pear-host $(ARGS)