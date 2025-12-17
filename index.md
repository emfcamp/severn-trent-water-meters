# Leak Counter 3000

Unofficial work in progress compatibility effort for the SM001E/SM150E Severn Trent Digital Water meters

# Pinouts

The SM001E and SM150E have two outgoing wire connections.

One is power (red +3V, Black Gnd)

The other is a 3 line synchronous digital IO protocol (likely similar to another discussed online as "ui1203"), carried in a beige insulator (photos to come).

These pins are as follows:

* **Black**: GND
* **Red**: Clock (3.0-5.0V)
* **Green**: I/O

The Clock signal powers the internal meter reading module as well as clocking the I/O data signal. This requires a square wave to be applied at under 5khz, with 2.4kbps working reasonably reliably.

The I/O port emits data on the rising edge in 7bit ascii with Even parity.

This I/O wire benefits from being read with a pullup resistor to a constant 3-5V source at 1-2kOhm.

# Data format

After about 120-160 clock cycles, the meter emits a reading of the following format:

```
V;RB070200;IB06050155\r
```

V appears to signal a meter reading response

**RBNNNNnn**:

RB - Reading Block
NNNN - cubic metres of water flowed
nn - decimals of cubic metres (aka 10s of litres)

The numbers after RB are the meter reading in 10 litre increments.

**IBYYSSSSSS**:

IB - Identifier Block
YY - Year of manufacture
SSSSSS - Serial number of water meter
