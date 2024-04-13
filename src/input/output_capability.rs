/// Output capabilities describe what kind of output events a source input device
/// is capable of handling. E.g. Force Feedback, LED control, etc.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OutputCapability {
    NotImplemented,
    ForceFeedback,
    ForceFeedbackUpload,
    ForceFeedbackErase,
    #[allow(clippy::upper_case_acronyms)]
    LED(LED),
}

/// LED capability
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LED {
    Brightness,
    Color,
}
