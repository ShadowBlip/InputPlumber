use std::fmt::Display;

/// Output capabilities describe what kind of output events a source input device
/// is capable of handling. E.g. Force Feedback, LED control, etc.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OutputCapability {
    NotImplemented,
    ForceFeedback,
    ForceFeedbackUpload,
    ForceFeedbackErase,
    Haptics(Haptic),
    #[allow(clippy::upper_case_acronyms)]
    LED(LED),
}

impl Display for OutputCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            OutputCapability::NotImplemented => "NotImplemented".to_string(),
            OutputCapability::ForceFeedback => "ForceFeedback".to_string(),
            OutputCapability::ForceFeedbackUpload => "ForceFeedbackUpload".to_string(),
            OutputCapability::ForceFeedbackErase => "ForceFeedbackErase".to_string(),
            OutputCapability::Haptics(haptic) => format!("Haptics:{haptic}"),
            OutputCapability::LED(led) => format!("LED:{led}"),
        };

        write!(f, "{}", str)
    }
}

/// LED capability
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LED {
    Brightness,
    Color,
}

impl Display for LED {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            LED::Brightness => "Brightness",
            LED::Color => "Color",
        };

        write!(f, "{}", str)
    }
}

/// Haptic capabilities
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Haptic {
    TrackpadLeft,
    TrackpadRight,
    //TrackpadCenter,
}

impl Display for Haptic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Haptic::TrackpadLeft => "TrackpadLeft",
            Haptic::TrackpadRight => "TrackpadRight",
        };

        write!(f, "{}", str)
    }
}
