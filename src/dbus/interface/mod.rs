pub mod composite_device;
pub mod force_feedback;
pub mod manager;
pub mod performance;
pub mod source;
pub mod target;

/// Used to return the name of a DBus interface
pub trait NamedInterface {
    /// Name of the DBus interface (e.g. "org.shadowblip.Input.CompositeDevice")
    fn interface_name() -> &'static str;
}
