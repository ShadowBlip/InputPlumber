# Persistent LED control (API version 1)

The existing org.shadowblip.Input.Source.LEDDevice interface retains Id. A source profile opts into managed control using config.led.settings_id. Only the AYANEO 3 joystick-ring source opts in initially, with identity ayaneo-3-joystick-rings. The existing controller/IMU mappings and other LED sources' game-output behaviour remain unchanged.

## Interface

- Capabilities (usasuu): API version, persistent identity, effect names, and minimum/maximum colour-cycle period. Both bounds are zero for firmware-fixed timing; clients must hide speed controls then.
- State (t(sayuu)ss): revision, complete configuration, application status and last error, in one atomic snapshot.
- Configuration (sayuu): effect, exactly three RGB bytes, brightness percent (0–100), remembered cycle period (2000–30000 ms).
- SetConfig((sayuu)) -> t: authorize the actual D-Bus caller, validate a complete configuration and persist it before scheduling hardware writes. Its reply confirms saved settings, not visible output. Observe State/PropertiesChanged for pending, applied, unavailable or failed.

The additional unconfigured status means no saved configuration exists. State.config then contains an editor placeholder, not a hardware readback or saved choice. Startup, reconnect, suspend, resume and stop perform no writes until an explicit SetConfig succeeds. Existing valid settings, including Off, are restored. Unreadable/corrupt files remain untouched and reported until the user explicitly applies a valid configuration.

Revision orders accepted changes within a service/device lifetime. Pin the unique service owner and rediscover after replacement. A timeout may follow a committed save: reread state, never automatically replay the request.

## Native AYANEO effects

Wire effects are off, solid, breathing and cycle (displayed as Rainbow). The current driver exposes effect_index=monocolor breathe rainbow. Solid clears effects with monocolor; Breathing selects breathe; Rainbow selects rainbow. Off writes zero brightness. Both rings share one configuration; firmware fixes effect speed. The saved cycle period remains valid but is ignored on this hardware.

Blank output before transitions, disable old triggers, clear native mode, configure colour/mode and apply final brightness. Hardware breathing does not require the pattern trigger or hw_pattern. The AYANEO hardware_cycle_only profile requires native Breathing/Rainbow and never falls back to software frames. Existing generic transport behaviour remains available to other profiles without changing their defaults.

One dedicated thread owns persistence and writes. Its bounded configuration queue cannot delay lifecycle/stop requests indefinitely; retiring a worker must finish before replacement. Native effects sleep between commands. Transport failures report failed and retain saved configuration, without a retry loop. Explicit Apply, reconnect, restart or resume can recover. Configured devices are blanked for suspend/orderly shutdown and restored afterward without replacing saved settings; a hard kill can leave the last firmware effect until recovery. Closing or uninstalling a frontend does not stop managed lighting.

## Storage and authorization

The systemd unit supplies StateDirectory=inputplumber, mode 0700. Version-1 JSON settings live in its leds subdirectory, named by the stable role. Writes synchronize the temporary file, atomically replace the saved file, and synchronize the directory before reporting durability. Pre-replacement failures preserve old settings; post-replacement synchronization errors expose the actual new configuration with a failed durability status and require rereading before retrying.

SetConfig uses its dedicated Polkit action. The proposal's root/admin and active-local authorization is retained, with inactive/remote callers excluded from the broader inputplumber-group rule. No frontend owns sysfs or state files. Bazzite must exclude the managed AYANEO rings from its generic writable RGB rule and enforce root-only write permissions on add/change. Other LED devices retain their existing behaviour.

## Validation

Run cargo test --locked and strict Clippy with dbus-daemon/busctl installed. Tests use fake device/storage/clock boundaries and a private bus/Polkit authority. Test native mode transitions without a pattern trigger, untouched first use and lifecycle, saved Off, persistence failures, caller authorization, uncertain requests, competing output and worker retirement. Physical output and effective distro permissions require attended device validation; official stock-image acceptance is separate.
