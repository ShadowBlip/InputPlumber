-- Input scripts are evaluated when InputPlumber starts managing an input device
-- and should return a table which includes functions that can be called during
-- each step of the input pipeline.
--
-- The input pipeline looks like this:
--
-- Source Device(s)
--       │
--    (event)
--       │
--       └── Composite Device
--                  │
--          (preprocess_event)
--                  │
--           (process_event)
--                  │
--          (postprocess_event)
--                  │
--                  └── Target Device(s)

print("Loaded example script")

-- Several global variables are available with system information, composite
-- device configuration, and methods for sending events.
--
-- Globals:
--   system - contains system and cpu information
--   device - composite device properties and methods
print("--- System DMI Data ---")
for key, value in pairs(system.dmi) do
	print(key, value)
end

print("--- System CPU ---")
for key, value in pairs(system.cpu) do
	print(key, value)
end

print("--- Device Config ---")
for key, value in pairs(device.config) do
	print(key, value)
end

-- Scripts can disable themselves under certain conditions by returning a
-- table with 'enabled' set to false.
if system.dmi.product_family == "Desktop" then
	return {
		enabled = false,
	}
end

-- The composite device configuration can also be accessed
if device.config.name ~= "Sony Interactive Entertainment DualSense Wireless Controller" then
	return {
		enabled = false,
	}
end

-- preprocess_event is called on every input event -before- capability translation
-- and input profile translation.
local preprocess_event = function(event)
	if event.capability == "Gamepad:Button:Guide" then
		print("[preprocess] Got guide button: ", event.value)
	end

	-- Returning 'true' allows the event to be processed further by the input
	-- pipeline.
	return true
end

-- process_event is called on every input event -after- capability translation
-- but -before- input profile translation.
local process_event = function(event)
	if event.capability == "Gamepad:Button:Guide" then
		print("[process] Got guide button: ", event.value)

		-- Events can be emitted using the 'write_event' method on the 'device' global
		local new_event = {
			capability = "Gamepad:Button:South",
			value = event.value,
		}
		device.write_event(new_event)

		-- Returning 'false' stops further processing of this event
		return false
	end

	return true
end

-- postprocess_event is called on every input event -after- capability translation
-- and -after- input profile translation.
local postprocess_event = function(event)
	if event.capability == "Gamepad:Button:Guide" then
		print("[postprocess] Got guide button: ", event.value)
	end

	return true
end

return {
	enabled = true,
	preprocess_event = preprocess_event,
	process_event = process_event,
	postprocess_event = postprocess_event,
}
