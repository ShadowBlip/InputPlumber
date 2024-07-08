use crate::{
    config::CapabilityConfig,
    input::capability::{Capability, Gamepad, Mouse, Touch, Touchpad},
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
    /// Bool values are typically used by button input.
    Bool(bool),
    /// Float values are typically used by trigger input.
    Float(f64),
    /// Vector2 values are typically used by axis input like joysticks.
    Vector2 {
        x: Option<f64>,
        y: Option<f64>,
    },
    /// Vector3 values are typically used by IMU sensors to detect position and
    /// velocity. The units of these values depend on the type of sensor the data
    /// is coming from.
    Vector3 {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
    },
    /// Touch values are normalized between (0.0, 0.0) and (1.0, 1.0) where (0, 0)
    /// is the top-left corner of the touch device. The touch index indicates
    /// the value for a particular finger.
    Touch {
        /// The finger id of the touch input for multi-touch devices.
        index: u8,
        /// Whether or not the device is sensing touch.
        is_touching: bool,
        /// Optionally the amount of pressure the touch is experiencing, normalized
        /// between 0.0 and 1.0.
        pressure: Option<f64>,
        /// The X position of the touch, normalized between 0.0-1.0, where 0
        /// is the left side of the input device and where 1.0 is the right side
        x: Option<f64>,
        /// The Y position of the touch, normalized between 0.0-1.0, where 0
        /// is the top side of the input device and where 1.0 is the bottom side
        y: Option<f64>,
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
            InputValue::Touch {
                index: _,
                is_touching: pressed,
                pressure: _,
                x: _,
                y: _,
            } => *pressed,
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
                            // Gamepad Button -> Touchpad
                            Capability::Touchpad(touch) => match touch {
                                Touchpad::LeftPad(_) => Err(TranslationError::NotImplemented),
                                Touchpad::RightPad(_) => Err(TranslationError::NotImplemented),
                                Touchpad::CenterPad(_) => Err(TranslationError::NotImplemented),
                            },
                            // Gamepad Button -> Touchscreen
                            Capability::Touchscreen(touch) => match touch {
                                // Gamepad Button -> Touchscreen Motion
                                Touch::Motion => Err(TranslationError::NotImplemented),
                                // Gamepad Button -> Touchscreen Button
                                Touch::Button(_) => Err(TranslationError::NotImplemented),
                            },
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
                                // Axis -> Mouse Motion
                                Mouse::Motion => self
                                    .translate_axis_to_mouse_motion(source_config, target_config),
                                // Axis -> Mouse Button
                                Mouse::Button(_) => self.translate_axis_to_button(source_config),
                            },
                            // Axis -> Keyboard
                            Capability::Keyboard(_) => self.translate_axis_to_button(source_config),
                            // Axis -> Touchpad
                            Capability::Touchpad(touch) => match touch {
                                Touchpad::LeftPad(_) => Err(TranslationError::NotImplemented),
                                Touchpad::RightPad(_) => Err(TranslationError::NotImplemented),
                                Touchpad::CenterPad(_) => Err(TranslationError::NotImplemented),
                            },
                            // Axis -> Touchscreen
                            Capability::Touchscreen(_) => Err(TranslationError::NotImplemented),
                        }
                    }
                    // Trigger -> ...
                    Gamepad::Trigger(_) => match target_cap {
                        // Trigger -> None
                        Capability::None => Ok(InputValue::None),
                        // Trigger -> NotImplemented
                        Capability::NotImplemented => Ok(InputValue::None),
                        // Trigger -> Sync
                        Capability::Sync => Ok(InputValue::None),
                        // Trigger -> DBus
                        Capability::DBus(_) => Ok(self.clone()),
                        // Trigger -> Gamepad
                        Capability::Gamepad(gamepad) => match gamepad {
                            // Trigger -> Button
                            Gamepad::Button(_) => self.translate_trigger_to_button(source_config),
                            // Trigger -> Axis
                            Gamepad::Axis(_) => Err(TranslationError::NotImplemented),
                            // Trigger -> Trigger
                            Gamepad::Trigger(_) => Ok(self.clone()),
                            // Trigger -> Accelerometer
                            Gamepad::Accelerometer => Err(TranslationError::NotImplemented),
                            // Trigger -> Gyro
                            Gamepad::Gyro => Err(TranslationError::NotImplemented),
                        },
                        // Trigger -> Mouse
                        Capability::Mouse(mouse) => match mouse {
                            // Trigger -> Mouse Motion
                            Mouse::Motion => Err(TranslationError::NotImplemented),
                            // Trigger -> Mouse Button
                            Mouse::Button(_) => self.translate_trigger_to_button(source_config),
                        },
                        // Trigger -> Keyboard
                        Capability::Keyboard(_) => self.translate_trigger_to_button(source_config),
                        // Trigger -> Touchpad
                        Capability::Touchpad(touch) => match touch {
                            Touchpad::LeftPad(_) => Err(TranslationError::NotImplemented),
                            Touchpad::RightPad(_) => Err(TranslationError::NotImplemented),
                            Touchpad::CenterPad(_) => Err(TranslationError::NotImplemented),
                        },
                        // Trigger -> Touchscreen
                        Capability::Touchscreen(_) => Err(TranslationError::NotImplemented),
                    },
                    // Accelerometer -> ...
                    Gamepad::Accelerometer => Err(TranslationError::NotImplemented),
                    // Gyro -> ...
                    Gamepad::Gyro => Err(TranslationError::NotImplemented),
                }
            }
            // Mouse -> ...
            Capability::Mouse(_) => Err(TranslationError::NotImplemented),
            // Keyboard -> ...
            Capability::Keyboard(_) => match target_cap {
                // Keyboard Key -> None
                Capability::None => Ok(InputValue::None),
                // Keyboard Key -> NotImplemented
                Capability::NotImplemented => Ok(InputValue::None),
                // Keyboard Key -> Sync
                Capability::Sync => Ok(InputValue::Bool(false)),
                // Keyboard Key -> DBus
                Capability::DBus(_) => Ok(self.clone()),
                // Keyboard Key -> Gamepad
                Capability::Gamepad(gamepad) => match gamepad {
                    Gamepad::Button(_) => Ok(self.clone()),
                    Gamepad::Axis(_) => Err(TranslationError::NotImplemented),
                    Gamepad::Trigger(_) => Err(TranslationError::NotImplemented),
                    Gamepad::Accelerometer => Err(TranslationError::NotImplemented),
                    Gamepad::Gyro => Err(TranslationError::NotImplemented),
                },
                // Keyboard Key -> Mouse
                Capability::Mouse(mouse) => match mouse {
                    Mouse::Motion => Err(TranslationError::NotImplemented),
                    Mouse::Button(_) => Ok(self.clone()),
                },
                // Keyboard Key -> Keyboard
                Capability::Keyboard(_) => Ok(self.clone()),
                // Keyboard Key -> Touchpad
                Capability::Touchpad(_) => Err(TranslationError::NotImplemented),
                // Keyboard Key -> Touchscreen
                Capability::Touchscreen(_) => Err(TranslationError::NotImplemented),
            },
            // Touchpad -> ...
            Capability::Touchpad(_) => Err(TranslationError::NotImplemented),
            // Touchscreen -> ...
            Capability::Touchscreen(touch) => match touch {
                // Touchscreen Motion -> ...
                Touch::Motion => match target_cap {
                    // Touchscreen Motion -> None
                    Capability::None => Ok(InputValue::None),
                    // Touchscreen Motion -> NotImplemented
                    Capability::NotImplemented => Ok(InputValue::None),
                    // Touchscreen Motion -> Sync
                    Capability::Sync => Ok(InputValue::Bool(false)),
                    // Touchscreen Motion -> DBus
                    Capability::DBus(_) => todo!(),
                    // Touchscreen Motion -> Gamepad ...
                    Capability::Gamepad(_) => Err(TranslationError::NotImplemented),
                    // Touchscreen Motion -> Mouse
                    Capability::Mouse(mouse) => match mouse {
                        // Touchscreen Motion -> Mouse Motion
                        Mouse::Motion => todo!(),
                        // Touchscreen Motion -> Mouse Button
                        Mouse::Button(_) => todo!(),
                    },
                    // Touchscreen Motion -> Keyboard
                    Capability::Keyboard(_) => Err(TranslationError::NotImplemented),
                    // Touchscreen Motion -> Touchpad
                    Capability::Touchpad(touchpad) => match touchpad {
                        Touchpad::LeftPad(target_touch) => match target_touch {
                            // Touchscreen Motion -> Touchpad Motion
                            Touch::Motion => Ok(self.clone()),
                            // Touchscreen Motion -> Touchpad Button
                            Touch::Button(_) => Err(TranslationError::NotImplemented),
                        },
                        Touchpad::RightPad(target_touch) => match target_touch {
                            // Touchscreen Motion -> Touchpad Motion
                            Touch::Motion => Ok(self.clone()),
                            // Touchscreen Motion -> Touchpad Button
                            Touch::Button(_) => Err(TranslationError::NotImplemented),
                        },
                        Touchpad::CenterPad(target_touch) => match target_touch {
                            // Touchscreen Motion -> Touchpad Motion
                            Touch::Motion => Ok(self.clone()),
                            // Touchscreen Motion -> Touchpad Button
                            Touch::Button(_) => Err(TranslationError::NotImplemented),
                        },
                    },
                    // Touchscreen Motion -> Touchscreen ...
                    Capability::Touchscreen(target_touch) => match target_touch {
                        // Touchscreen Motion -> Touchscreen Motion
                        Touch::Motion => Ok(self.clone()),
                        // Touchscreen Motion -> Touchscreen Button
                        Touch::Button(_) => Err(TranslationError::NotImplemented),
                    },
                },
                // Touchscreen Button -> ...
                Touch::Button(_) => Err(TranslationError::NotImplemented),
            },
        }
    }

    /// Translate the axis value into mouse motion
    fn translate_axis_to_mouse_motion(
        &self,
        _source_config: &CapabilityConfig,
        target_config: &CapabilityConfig,
    ) -> Result<InputValue, TranslationError> {
        // Use provided mapping to determine mouse motion value
        if let Some(mouse_config) = target_config.mouse.as_ref() {
            if let Some(mouse_motion) = mouse_config.motion.as_ref() {
                // Get the mouse speed in pixels-per-second
                let speed_pps = mouse_motion.speed_pps.unwrap_or(800);

                // Get the value from the axis event
                let (mut x, mut y) = match self {
                    InputValue::Vector2 { x, y } => (*x, *y),
                    InputValue::Vector3 { x, y, z: _ } => (*x, *y),
                    _ => (None, None),
                };

                // Check to see if the value is below a given threshold to prevent
                // mouse movements for axes that don't recenter to 0.
                if let Some(value) = x {
                    if value.abs() < 0.20 {
                        x = Some(0.0);
                    }
                }
                if let Some(value) = y {
                    if value.abs() < 0.20 {
                        y = Some(0.0);
                    }
                }

                // Multiply the value by the speed
                if let Some(value) = x {
                    x = Some(value * speed_pps as f64);
                }
                if let Some(value) = y {
                    y = Some(value * speed_pps as f64);
                }

                // If a direction is defined, map only the selected directions to
                // the value.
                if let Some(direction) = mouse_motion.direction.as_ref() {
                    // Create a vector2 value based on axis direction
                    match direction.as_str() {
                        // Horizontal takes both positive and negative
                        "horizontal" => Ok(InputValue::Vector2 { x, y: None }),
                        // Vertical takes both positive and negative
                        "vertical" => Ok(InputValue::Vector2 { x: None, y }),
                        // Left should be a negative value
                        "left" => {
                            if let Some(x) = x {
                                if x <= 0.0 {
                                    Ok(InputValue::Vector2 {
                                        x: Some(x),
                                        y: None,
                                    })
                                } else {
                                    Ok(InputValue::Vector2 { x: None, y: None })
                                }
                            } else {
                                Ok(InputValue::Vector2 { x: None, y: None })
                            }
                        }
                        // Right should be a positive value
                        "right" => {
                            if let Some(x) = x {
                                if x >= 0.0 {
                                    Ok(InputValue::Vector2 {
                                        x: Some(x),
                                        y: None,
                                    })
                                } else {
                                    Ok(InputValue::Vector2 { x: None, y: None })
                                }
                            } else {
                                Ok(InputValue::Vector2 { x: None, y: None })
                            }
                        }
                        // Up should be a negative value
                        "up" => {
                            if let Some(y) = y {
                                if y <= 0.0 {
                                    Ok(InputValue::Vector2 {
                                        x: None,
                                        y: Some(y),
                                    })
                                } else {
                                    Ok(InputValue::Vector2 { x: None, y: None })
                                }
                            } else {
                                Ok(InputValue::Vector2 { x: None, y: None })
                            }
                        }
                        // Down should be a positive value
                        "down" => {
                            if let Some(y) = y {
                                if y >= 0.0 {
                                    Ok(InputValue::Vector2 {
                                        x: None,
                                        y: Some(y),
                                    })
                                } else {
                                    Ok(InputValue::Vector2 { x: None, y: None })
                                }
                            } else {
                                Ok(InputValue::Vector2 { x: None, y: None })
                            }
                        }
                        _ => Err(TranslationError::InvalidTargetConfig(format!(
                            "Invalid axis direction: {direction}"
                        ))),
                    }
                }
                // If no direction is defined, map both axes to the mouse values
                else {
                    Ok(InputValue::Vector2 { x, y })
                }
            } else {
                Err(TranslationError::InvalidTargetConfig(
                    "No mouse motion config to translate axis to mouse motion".to_string(),
                ))
            }
            //
        } else {
            Err(TranslationError::InvalidTargetConfig(
                "No mouse config to translate axis to mouse motion".to_string(),
            ))
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

    /// Translate the trigger value into a button value based on the given config.
    fn translate_trigger_to_button(
        &self,
        source_config: &CapabilityConfig,
    ) -> Result<InputValue, TranslationError> {
        if let Some(gamepad_config) = source_config.gamepad.as_ref() {
            if let Some(trigger) = gamepad_config.trigger.as_ref() {
                // Get the threshold to consider the trigger as 'pressed' or not
                let threshold = trigger.deadzone.unwrap_or(0.3);

                // Get the trigger value
                let value = match self {
                    InputValue::Float(value) => *value,
                    _ => 0.0,
                };

                if value >= threshold {
                    Ok(InputValue::Bool(true))
                } else {
                    Ok(InputValue::Bool(false))
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
