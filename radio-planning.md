# Required features

* Sufficient Radio Range to span EMF Camp site (roughly 1km^2)
* No license band requirement
* Relatively hardware flexible
* Network stable (no signal jamming, routing loops, etc)
* Compatibility with upstream telemetry monitoring systems (Chirpstack?)
* Reasonable resistance to packet spoofing or abuse
* Reasonable resistance to physical proximity abuse (e.g. resistance to being trivially reprogrammed, resistance to leaking EUI/APP/etc trivially via serial port)

# Fun to have

Would be nice if the transponders could transmit periodic broadcast readings in the clear for EMF Camp guests to receive or if a Tildagon widget could track monitoring somehow.

# Hardware prototyping

* Starting with hobbyist ESP32-S3 devkit with Sx1262 RF IC.
* Initial prototyping for transponder [`embassy`](https://github.com/embassy-rs/embassy) (an embedded async runtime for Rust that replaces the need for an RTOS and supports multiple LoRa devkit boards), [`lora-rs`](https://github.com/lora-rs/lora-rs) (a LoRa modem package with LoRaWAN compatibility).
* Synchronous serial modulation on water meters is now well understood, but need to solve problem of demodulating this with only peripherals exposed by ESP32-S3 board.
  * This can be bitbashed by spamming 0xAAAAAAAA/0x555555555 over UART at double speed as a pseudo-clock signal, or potentially just recovered using a low speed SPI read if external SPI peripheral is free. One SPI bus is consumed already by the LoRa RF IC.

# Secure boot?

ESP32-S3 permits fusing secret key without this being legible from serial which may permit a primitive secure boot implementation and LoRaWAN key masking.

Not clear yet if this is required by EMF.
