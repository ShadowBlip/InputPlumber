# Persistent LED control (API version 1)

The existing `org.shadowblip.Input.Source.LEDDevice` interface retains `Id`.
A source profile opts into persistent control with `config.led.persistent_id`.
Only AYANEO 3 joystick rings opt in initially. Other LED sources retain their
existing game-output behavior. The profile key identifies the hardware role,
not a USB/HID enumeration number; do not rename it without migrating its state.

Read-only properties:

- `Capabilities (usasuu)`: API version, persistent identity, supported effects,
  minimum and maximum colour-cycle period in milliseconds. Both bounds are zero
  for a firmware-fixed cycle; hide the speed control in that case. The saved
  period remains a valid 2000–30000 value and is ignored by fixed-speed hardware,
  preserving the configuration format and remembered software-cycle preference.
- `State (t(sayuu)ss)`: revision, complete configuration, application status,
  and last hardware/restore error. This property is one atomic snapshot.
- Configuration `(sayuu)`: effect name, exactly three RGB bytes, brightness
  percent (0–100), and cycle period (2000–30000 milliseconds).

`SetConfig((sayuu)) -> t` authorizes the actual D-Bus caller with the dedicated
Polkit action and validates an entire configuration. A successful reply confirms
saved settings, not visible output. Observe `State` and `PropertiesChanged` for
`pending`, `applied`, `unavailable`, or `failed`. Revision orders accepted changes
within one service/device lifetime; reconnect and reread after owner changes.
An operation timeout has an uncertain outcome if persistence already started:
read state before retrying. Invalid requests never change saved settings.

Effects are `off`, `solid`, `breathing`, and `cycle`. Hardware breathing is
advertised only if the pattern trigger is available. Its tempo is fixed by
firmware. A colour cycle preserves the remembered solid colour. Off preserves
colour, brightness, and cycle speed. Defaults are Off, white, 30%, and 8 seconds.

A dedicated LED thread serializes persistence and hardware writes. At most eight
configuration commands can be queued, and there is no frame queue. Suspend and
resume use coalesced lifecycle state with revision acknowledgements outside that
queue, applied before configuration and frame writes.
The worker sleeps between commands for native cycling, Off, Solid and zero
brightness. Native cycling selects the driver's `effect=rainbow` once and leaves
animation to firmware. Other devices can use software frames at most every
100 ms, skipping missed frames and writing hue only. AYANEO 3 profiles set
`hardware_cycle_only: true`: if the driver lacks both `none` and `rainbow` in
`effect_index`, Cycle is unavailable instead of falling back to the observed
flickering path. Clearing a native effect precedes hardware breathing; Off blanks
the rings before clearing it. A transport failure stops animation and retains configuration.
Apply, reconnect, restart, or resume may retry; there is no failure retry loop.
Stopping takes priority over a saturated command queue. System sleep and orderly
shutdown request Off without overwriting saved settings. A hard kill or blocked
kernel I/O may leave the last hardware state until recovery.

The systemd unit provides `/var/lib/inputplumber`; state is atomically replaced
in its `leds` subdirectory. Files and their directory are synchronized before
reporting a durable save. A failure before replacement leaves prior settings
unchanged. If replacement succeeds but directory synchronization fails, SetConfig
returns an error while State exposes the actual new file/configuration and a
failed durability status. Animation stops until a successful Apply; clients must
reread State instead of assuming an error always means the old file survived.
No frontend has file or sysfs ownership. The distro
must restrict direct writes to the opted-in device's LED attributes so Steam's
independent sysfs path cannot override settings. The AYANEO driver may set rumble
strength to medium when writing LEDs; this is a kernel/firmware interaction.

## Tests

Run `cargo test --locked` with `dbus-daemon` and `busctl` installed. LED tests
start a private bus and fake Polkit/device/storage/clock boundaries; they do not
connect to the host bus or touch hardware. The fake-bus test also verifies actual
busctl discovery JSON. Physical output and distro permission enforcement require
separate device validation.
