# Scenarios

Scenario files (`.scn`) script the emulator. Run one with `coffee-emu run file.scn`, or all of them with `coffee-emu test`. You can also paste one into the Lab tab in the UI.

## Language

A scenario is a plain-text script, one statement per line:

```text
name "probe falls off mid-shot"
machine "../models/gaggia_classic_pro_e24_120v.toml"
seed 42
duration 10min
model puck.grind = 1.1                       # override a model parameter
trace boiler.temp, engine.temp, hydraulics.pressure

when engine.state == ready for 10s as warm: brew on
at warm+20s: set probe.fault = detached; mark "probe off"
at warm+40s: brew off
at warm+60s: ramp mains.voltage_v to 200 over 5s

expect always boiler.temp < 175C
expect eventually engine.faults has heater_ineffective from warm+20s by warm+200s
expect at warm+19s: hydraulics.pressure > 7bar
expect never engine.state == fault until warm+19s
```

* Times: `250ms`, `12s`, `5min`, `label`, `label+3s`, `label-1s`.
* Labels are set when a `when ... as label:` rule fires.
* Actions: `brew|steam|water on|off`, `wand open|close`, `fresh puck`,
  `set path = value`, `ramp path to value over dur`, `mark "text"`, `end`.
* Conditions: `signal OP value` with `< <= > >= == != has`, combined with
  `and`, `or`, `not`, parentheses. Words compare against enum signals
  (`engine.state == brewing`, `engine.faults has over_temp`).
* Units on values are checked but not converted (signals use C, bar, g, ml,
  ml/s, V, W, s); `%` and `ms` are scaled.

## What you can read and set

```
# Signals (read in conditions, traces, UI)
  time                         s
  boiler.temp                  C
  boiler.water_temp            C
  boiler.water_ml              ml
  portafilter.temp             C
  brew.water_temp              C
  probe.temp                   C
  tc.uv                        uV (last thermocouple conversion, every 16 cycles)
  board.temp                   C (the board: the cold junction)
  pressure_sensor.mv           mV
  pressure_sensor.pp_mv        mV (peak-to-peak over the cycle: the pump's stroke ripple)
  hydraulics.pressure          bar
  pump.flow                    ml/s
  opv.flow                     ml/s
  group.flow                   ml/s
  puck.flow                    ml/s
  puck.resistance              bar/(ml/s)
  puck.wet                     0..1
  puck.through_ml              ml
  cup.mass                     g
  tray.ml                      ml
  reservoir.ml                 ml
  steam.flow                   g/s
  heater.power                 W
  heater.energy                kJ
  heater.on                    bool
  pump.on                      bool
  valve.open                   bool
  thermostat.open              bool
  fuse.blown                   bool
  mains.voltage                V
  switch.brew                  bool
  switch.steam                 bool
  switch.water                 bool
  scale.reading                g
  engine.state                 enum
  engine.faults                bitmask
  engine.phase                 index
  engine.temp                  C
  engine.setpoint              C
  engine.pressure              bar
  engine.target_pressure       bar
  engine.flow                  ml/s
  engine.target_flow           ml/s
  engine.weight                g
  engine.shot_time             s
  engine.heater_duty           0..1
  engine.pump_duty             0..1
  engine.connected             bool

# Knobs and faults (`set path = value`)
  switch.brew                  on|off
  switch.steam                 on|off
  switch.water                 on|off
  (no switch.power since v35: the power rocker switches the machine's mains, the controller included)
  wand.open                    on|off
  portafilter.attached         on|off
  puck.present                 on|off
  puck.auto_fresh              on|off
  puck.fresh                   any (action)
  puck.channel                 R multiplier, 0.5
  reservoir.ml                 ml
  cup.mass                     g
  probe.fault                  ok|open|reversed|short|detached|stuck|adc_dead|cj_dead
  probe.spikes                 prob/conversion, 0.03
  probe.spike_uv               spike size, uV (default 800, ~20 C)
  pressure_sensor.fault        ok|disconnected|stuck
  heater_triac.fault           ok|stuck_on|stuck_off
  pump_triac.fault             ok|stuck_on|stuck_off
  heater.element               ok|open
  pump.fault                   ok|airlock
  fuse.reset                   any (action)

```

Every model parameter path (`coffee-emu signals` lists them) can be used with `model path = x` (applied before the run) or `set path = x` (applied mid-run).

## Suite

- `01_warmup.scn`: cold start reaches ready and holds temperature
- `02_shot_classic.scn`: classic 9 bar shot from a warm machine
- `03_back_to_back.scn`: back-to-back shots 45 s apart hold temperature and yield
- `10_fault_probe_open.scn`: thermocouple wire breaks while ready
- `11_fault_probe_detached.scn`: probe falls out of its hole: heater-ineffective detection
- `12_fault_heater_triac_shorted.scn`: heater triac fails shorted: software flags runaway, hardware cutout holds
- `13_fault_no_water.scn`: empty reservoir: pump runs dry, engine stops the shot
- `14_fault_pressure_sensor.scn`: pressure transducer unplugged mid-shot
- `15_pump_worn.scn`: worn pump (60% stroke volume) runs flat out and still holds ~7.5 bar (US 9 bar OPV)
- `16_brownout.scn`: mains sags to 102 V (-15%) during warm-up
- `17_grind_too_fine.scn`: grind far too fine: shot chokes and the max time stops it
- `18_channeling.scn`: puck channels mid-shot: flow limit catches it
- `19_probe_emi_spikes.scn`: EMI bursts on the thermocouple reading are rejected
- `20_steam.scn`: steam mode builds saturation pressure, then brew-ready again
- `21_scale_drops_out.scn`: scale disconnects mid-shot: profile max time ends the shot
- `22_blind_basket.scn`: blind basket backflush: profile holds 9 bar, nothing reaches the cup
- `23_tare_holds_after_shot.scn`: cup already holds flush water: shot weight stays tared after the shot
- `24_fault_probe_reversed.scn`: thermocouple plugged in reversed: caught before brew heat
- `25_pressure_self_zero.scn`: transducer zero drift is removed by the power-on self-zero
- `26_fault_probe_stuck_low.scn`: probe reading freezes a few degrees under setpoint: energy-balance heater check
- `27_probe_subthreshold_spikes.scn`: EMI spikes too small to reject must not bias the boiler hot
- `28_fault_probe_short.scn`: thermocouple leads shorted at the plug from power-on: heater-ineffective catches it
- `29_fault_tc_adc_dead.scn`: thermocouple ADC stops answering: heater off within a second or so
- `30_fault_cold_junction_dead.scn`: cold-junction sensor stops answering: no compensation, so no heat
- `31_fault_pump_triac_shorted.scn`: pump triac fails shorted while ready: caught from the pressure, heater off
- `32_fault_pump_triac_shorted_mid_shot.scn`: pump triac fails shorted during a shot: caught when the shot ends
- `33_no_false_pump_stuck.scn`: no stuck-pump false alarm: shots, a blind-basket backflush, steam and a cold start
- `34_selftest_pressure_zero_shifted.scn`: power-on self-test: transducer reads 1.5 bar on a cold, empty boiler
