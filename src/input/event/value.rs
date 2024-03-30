use crate::{
    config::CapabilityConfig,
    input::capability::{Capability, Gamepad, Mouse},
};

/// Possible errors while doing input value translation
pub enum TranslationError {
    /// Translation not yet implemented
    NotImplemented,
    /// Impossible translation
    ImpossibleTranslation(String),
    /// Unable to translate value due to invalid or missing capability config
    /// in source config.
    InvalidSourceConfig(String),
    /// Unable to translate value due to invalid or missing capability config
    /// in target config.
    InvalidTargetConfig(String),
}

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
    ) -> Result<InputValue, TranslationError> {
        match source_cap {
            // None values cannot be translated
            Capability::None => Err(TranslationError::ImpossibleTranslation(
                "None events cannot be translated".to_string(),
            )),
            // NotImplemented values cannot be translated
            Capability::NotImplemented => Ok(InputValue::None),
            // Sync values can only be translated to '0'
            Capability::Sync => Ok(InputValue::Bool(false)),
            // DBus -> ...
            Capability::DBus(_) => Ok(self.clone()),
            // Gamepad -> ...
            Capability::Gamepad(gamepad) => {
                match gamepad {
                    // Gamepad Button -> ...
                    Gamepad::Button(_) => {
                        match target_cap {
                            // Gamepad Button -> None
                            Capability::None => Ok(InputValue::None),
                            // Gamepad Button -> NotImplemented
                            Capability::NotImplemented => Ok(InputValue::None),
                            // Gamepad Button -> Sync
                            Capability::Sync => Ok(InputValue::Bool(false)),
                            // Gamepad Button -> DBus
                            Capability::DBus(_) => Ok(self.clone()),
                            // Gamepad Button -> Gamepad
                            Capability::Gamepad(gamepad) => match gamepad {
                                // Gamepad Button -> Gamepad Button
                                Gamepad::Button(_) => Ok(self.clone()),
                                // Gamepad Button -> Axis
                                Gamepad::Axis(_) => self.translate_button_to_axis(target_config),
                                // Gamepad Button -> Trigger
                                Gamepad::Trigger(_) => Ok(self.translate_button_to_trigger()),
                                // Gamepad Button -> Accelerometer
                                Gamepad::Accelerometer => Err(TranslationError::NotImplemented),
                                // Gamepad Button -> Gyro
                                Gamepad::Gyro => Err(TranslationError::NotImplemented),
                            },
                            // Gamepad Button -> Mouse
                            Capability::Mouse(mouse) => match mouse {
                                // Gamepad Button -> Mouse Motion
                                Mouse::Motion => Err(TranslationError::NotImplemented),
                                // Gamepad Button -> Mouse Button
                                Mouse::Button(_) => Ok(self.clone()),
                            },
                            // Gamepad Button -> Keyboard
                            Capability::Keyboard(_) => Ok(self.clone()),
                        }
                    }
                    // Axis -> ...
                    Gamepad::Axis(_) => {
                        match target_cap {
                            // Axis -> None
                            Capability::None => Ok(InputValue::None),
                            // Axis -> NotImplemented
                            Capability::NotImplemented => Ok(InputValue::None),
                            // Axis -> Sync
                            Capability::Sync => Ok(InputValue::None),
                            // Axis -> DBus
                            Capability::DBus(_) => Ok(self.clone()),
                            // Axis -> Gamepad
                            Capability::Gamepad(gamepad) => match gamepad {
                                // Axis -> Button
                                Gamepad::Button(_) => self.translate_axis_to_button(source_config),
                                // Axis -> Axis
                                Gamepad::Axis(_) => Ok(self.clone()),
                                // Axis -> Trigger
                                Gamepad::Trigger(_) => Err(TranslationError::NotImplemented),
                                // Axis -> Accelerometer
                                Gamepad::Accelerometer => Err(TranslationError::NotImplemented),
                                // Axis -> Gyro
                                Gamepad::Gyro => Err(TranslationError::NotImplemented),
                            },
                            // Axis -> Mouse
                            Capability::Mouse(mouse) => match mouse {
                                Mouse::Motion => Err(TranslationError::NotImplemented),
                                Mouse::Button(_) => self.translate_axis_to_button(source_config),
                            },
                            // Axis -> Keyboard
                            Capability::Keyboard(_) => self.translate_axis_to_button(source_config),
                        }
                    }
                    // Trigger -> ...
                    Gamepad::Trigger(_) => Err(TranslationError::NotImplemented),
                    // Accelerometer -> ...
                    Gamepad::Accelerometer => Err(TranslationError::NotImplemented),
                    // Gyro -> ...
                    Gamepad::Gyro => Err(TranslationError::NotImplemented),
                }
            }
            // Mouse -> ...
            Capability::Mouse(_) => Err(TranslationError::NotImplemented),
            // Keyboard -> ...
            Capability::Keyboard(_) => Err(TranslationError::NotImplemented),
        }
    }

    /// Translate the button value into an axis value based on the given config
    fn translate_button_to_axis(
        &self,
        target_config: &CapabilityConfig,
    ) -> Result<InputValue, TranslationError> {
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
                        "left" => Ok(InputValue::Vector2 {
                            x: Some(-button_value),
                            y: None,
                        }),
                        // Right should be a positive value
                        "right" => Ok(InputValue::Vector2 {
                            x: Some(button_value),
                            y: None,
                        }),
                        // Up should be a negative value
                        "up" => Ok(InputValue::Vector2 {
                            x: None,
                            y: Some(-button_value),
                        }),
                        // Down should be a positive value
                        "down" => Ok(InputValue::Vector2 {
                            x: None,
                            y: Some(button_value),
                        }),
                        _ => Err(TranslationError::InvalidTargetConfig(format!(
                            "Invalid axis direction: {direction}"
                        ))),
                    }
                } else {
                    Err(TranslationError::InvalidTargetConfig(
                        "No axis direction defined to translate button to axis".to_string(),
                    ))
                }
            } else {
                Err(TranslationError::InvalidTargetConfig(
                    "No axis config to translate button to axis".to_string(),
                ))
            }
        } else {
            Err(TranslationError::InvalidTargetConfig(
                "No gamepad config to translate button to axis".to_string(),
            ))
        }
    }

    /// Translate the button value into trigger value based on the given config
    fn translate_button_to_trigger(&self) -> InputValue {
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
        InputValue::Float(button_value)
    }

    /// Translate the axis value into a button value based on the given config.
    fn translate_axis_to_button(
        &self,
        source_config: &CapabilityConfig,
    ) -> Result<InputValue, TranslationError> {
        if let Some(gamepad_config) = source_config.gamepad.as_ref() {
            if let Some(axis) = gamepad_config.axis.as_ref() {
                // Get the threshold to consider the axis as 'pressed' or not
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
                                    Ok(InputValue::Bool(true))
                                } else {
                                    Ok(InputValue::Bool(false))
                                }
                            } else {
                                Ok(InputValue::Bool(false))
                            }
                        }
                        // Right should be a positive value
                        "right" => {
                            if let Some(x) = x {
                                if x >= threshold {
                                    Ok(InputValue::Bool(true))
                                } else {
                                    Ok(InputValue::Bool(false))
                                }
                            } else {
                                Ok(InputValue::Bool(false))
                            }
                        }
                        // Up should be a negative value
                        "up" => {
                            if let Some(y) = y {
                                if y <= -threshold {
                                    Ok(InputValue::Bool(true))
                                } else {
                                    Ok(InputValue::Bool(false))
                                }
                            } else {
                                Ok(InputValue::Bool(false))
                            }
                        }
                        // Down should be a positive value
                        "down" => {
                            if let Some(y) = y {
                                if y >= threshold {
                                    Ok(InputValue::Bool(true))
                                } else {
                                    Ok(InputValue::Bool(false))
                                }
                            } else {
                                Ok(InputValue::Bool(false))
                            }
                        }
                        _ => Err(TranslationError::InvalidSourceConfig(format!(
                            "Invalid axis direction: {direction}"
                        ))),
                    }
                } else {
                    Err(TranslationError::InvalidSourceConfig(
                        "No axis direction defined to translate button to axis".to_string(),
                    ))
                }
            } else {
                Err(TranslationError::InvalidSourceConfig(
                    "No axis config to translate button to axis".to_string(),
                ))
            }
        } else {
            Err(TranslationError::InvalidSourceConfig(
                "No gamepad config to translate button to axis".to_string(),
            ))
        }
    }
}
