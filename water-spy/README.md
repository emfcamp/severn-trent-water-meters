# README

This crate implements a Class A LoRaWAN transponder for the SM150E/SM001E water meters.

This has been developed for the Heltec v3 but should take little modification to work
with other esp32s3 boards supplied with an sc1262 RF IC.

# Dependencies

To build you will need to install the esp rust toolchain using [`esp-up`](https://github.com/esp-rs/espup)

This includes installing the toolchain for esp32s3:

```shell
espup install -t esp32s3
```

# Building

Before building you will need to source the esp build environment for Rust into your shell

```shell
. $HOME/export-esp.sh
```

Then just build as normal using `cargo build`.

# Running

Note that the following commane will flash a compatibel device over USB and monitor `defmt` serial output use the following command:

```shell
cargo run --release
```


