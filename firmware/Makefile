flash:
	cargo flash --chip STM32L412KBUx --release

build:
	cargo build --release

check:
	cargo clippy --release

debug:
	openocd -f interface/stlink-v2.cfg -f target/stm32l4x.cfg