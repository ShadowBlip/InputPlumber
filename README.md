<h1 align="center">
  <img src="https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/icon.svg" alt="InputPlumber Logo" width="200">
  <br>
  InputPlumber
</h1>

<p align="center">
  <a href="https://github.com/ShadowBlip/InputPlumber/stargazers"><img src="https://img.shields.io/github/stars/ShadowBlip/InputPlumber" /></a>
  <a href="https://github.com/ShadowBlip/InputPlumber/blob/main/LICENSE"><img src="https://img.shields.io/github/license/ShadowBlip/InputPlumber" /></a>
  <a href="https://discord.gg/fKsUbrt"><img src="https://img.shields.io/badge/discord-server-%235865F2" /></a>
  <br>
</p>

## About

InputPlumber is an open source input routing and control daemon for Linux. It can
be used to combine any number of input devices (like gamepads, mice, and keyboards)
and translate their input to a variety of virtual device formats.

### Features

- [x] Combine multiple input devices
- [x] Handheld gamepad support (Steam Deck, ROG Ally, Legion Go, and more)
- [x] Emulate mouse, keyboard, and gamepad inputs
- [x] Intercept and route input over DBus for overlay interface control
- [x] Input mapping profiles to translate source input into the desired target input
- [x] Custom input routing via Lua scripting API
- [ ] Route input over the network

## Install

InputPlumber requires the following system dependencies:

- `libevdev`
- `libiio`

You can install with:

```bash
make build
sudo make install
```

If you are using ArchLinux, you can install InputPlumber from the AUR:

```bash
yay -S inputplumber-bin
```

Then start the service with:

```bash
sudo systemctl enable inputplumber
sudo systemctl enable inputplumber-suspend
sudo systemctl start inputplumber
```

## Documentation

XML specifications for all interfaces can be found in [bindings/dbus-xml](./bindings/dbus-xml).

## Usage

When InputPlumber is running as a service, you can interact with it over DBus.
There are various DBus libraries available for popular programming languages
like Python, Rust, C++, etc.

You can also interface with DBus using the `busctl` command:

```bash
busctl tree org.shadowblip.InputPlumber
```

### Input Profiles

InputPlumber is capable of loading input device profiles to translate inputs into
any other supported input event. Input profiles are defined as YAML configuration
files that can be loaded on-demand for any device that InputPlumber manages. The
format of an input profile config is defined by the [Device Profile Schema](rootfs/usr/share/inputplumber/schema/device_profile_v1.json) to make it easier to create profiles.

Typically input profiles should be generated using an external tool, but you 
can manually create your own profiles using any text editor. If you use an editor
that supports the [YAML Language Server](https://github.com/redhat-developer/yaml-language-server)
you can write profiles with auto-complete and usage information.

Here is a short example:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/inputplumber/schema/device_profile_v1.json
version: 1
kind: DeviceProfile
name: Start Button to Escape Key
description: Profile to map a gamepad's start button to the Escape keyboard key

mapping:
  - name: Menu
    source_event:
      gamepad:
        button: Start
    target_events:
      - keyboard: KeyEsc
```

This example will remap the `Start` button from a gamepad to the `ESC` key.

To load the input profile, you can use the `LoadProfilePath` method on the input
device you want the profile applied to. You can also do this from the command
line using `busctl`:

```bash
busctl call org.shadowblip.InputPlumber \
  /org/shadowblip/InputPlumber/CompositeDevice0 \
  org.shadowblip.Input.CompositeDevice \
  LoadProfilePath "s" /usr/share/inputplumber/profiles/mouse_keyboard_wasd.yaml
```

### Intercept Mode

Intercept Mode is a feature of InputPlumber that can allow external applications
to intercept input events from an input device and re-route them over DBus
instead. The primary use case for this feature is typically to allow overlay
applications (like [OpenGamepadUI](https://github.com/ShadowBlip/OpenGamepadUI))
to stop input from reaching other running applications (like a game),
allowing the overlay to process inputs without those inputs leaking into other
running apps.

You can set the intercept mode by setting the `InterceptMode` property on the
input device you want to intercept input from. The intercept mode can be one
of three values:

- `0` (NONE) - No inputs are intercepted and re-routed
- `1` (PASS) - No inputs are intercepted and re-routed *except* for gamepad `Guide` events. Upon receiving a gamepad `Guide` event, the device is automatically switched to intercept mode `2` (ALL).
- `2` (ALL) - All inputs are intercepted and re-routed over DBus
- `3` (GAMEPAD_ONLY) - All gamepad inputs are intercepted and re-routed over DBus

Typically the intercept mode should be handled by an external application, but
you can also set the intercept mode from the command line using `busctl`:

```bash
busctl set-property org.shadowblip.InputPlumber \
  /org/shadowblip/InputPlumber/CompositeDevice0 \
  org.shadowblip.Input.CompositeDevice \
  InterceptMode u 2
```

### Virtual Keyboard

When InputPlumber is running, a virtual keyboard is created that is used for
sending keyboard inputs. You can also use this keyboard to send keyboard events
using the DBus interface with the `SendKey` method. You can do this from the
command line using `busctl`:

```bash
busctl call org.shadowblip.InputPlumber \
  /org/shadowblip/InputPlumber/devices/target/keyboard0 \
  org.shadowblip.Input.Keyboard \
  SendKey sb KEY_ESC 1
```

### Device Compositing & Capability Maps

One feature of InputPlumber is the ability to combine multiple input devices
together into a single logical input device called a "Composite Device". This is
often required for many handheld gaming PCs that have built-in gamepads with
special non-standard buttons that show up as multiple independent input devices.

Composite devices are defined as YAML configuration files that follow the
[Composite Device Schema](./rootfs/usr/share/inputplumber/schema/composite_device_v1.json)
to combine the defined input devices together. When InputPlumber starts up, it
looks at all the input devices on the system and checks to see if they match a
composite device configuration. If they do, the input devices are combined into
a single logical composite device.

InputPlumber looks for composite device configuration files in the following
locations:

* `/etc/inputplumber/devices.d`
* `/usr/share/inputplumber/devices`

A composite device configuration looks like this:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/inputplumber/schema/composite_device_v1.json
version: 1
kind: CompositeDevice
name: OneXPlayer Intel

# Only check for source devices if *any* of the given data matches. If this list is
# empty, then the source devices will *always* be checked.
matches:
  - dmi_data:
      product_name: ONEXPLAYER
      sys_vendor: ONE-NETBOOK
      cpu_vendor: GenuineIntel
  - dmi_data:
      product_name: ONE XPLAYER
      sys_vendor: ONE-NETBOOK TECHNOLOGY CO., LTD.
      cpu_vendor: GenuineIntel

# One or more source devices to combine into a single virtual device. The events
# from these devices will be watched and translated according to the capability map.
source_devices:
  - group: gamepad
    evdev:
      name: OneXPlayer Gamepad
      phys_path: usb-0000:00:14.0-9/input0
  - group: keyboard
    evdev:
      name: AT Translated Set 2 keyboard
      phys_path: isa0060/serio0/input0

# The target input device(s) that will be created for this composite device
target_devices:
  - gamepad
  - mouse
  - keyboard

# The ID of a device capability mapping in the 'capability_maps' folder (optional)
capability_map_id: oxp1
```

In addition to combining multiple input devices together, composite devices can
also have a "Capability Map" to define the real capabilities of the input
device. This is commonly necessary for handheld gaming PCs where special
non-standard buttons will emit keyboard events (like `CTRL`+`ALT`+`DEL`) instead
of actual gamepad events.

Capability maps are defined in a separate YAML configuration file that follows
the [Capability Map Schema](./rootfs/usr/share/inputplumber/schema/capability_map_v1.json)
and are referenced by their unique ID.

InputPlumber loads capabilities maps from the following locations:

* `/etc/inputplumber/capability_maps.d`
* `/usr/share/inputplumber/capability_maps`

A capability map configuration looks like this:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/ShadowBlip/InputPlumber/main/rootfs/usr/share/inputplumber/schema/capability_map_v1.json
version: 1
kind: CapabilityMap
name: OneXPlayer Type 1
id: oxp1

# List of mapped events that are activated by a specific set of activation keys.
mapping:
  - name: Orange Button
    source_events:
      - keyboard: KeyLeftMeta
      - keyboard: KeyD
    target_event:
      gamepad:
        button: Guide
```

## Custom routing & scripting

InputPlumber supports custom input routing with a Lua scripting API. Input scripts
allow you to define custom input routing logic that can run at any step of the input
pipeline for any device managed by InputPlumber, giving you the power to route and
transform input events in almost any way you can imagine.

When InputPlumber starts managing a device, it will load input scripts from the
following locations:

* `/etc/inputplumber/scripts.d`
* `/usr/share/inputplumber/scripts`

Each input script should return a table with function(s) defined to run at
particular stages of the input pipeline. As input events flow from physical
source device(s) to virtual target device(s), the composite device will execute
the Lua function in each script that matches the pipeline stage.

These are the stages of the input pipeline in order of execution:

* `preprocess_event` - executed on each event _before_ capability mapping **and** _before_ input profile translation
* `process_event` - executed on each event _after_ capability mapping **but** _before_ input profile translation
* `postprocess_event` - executed on each event _after_ capability mapping **and** _after_ input profile translation

```
 Source Device(s)
       │
    (event)
       │
       └── Composite Device
                  │
          (preprocess_event)
                  │
           (process_event)
                  │
         (postprocess_event)
                  │
                  └── Target Device(s)
                            │
                         (event)
```

For example, this script will print a message to the log whenever the start button is pressed
and processed during the `process_event` stage:

```lua
-- process_event is called on every input event -after- capability mapping
-- but -before- input profile translation.
local process_event = function(event)
  if event.capability == "Gamepad:Button:Start" then
    print("Start button was pressed! Value:", event.value)
  end

  -- Returning 'true' allows the event to be processed further by the input
  -- pipeline.
  return true
end

return {
  process_event = process_event,
}
```

### Global Variables

Several global variables are available that can be used in your script that
expose information about the input device and methods for emitting input events.

* `system` - contains system and cpu information
* `device` - composite device properties and methods

### Examples

#### Transform guide button events (XBox button, PS button, etc.) into start button events

```lua
-- process_event is called on every input event -after- capability mapping
-- but -before- input profile translation.
local process_event = function(event)
  -- If this is not a guide button event, continue processing the event like
  -- normal by returning 'true'.
  if event.capability ~= "Gamepad:Button:Guide" then
    return true
  end

  -- Events can be emitted using the 'write_event' method on the 'device' global
  local new_event = {
    capability = "Gamepad:Button:Start",
    value = event.value,
  }
  device.write_event(new_event)

  -- Returning 'false' stops further processing of this event
  return false
end

return {
  process_event = process_event,
}
```

#### Print every input event only if the device is a Sony DualSense controller

```lua
-- If the composite device configuration is not a Sony DualSense controller,
-- return a table with 'enabled' set to false so no events are procesed by
-- this script.
if device.config.name ~= "Sony Interactive Entertainment DualSense Wireless Controller" then
  return {
    enabled = false,
  }
end

-- preprocess_event is called on every input event -before- capability mapping
-- and input profile translation.
local preprocess_event = function(event)
  print("Event: ", event.capability, event.value)
  return true
end

return {
  preprocess_event = preprocess_event,
}
```

## License

InputPlumber is licensed under THE GNU GPLv3+. See LICENSE for details.
