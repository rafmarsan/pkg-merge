IMAGE := pkg-merge-dev
CARGO_VOLUME := pkg-merge-cargo
TARGET_VOLUME := pkg-merge-target

HOST_ARCH := $(shell uname -m)
ifeq ($(filter $(HOST_ARCH),arm64 aarch64),$(HOST_ARCH))
LINUX_ARCH := arm64
else
LINUX_ARCH := x86_64
endif

DOCKER_RUN := docker run --rm \
	-v "$(CURDIR)":/work \
	-v $(CARGO_VOLUME):/usr/local/cargo/registry \
	-v $(TARGET_VOLUME):/target \
	-e CARGO_TARGET_DIR=/target \
	-w /work $(IMAGE)

.PHONY: help image test fmt clippy check release-check \
	linux-binary macos-binary clean-volumes clean

help:
	@echo "pkg-merge - comandos disponibles"
	@echo ""
	@echo "  make help            Muestra esta ayuda"
	@echo "  make image           Construye la imagen Docker de desarrollo"
	@echo "  make test            Ejecuta las pruebas dentro de Docker"
	@echo "  make check           Compila sin ejecutar (Docker)"
	@echo "  make fmt             Comprueba el formato (Docker)"
	@echo "  make clippy          Ejecuta el linter (Docker)"
	@echo "  make release-check   Compila en modo release (Docker)"
	@echo ""
	@echo "  make linux-binary    Crea dist/pkg-merge-linux-<arch> usando Docker"
	@echo "  make macos-binary    Crea dist/pkg-merge-macos-<arch> usando Rust nativo"
	@echo ""
	@echo "  make clean-volumes   Borra las caches de Docker"
	@echo "  make clean           Limpia los artefactos dentro de Docker"
	@echo ""
	@echo "Nota: Docker en macOS compila un binario Linux, no macOS."

image:
	docker build -f .devcontainer/Dockerfile -t $(IMAGE) .

test: image
	$(DOCKER_RUN) cargo test --all

fmt: image
	$(DOCKER_RUN) cargo fmt --all -- --check

clippy: image
	$(DOCKER_RUN) cargo clippy --all-targets -- -D warnings

check: image
	$(DOCKER_RUN) cargo check --all-targets

release-check: image
	$(DOCKER_RUN) cargo build --release --locked

# Compila dentro del contenedor Linux y copia el binario al host.
linux-binary: image
	mkdir -p dist
	$(DOCKER_RUN) cargo build --release --locked
	docker run --rm \
		-v "$(CURDIR)":/work \
		-v $(TARGET_VOLUME):/target \
		$(IMAGE) cp /target/release/pkg-merge /work/dist/pkg-merge-linux-$(LINUX_ARCH)
	@echo "Listo: dist/pkg-merge-linux-$(LINUX_ARCH)"

# Requiere Rust instalado en el Mac (rustup) y Xcode Command Line Tools.
macos-binary:
	@command -v cargo >/dev/null 2>&1 || { \
		echo "Rust no esta instalado. Instala rustup desde https://rustup.rs"; \
		exit 1; \
	}
	mkdir -p dist
	cargo build --release --locked
	cp target/release/pkg-merge "dist/pkg-merge-macos-$$(uname -m)"
	@echo "Listo: dist/pkg-merge-macos-$$(uname -m)"

clean-volumes:
	docker volume rm $(CARGO_VOLUME) $(TARGET_VOLUME)

clean:
	$(DOCKER_RUN) cargo clean
