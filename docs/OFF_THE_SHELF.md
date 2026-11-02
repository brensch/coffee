# Off-the-shelf alternatives to the custom board

Surveyed November 2026. GaggiMate's own controller is excluded on purpose. Prices are what the
linked pages showed at the time.

**Short answer.** Nothing off the shelf both covers the whole job and runs our Rust (esp-hal)
firmware.
- The closest complete boards are the Gaggiuino PCBs, and they're STM32.
- The tidiest modules-only build is an ESP32-S3 devkit plus a Krida 2-channel triac dimmer, with
  separate sensor modules. That works for **about $130–170** and is a good stopgap.
- The custom board (CIRCUIT.md) is still the better end state: one creepage-checked board instead
  of five modules on flying leads inside a hot, wet case.

| Option | What it covers | Price / availability | Fits our firmware? |
|---|---|---|---|
| [Gaggiuino PCB v4](https://diy-efi.co.uk/product/gaggiuino-4-0-pcb-stm32u585) / [v3.1](https://www.peakcoffee.cc/product/gaggiuino-pcb/) | Nearly everything: pump PSM + zero-cross, valve, thermocouple, ADC, switch inputs, scale header | v4 ~$68, v3.1 ~$48; complete kits $340–388 | **No.** STM32U585 / F411 BlackPill. Sold as an "R&D prototype", no certification |
| [Krida 2CH dimmer](https://www.tindie.com/products/bugrovs2012/2ch-ac-dimmer-module-controller-board-v20) | 2 isolated triacs + zero-cross SYNC output, 110–240 V | ~$23, in stock | **Yes**: 3 GPIOs (SYNC, two gates). Check it accepts 3.3 V logic |
| [RobotDyn AC dimmer](https://robotdyn.com/ac-light-dimmer-module-1-channel-3-3v-5v-logic-ac-50-60hz-220v-110v) | BTA16 triac + zero-cross, 3.3 V logic | 1CH $3–8; 2CH discontinued, clones only | Yes |
| [KinCony ADR](https://devices.esphome.io/devices/kincony-adr/) | ESP32 dimmer (ZC on GPIO25, gate on GPIO33) | price not found | Yes, partial pin map |
| [Waveshare ESP32-S3-Relay-6CH](https://www.waveshare.com/esp32-s3-relay-6ch.htm) | ESP32-S3 + 6 relays | ~$29 | Runs our firmware, but its **mechanical relays can't do half-cycle pump control**, and it needs 7–36 V DC in (no mains PSU) |
| [LilyGO T-Relay](https://www.lilygo.cc/en-cn/products/t-relay-5v-8-channel-relay) | ESP32 + mechanical relays | ~$16 | Poor fit, same relay problem |
| [Shelly Dimmer 2](https://esphome.io/components/light/shelly_dimmer.html) | Certified dimmer: ESP8266 plus an STM32 co-processor on the mains side | retail | No (and you lose the certification once you open it) |
| [Schematix Smart-Coffee](https://schematix.co.nz/community/smart_coffee/smart-coffee-esp32-version-programmable-espresso-machine-controller/) | Dual ESP32, mains PSU, SSRs, 2× PT100, pressure, load cell | ~$399 kit | ESP32, but no public pin map and no zero-cross |
| [Auber](https://idrinkcoffee.com/products/auber-instruments-pid-kit-for-gaggia-classic) / [Shades](https://www.shadesofcoffee.co.uk/shades-of-coffee/gaggia-classic-pro2019-pid-kit---132din-single-display) PID kits | Heater PID only | $150–240 | No (closed controller) |

## If you want to run before the board exists: the module build

| Part | ~$ |
|---|---|
| ESP32-S3-DevKitC-1 | 12 |
| Krida 2CH dimmer: pump + valve, and its SYNC pin is our zero-cross | 23 |
| Genuine zero-cross heater SSR on the chassis (Crydom D2425 / Omron G3NA). **Not** a Fotek ([UL counterfeit warning](https://www.fierceelectronics.com/components/ul-warns-solid-state-relay-counterfeit-ul-recognition-mark)) | 30–45 |
| MAX31855 breakout ([Adafruit 269](https://www.adafruit.com/products/269)) + ungrounded K probe | 25 |
| ADS1115 breakout ([Adafruit 1085](https://www.adafruit.com/product/1085)) + transducer | 35 |
| HLK-5M05 + fuse + MOV | 10 |
| Brew/steam switches: rewired as dry contacts (no module) | 0 |
| **Total** | **~$135–150** + plumbing kit |

The firmware drives the Krida gates and the SSR input exactly as it drives the board's triac
drivers: whole half-cycles, applied just before each zero crossing. Only the pin map changes
(`firmware/esp32/src/main.rs`).

## Why the custom board is still the end state
- **One enclosed, creepage-checked board.** No board in the table lists safety certification.
  Their mains wiring runs on flying leads in a 60–70 °C case that gets steam and drips.
- **Known parts on the mains side.** Genuine ST triacs and onsemi optos, instead of gambling on
  SSR sourcing.
- **Brew-thermostat-position heater switching.** The stock steam thermostat and thermal fuse stay
  in series as hardware cut-outs with zero new heater wiring (CIRCUIT.md).
- **Cost.** Under ~$60 a board at 5 units, assembled.
