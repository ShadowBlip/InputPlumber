use std::{collections::HashMap, str::FromStr};

use mlua::prelude::*;

use crate::{
    config::CompositeDeviceConfig,
    dmi::{get_cpu_info, get_dmi_data},
    input::{
        capability::Capability,
        event::{native::NativeEvent, value::InputValue},
    },
};

use super::{client::CompositeDeviceClient, InterceptMode};

/// List of lua functions to load from each discovered script
const LUA_FUNCTIONS: &[&str] = &["preprocess_event", "process_event", "postprocess_event"];

/// [ScriptEventAction] defines how an event should be processed further in the
/// input pipeline after executing a script function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEventAction {
    /// Continue processing the event through the pipeline
    Continue,
    /// Stop processing the event any further
    Stop,
}

/// CompositeDeviceLua is responsible for managing a Lua runtime to execute user
/// defined scripts during the CompositeDevice input pipeline.
#[derive(Debug)]
pub struct CompositeDeviceLua {
    /// Lua state instance
    lua: Lua,
    /// Lua event pipeline scripts
    lua_scripts: HashMap<&'static str, Vec<LuaFunction>>,
    /// Reference to the composite device
    composite_device: CompositeDeviceClient,
}

impl CompositeDeviceLua {
    /// Creates a new Lua instance
    pub fn new(client: CompositeDeviceClient, config: CompositeDeviceConfig) -> Self {
        // Initialize lua state
        let lua = Lua::new();
        let mut lua_scripts = HashMap::new();

        // Initialize globals in Lua
        CompositeDeviceLua::init_globals(&lua, &client, config);

        // Load the script(s) to execute during the event pipeline
        let scripts = ["./rootfs/usr/share/inputplumber/scripts/test.lua"];
        for path in scripts {
            let script_data = std::fs::read_to_string(path).unwrap();
            let chunk = lua.load(script_data);

            // Valid chunks should evaluate to returning a table
            let table = match chunk.eval::<LuaTable>() {
                Ok(table) => table,
                Err(e) => {
                    log::error!("Error loading script '{path}': {e:?}");
                    continue;
                }
            };

            // Load all functions from the evaluated table
            for func_name in LUA_FUNCTIONS.iter() {
                CompositeDeviceLua::load_function(&mut lua_scripts, &table, func_name, path);
            }
        }

        Self {
            lua,
            lua_scripts,
            composite_device: client,
        }
    }

    /// Initializes global scripting variables that will be available inside Lua
    /// on startup.
    fn init_globals(lua: &Lua, client: &CompositeDeviceClient, config: CompositeDeviceConfig) {
        // Global 'system'
        let system = lua.create_table().unwrap();

        // Load DMI data and expose it to Lua
        log::debug!("Loading DMI data");
        let dmi_data = lua.to_value(&get_dmi_data()).unwrap();
        system.set("dmi", dmi_data).unwrap();

        // Load CPU data and expose it to Lua
        log::debug!("Loading CPU info");
        let cpu_info = match get_cpu_info() {
            Ok(info) => info,
            Err(e) => {
                log::error!("Failed to get CPU info: {e:?}");
                panic!("Unable to determine CPU info!");
            }
        };
        let cpu_info = lua.to_value(&cpu_info).unwrap();
        system.set("cpu", cpu_info).unwrap();
        if let Err(e) = lua.globals().set("system", system) {
            log::error!("Failed to set cpu info: {e:?}");
        }

        // Global 'device'
        let device = lua.create_table().unwrap();

        // Populate event globals
        lua.globals()
            .set("_events_to_write", lua.create_table().unwrap())
            .unwrap();

        // Load the device config and expose it to Lua
        let config = lua.to_value(&config).unwrap();
        if let Err(e) = device.set("config", config) {
            log::error!("Failed to set device config: {e:?}");
        }
        if let Err(e) = device.set("intercept_mode", 0) {
            log::error!("Failed to set intercept mode: {e:?}");
        }

        // Bind the 'write_event' method to the 'device' global
        let write_event = lua
            .create_function(move |lua, event: LuaTable| {
                // The [CompositeDeviceClient] cannot be moved into a lua method, so
                // instead, write the event to a global so it can be sent later.
                let events_to_write = lua.globals().get::<LuaTable>("_events_to_write")?;
                events_to_write.push(event)?;

                Ok(())
            })
            .unwrap();
        if let Err(e) = device.set("write_event", write_event) {
            log::error!("Failed to set write_event method: {e:?}");
        }

        // Expose the 'device' global to Lua
        if let Err(e) = lua.globals().set("device", device) {
            log::error!("Failed to set cpu info: {e:?}");
        }
    }

    /// Load the function with the given name from the Lua table and update the
    /// hashmap.
    fn load_function(
        lua_scripts: &mut HashMap<&'static str, Vec<LuaFunction>>,
        table: &LuaTable,
        name: &'static str,
        path: &str,
    ) {
        // Extract the functions for each step in the pipeline
        let process_event = table.get::<LuaFunction>(name);
        match process_event {
            Ok(func) => {
                log::info!("Successfully loaded '{name}' from script '{path}'");
                lua_scripts
                    .entry(name)
                    .and_modify(|e: &mut Vec<LuaFunction>| e.push(func.clone()))
                    .or_insert_with(|| vec![func]);
            }
            Err(e) => match e {
                LuaError::FromLuaConversionError {
                    from,
                    to: _,
                    message: _,
                } => {
                    if from == "nil" {
                        log::trace!("Function not found in table");
                    }
                }
                _ => {
                    log::error!("Failed to load '{name}' function from '{path}': {e:?}");
                }
            },
        }
    }

    /// Expose the given intercept mode to lua
    pub fn set_intercept_mode(&self, mode: InterceptMode) {
        let device = match self.lua.globals().get::<LuaTable>("device") {
            Ok(dev) => dev,
            Err(e) => {
                log::error!("Failed to get 'device' global: {e:?}");
                return;
            }
        };
        let mode = match mode {
            InterceptMode::None => 0,
            InterceptMode::Pass => 1,
            InterceptMode::Always => 2,
            InterceptMode::GamepadOnly => 3,
        };
        if let Err(e) = device.set("intercept_mode", mode) {
            log::error!("Failed to set intercept mode on device: {e:?}");
        }
    }

    /// Expose the given source device paths in lua
    pub fn set_source_device_paths(&self, paths: Vec<String>) {
        let device = match self.lua.globals().get::<LuaTable>("device") {
            Ok(dev) => dev,
            Err(e) => {
                log::error!("Failed to get 'device' global: {e:?}");
                return;
            }
        };
        if let Err(e) = device.set("source_device_paths", paths) {
            log::error!("Failed to set source_device_paths on device: {e:?}");
        }
    }

    /// Executes the 'preprocess_event' function in any loaded Lua scripts.
    /// The preprocess_event function should be executed on all input events
    /// -before- capability map translation.
    pub fn preprocess_event(&self, event: &NativeEvent) -> ScriptEventAction {
        self.process_event_func("preprocess_event", event)
    }

    /// Executes the 'process_event' function in any loaded Lua scripts.
    /// The process_event function should be executed on all input events
    /// -after- capability map translation, but -before- input profile translation.
    pub fn process_event(&self, event: &NativeEvent) -> ScriptEventAction {
        self.process_event_func("process_event", event)
    }

    /// Executes the 'postprocess_event' function in any loaded Lua scripts.
    /// The postprocess_event function should be executed on all input events
    /// -after- capability map translation and -after- input profile translation.
    pub fn postprocess_event(&self, event: &NativeEvent) -> ScriptEventAction {
        self.process_event_func("postprocess_event", event)
    }

    /// Executes the event function with the given name from any loaded Lua scripts.
    fn process_event_func(&self, func_name: &str, event: &NativeEvent) -> ScriptEventAction {
        let Some(scripts) = self.lua_scripts.get(func_name) else {
            return ScriptEventAction::Continue;
        };
        if scripts.is_empty() {
            return ScriptEventAction::Continue;
        }

        // Convert the event into a lua table
        let Some(event_table) = self.event_to_table(event) else {
            log::error!("Unable to convert event");
            return ScriptEventAction::Continue;
        };

        // Clear any global data that is used to dispatch events
        if let Ok(events_to_write) = self.lua.globals().get::<LuaTable>("_events_to_write") {
            events_to_write.clear().unwrap();
        }

        // Execute the process_event method on all scripts
        let mut action = ScriptEventAction::Continue;
        for script in scripts {
            match script.call::<bool>(event_table.clone()) {
                Ok(true) => (),
                Ok(false) => {
                    action = ScriptEventAction::Stop;
                    break;
                }
                Err(e) => {
                    log::error!("Failed to execute {func_name}: {e:?}");
                    continue;
                }
            };
        }

        // Execute composite device commands based on global state
        if let Ok(events_to_write) = self.lua.globals().get::<LuaTable>("_events_to_write") {
            if events_to_write.is_empty() {
                return action;
            }

            // Write the events to the composite device
            for event in events_to_write.sequence_values::<LuaTable>() {
                let Ok(event) = event else {
                    continue;
                };

                // Convert the table to an event to emit
                let Some(native_event) = self.table_to_event(&event) else {
                    continue;
                };

                // Write the event to the composite device
                let device = self.composite_device.clone();
                tokio::task::spawn(async move {
                    if let Err(e) = device.write_event(native_event).await {
                        log::error!("Failed to write event: {e:?}");
                    }
                });
            }
        }

        action
    }

    /// Convert the given event to a Lua table
    fn event_to_table(&self, event: &NativeEvent) -> Option<LuaTable> {
        // Convert the event into a lua table
        let event_table = self.lua.create_table().ok()?;
        event_table
            .set("capability", event.as_capability().to_capability_string())
            .ok()?;
        match event.get_value() {
            InputValue::None => (),
            InputValue::Bool(value) => event_table.set("value", value).unwrap(),
            InputValue::Float(value) => event_table.set("value", value).unwrap(),
            InputValue::Vector2 { x, y } => {
                let value = self.lua.create_table().unwrap();
                if let Some(x) = x {
                    value.set("x", x).unwrap();
                }
                if let Some(y) = y {
                    value.set("y", y).unwrap();
                }
                event_table.set("value", value).unwrap();
            }
            InputValue::Vector3 { x, y, z } => {
                let value = self.lua.create_table().unwrap();
                if let Some(x) = x {
                    value.set("x", x).unwrap();
                }
                if let Some(y) = y {
                    value.set("y", y).unwrap();
                }
                if let Some(z) = z {
                    value.set("z", z).unwrap();
                }
                event_table.set("value", value).unwrap();
            }
            InputValue::Touch {
                index,
                is_touching,
                pressure,
                x,
                y,
            } => {
                //TODO
            }
        }

        Some(event_table)
    }

    /// Convert the given Lua table into a native event
    fn table_to_event(&self, table: &LuaTable) -> Option<NativeEvent> {
        let cap = table.get::<String>("capability").ok()?;
        let cap = Capability::from_str(cap.as_str()).ok()?;
        let value = table.get::<LuaValue>("value").ok()?;

        let value = match value.type_name() {
            "boolean" => {
                let value = value.as_boolean().unwrap();
                InputValue::Bool(value)
            }
            "number" => {
                let value = value.as_f64().unwrap();
                InputValue::Float(value)
            }
            "integer" => {
                let value = value.as_f64().unwrap();
                InputValue::Float(value)
            }
            // TODO:
            //"table" =>
            _ => return None,
        };

        let event = NativeEvent::new(cap, value);

        Some(event)
    }
}
