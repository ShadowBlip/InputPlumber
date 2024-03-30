use crate::{
    config::CapabilityConfig,
    input::capability::{Capability, Gamepad, Mouse},
};

/// InputValue represents different ways to represent a value from an input event.
#[derive(Debug, Clone)]
pub enum InputValue {
    None,
    Bool(bool),
    Float(f64),
    Vector2 {
        x: Option<f64>,
        y: Option<f64>,
    },
    Vector3 {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
}

impl InputValue {
    /// Returns whether or not the value is "pressed"
    pub fn pressed(&self) -> bool {
        match self {
            InputValue::None => false,
            InputValue::Bool(value) => *value,
            InputValue::Float(value) => *value != 0.0,
            InputValue::Vector2 { x: _, y: _ } => true,
            InputValue::Vector3 { x: _, y: _, z: _ } => true,
        }
    }

    /// Translates the input value based on the source and target capabilities
    pub fn translate(
        &self,
        source_cap: &Capability,
        source_config: &CapabilityConfig,
        target_cap: &Capability,
        target_config: &CapabilityConfig,
    ) -> InputValue {
        match source_cap {
            // None values cannot be translated
            Capability::None => InputValue::None,
            // NotImplemented values cannot be translated
            Capability::NotImplemented => InputValue::None,
            // Sync values can only be translated to '0'
            Capability::Sync => InputValue::Bool(false),
            // Gamepad -> ...
            Capability::Gamepad(gamepad) => {
                match gamepad {
                    // Gamepad Button -> ...
                    Gamepad::Button(_) => match target_cap {
                        // Gamepad Button -> None
                        Capability::None => InputValue::None,
                        // Gamepad Button -> NotImplemented
                        Capability::NotImplemented => InputValue::None,
                        // Gamepad Button -> Sync
                        Capability::Sync => InputValue::Bool(false),
                        // Gamepad Button -> Gamepad
                        Capability::Gamepad(gamepad) => match gamepad {
                            // Gamepad Button -> Gamepad Button
                            Gamepad::Button(_) => self.clone(),
                            // Gamepad Button -> Axis
                            Gamepad::Axis(_) => {
                                // Use provided mapping to determine axis values
                                if let Some(gamepad_config) = target_config.gamepad.as_ref() {
                                    if let Some(axis) = gamepad_config.axis.as_ref() {
                                        if let Some(direction) = axis.direction.as_ref() {
                                            // Get the button value
                                            let button_value = match self {
                                                InputValue::Bool(v) => {
                                                    if *v {
                                                        1.0
                                                    } else {
                                                        0.0
                                                    }
                                                }
                                                InputValue::Float(v) => *v,
                                                _ => 0.0,
                                            };

                                            // Create a vector2 value based on axis direction
                                            match direction.as_str() {
                                                // Left should be a negative value
                                                "left" => InputValue::Vector2 {
                                                    x: Some(-button_value),
                                                    y: None,
                                                },
                                                // Right should be a positive value
                                                "right" => InputValue::Vector2 {
                                                    x: Some(button_value),
                                                    y: None,
                                                },
                                                // Up should be a negative value
                                                "up" => InputValue::Vector2 {
                                                    x: None,
                                                    y: Some(-button_value),
                                                },
                                                // Down should be a positive value
                                                "down" => InputValue::Vector2 {
                                                    x: None,
                                                    y: Some(button_value),
                                                },
                                                _ => {
                                                    log::warn!(
                                                        "Invalid axis direction: {direction}"
                                                    );
                                                    InputValue::None
                                                }
                                            }
                                        } else {
                                            log::warn!("No axis direction defined to translate button to axis");
                                            InputValue::None
                                        }
                                    } else {
                                        log::warn!("No axis config to translate button to axis");
                                        InputValue::None
                                    }
                                } else {
                                    log::warn!("No gamepad config to translate button to axis");
                                    InputValue::None
                                }
                            }
                            // Gamepad Button -> Trigger
                            Gamepad::Trigger(_) => todo!(),
                            // Gamepad Button -> Accelerometer
                            Gamepad::Accelerometer => todo!(),
                            // Gamepad Button -> Gyro
                            Gamepad::Gyro => todo!(),
                        },
                        // Gamepad Button -> Mouse
                        Capability::Mouse(mouse) => match mouse {
                            // Gamepad Button -> Mouse Motion
                            Mouse::Motion => todo!(),
                            // Gamepad Button -> Mouse Button
                            Mouse::Button(_) => self.clone(),
                        },
                        // Gamepad Button -> Keyboard
                        Capability::Keyboard(_) => self.clone(),
                    },
                    // Axis -> ...
                    Gamepad::Axis(_) => {
                        match target_cap {
                            // Axis -> None
                            Capability::None => InputValue::None,
                            // Axis -> NotImplemented
                            Capability::NotImplemented => InputValue::None,
                            // Axis -> Sync
                            Capability::Sync => InputValue::None,
                            // Axis -> Gamepad
                            Capability::Gamepad(gamepad) => match gamepad {
                                // Axis -> Button
                                Gamepad::Button(_) => {
                                    if let Some(gamepad_config) = source_config.gamepad.as_ref() {
                                        if let Some(axis) = gamepad_config.axis.as_ref() {
                                            let threshold = axis.deadzone.unwrap_or(0.3);
                                            if let Some(direction) = axis.direction.as_ref() {
                                                // TODO: Axis input is a special case where we need
                                                // to keep track of the state of the axis and only
                                                // emit events whenever the axis passes or falls
                                                // below the defined threshold

                                                // Get the axis value
                                                let (x, y) = match self {
                                                    InputValue::Vector2 { x, y } => (*x, *y),
                                                    InputValue::Vector3 { x, y, z: _ } => (*x, *y),
                                                    _ => (None, None),
                                                };

                                                match direction.as_str() {
                                                    // Left should be a negative value
                                                    "left" => {
                                                        if let Some(x) = x {
                                                            if x <= -threshold {
                                                                InputValue::Bool(true)
                                                            } else {
                                                                InputValue::Bool(false)
                                                            }
                                                        } else {
                                                            InputValue::Bool(false)
                                                        }
                                                    }
                                                    // Right should be a positive value
                                                    "right" => {
                                                        if let Some(x) = x {
                                                            if x >= threshold {
                                                                InputValue::Bool(true)
                                                            } else {
                                                                InputValue::Bool(false)
                                                            }
                                                        } else {
                                                            InputValue::Bool(false)
                                                        }
                                                    }
                                                    // Up should be a negative value
                                                    "up" => {
                                                        if let Some(y) = y {
                                                            if y <= -threshold {
                                                                InputValue::Bool(true)
                                                            } else {
                                                                InputValue::Bool(false)
                                                            }
                                                        } else {
                                                            InputValue::Bool(false)
                                                        }
                                                    }
                                                    // Down should be a positive value
                                                    "down" => {
                                                        if let Some(y) = y {
                                                            if y >= threshold {
                                                                InputValue::Bool(true)
                                                            } else {
                                                                InputValue::Bool(false)
                                                            }
                                                        } else {
                                                            InputValue::Bool(false)
                                                        }
                                                    }
                                                    _ => {
                                                        log::warn!(
                                                            "Invalid axis direction: {direction}"
                                                        );
                                                        InputValue::None
                                                    }
                                                }
                                            } else {
                                                log::warn!("No axis direction defined to translate axis to button");
                                                InputValue::None
                                            }
                                        } else {
                                            log::warn!(
                                                "No axis config to translate axis to button"
                                            );
                                            InputValue::None
                                        }
                                    } else {
                                        log::warn!("No gamepad config to translate axis to button");
                                        InputValue::None
                                    }
                                }
                                // Axis -> Axis
                                Gamepad::Axis(_) => self.clone(),
                                // Axis -> Trigger
                                Gamepad::Trigger(_) => todo!(),
                                // Axis -> Accelerometer
                                Gamepad::Accelerometer => todo!(),
                                // Axis -> Gyro
                                Gamepad::Gyro => todo!(),
                            },
                            Capability::Mouse(_) => todo!(),
                            Capability::Keyboard(_) => todo!(),
                        }
                    }
                    // Trigger -> ...
                    Gamepad::Trigger(_) => todo!(),
                    // Accelerometer -> ...
                    Gamepad::Accelerometer => todo!(),
                    // Gyro -> ...
                    Gamepad::Gyro => todo!(),
                }
            }
            // Mouse -> ...
            Capability::Mouse(_) => todo!(),
            // Keyboard -> ...
            Capability::Keyboard(_) => todo!(),
        }
    }
}
