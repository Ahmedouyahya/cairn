# `make verify` reproduces every number quoted in the paper and the README.
CARGO ?= cargo
PY    ?= python3 -I
LAB   := target/release/cairn-lab

.PHONY: verify build lint test kat lab attacks ctgrind asm miri figures paper nodash clean video video-draft

verify: lint build test kat lab attacks ctgrind asm figures paper nodash
	@echo "== all checks passed"

build:
	$(CARGO) build --release --workspace

lint:
	$(CARGO) fmt --all --check
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --release --workspace

kat:
	$(PY) reference/cairn_ref.py
	$(PY) reference/gen_kat.py
	$(CARGO) test --release -p cairn-core --test kat

lab: build
	$(LAB) all

attacks: build
	$(PY) attacks/tompa_woll.py
	$(PY) attacks/mobile_adversary.py

ctgrind: build
	tools/ctgrind.sh

asm: build
	tools/asm_audit.sh

miri:
	PROPTEST_CASES=8 PROPTEST_DISABLE_FAILURE_PERSISTENCE=1 MIRIFLAGS=-Zmiri-disable-isolation \
	  $(CARGO) +nightly miri test -p cairn-core --lib

figures:
	uv run --with matplotlib --with numpy python paper/figures.py

paper: figures
	cd paper && latexmk -pdf -interaction=nonstopmode -halt-on-error cairn.tex

nodash:
	tools/nodash.sh

clean:
	$(CARGO) clean
	cd paper && latexmk -C

VIDEO_PY := uv run --python 3.12 --with numpy --with soundfile python

video: build
	python3 video/capture_demo.py
	$(VIDEO_PY) video/build.py
	python3 video/youtube.py

video-draft: build
	$(VIDEO_PY) video/build.py --draft
