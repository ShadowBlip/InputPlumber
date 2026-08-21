use libloading::Library;
use std::error::Error;

#[allow(non_camel_case_types)]
pub type gboolean = std::ffi::c_int;
pub const GFALSE: std::ffi::c_int = 0;

#[allow(non_camel_case_types)]
pub type gpointer = *mut std::ffi::c_void;

pub type GObject = std::ffi::c_void;

/// Opaque data type representing a set of sources to be handled in a main loop.
/// https://docs.gtk.org/glib/struct.MainContext.html
pub type GMainContext = std::ffi::c_void;

/// Connection flags used to specify the behaviour of a signal’s connection.
/// https://docs.gtk.org/gobject/flags.ConnectFlags.html
pub type GConnectFlags = std::ffi::c_uint;

/// Allows operations to be cancelled.
/// https://docs.gtk.org/gio/class.Cancellable.html
pub type GCancellable = std::ffi::c_void;

/// Non-zero integer which uniquely identifies a particular string.
/// https://docs.gtk.org/glib/alias.Quark.html
pub type GQuark = std::ffi::c_uint;

/// Contains information about an error that has occurred.
/// https://docs.gtk.org/glib/struct.Error.html
#[repr(C)]
pub struct GError {
    pub domain: GQuark,
    pub code: i32,
    pub message: *mut std::ffi::c_char,
}

/// Represents a callback supplied by the programmer.
/// https://docs.gtk.org/gobject/struct.Closure.html
#[repr(C)]
pub struct GClosure {
    pub ref_count: u32,
    _truncated_record_marker: std::ffi::c_void,
}

/// The type used for the various notification callbacks which can be registered on closures.
/// https://docs.gtk.org/gobject/callback.ClosureNotify.html
pub type GClosureNotify = Option<unsafe extern "C" fn(gpointer, *mut GClosure)>;

/// The type used for callback functions in structure definitions and function signatures.
/// https://docs.gtk.org/gobject/callback.Callback.html
pub type GCallback = Option<unsafe extern "C" fn()>;

/* GLib functions */
/// g_main_context_iteration:
/// Runs a single iteration for the given main loop.
/// https://docs.gtk.org/glib/method.MainContext.iteration.html
pub type FnGMainContextIteration =
    unsafe extern "C" fn(context: *mut GMainContext, may_block: gboolean) -> gboolean;

/// g_quark_to_string:
/// Gets the string associated with the given GQuark.
/// https://docs.gtk.org/glib/func.quark_to_string.html
pub type FnGQuarkToString = unsafe extern "C" fn(quark: GQuark) -> *const std::ffi::c_char;

/* GObject functions */
/// g_object_unref:
/// Decreases the reference count of the provided object.
/// https://docs.gtk.org/gobject/method.Object.unref.html
pub type FnGObjectUnref = unsafe extern "C" fn(object: *mut GObject);

/// g_signal_connect_data:
/// Connects a GCallback function to a signal for a particular object.
/// This function cannot fail. If the given signal name doesn’t exist, a critical warning is emitted.
/// https://docs.gtk.org/gobject/func.signal_connect_data.html
pub type FnGSignalConnectData = unsafe extern "C" fn(
    instance: *mut GObject,
    // A string of the form “signal-name::detail”
    detailed_signal: *const std::ffi::c_char,
    c_handler: GCallback,

    // Data to pass to c_handler calls.
    data: gpointer,
    destroy_data: GClosureNotify,
    connect_flags: GConnectFlags,
);

/* Gio functions */
/// g_cancellable_new:
/// Creates a new GCancellable object.
/// Applications that want to start one or more operations that should be cancellable should create a GCancellable and pass it to the operations.
/// https://docs.gtk.org/gio/ctor.Cancellable.new.html
pub type FnGCancellableNew = unsafe extern "C" fn() -> *mut GCancellable;

/// g_cancellable_cancel:
/// Will set cancellable to cancelled, and will emit the GCancellable::cancelled signal. Thread safe.
/// https://docs.gtk.org/gio/method.Cancellable.cancel.html
pub type FnGCancellableCancel = unsafe extern "C" fn(cancellable: *mut GCancellable);

/* SSC functions */
/// ssc_sensor_*_new_sync:
/// This constructs a sensor object and returns a pointer to it.
/// As the signature is the same between ssc_sensor_gyroscope_new_sync & ssc_sensor_accelerometer_new_sync,
/// we reuse this for both.
pub type FnSscSensorNewSync = unsafe extern "C" fn(
    cancellable: *mut GCancellable,
    error: *mut *mut GError,
) -> *mut std::ffi::c_void;

/// ssc_sensor_*_open_sync:
/// Takes a sensor object and opens / starts communications with it.
/// This is a blocking operation.
pub type FnSscSensorOpenSync = unsafe extern "C" fn(
    sensor: *mut GObject,
    cancellable: *mut GCancellable,
    error: *mut *mut GError,
) -> u32;

/// Holds handles to the symbols in libssc and its dependencies.
/// This should stay thin (just bindings), keep the helper stuff elsewhere (like runtime.rs)
/// In the future this file could be replaced using bindgen's dylib support.
pub struct SscDylibs {
    _glib: Library,
    _gio: Library,
    _gobject: Library,
    _ssc: Library,

    /* GLib */
    pub g_main_context_iteration: FnGMainContextIteration,
    pub g_quark_to_string: FnGQuarkToString,

    /* Gio */
    pub g_cancellable_new: FnGCancellableNew,
    pub g_cancellable_cancel: FnGCancellableCancel,

    /* GObject */
    pub g_object_unref: FnGObjectUnref,
    pub g_signal_connect_data: FnGSignalConnectData,

    /* SSC */
    pub ssc_sensor_gyroscope_new_sync: FnSscSensorNewSync,
    pub ssc_sensor_gyroscope_open_sync: FnSscSensorOpenSync,
    pub ssc_sensor_accelerometer_new_sync: FnSscSensorNewSync,
    pub ssc_sensor_accelerometer_open_sync: FnSscSensorOpenSync,
}

/// Helper function for Library::new to return a nice error
#[macro_export]
macro_rules! load_library {
    ( $library_name:expr ) => {
        Library::new($library_name)
            .map_err(|e| format!("Library {} not found: {}", $library_name, e))
    };
}

/// Helper function for Library.get to return a nice error
#[macro_export]
macro_rules! load_symbol {
    ( $library:expr, $symbol_name:expr ) => {
        $library
            .get($symbol_name)
            .map_err(|e| format!("Symbol {} not found: {}", $symbol_name, e))
    };
}

impl SscDylibs {
    /// Load symbols dynamically from libssc and its dependencies
    pub unsafe fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
        // SAFETY: Library::new (from libloading) is "safe" to use as it returns an error, but dynamic library
        // loading can be unsafe in general. We handle possible errors here but we can't do much else.
        let glib = unsafe { load_library!("libglib-2.0.so.0")? };
        let gio = unsafe { load_library!("libgio-2.0.so.0")? };
        let gobject = unsafe { load_library!("libgobject-2.0.so.0")? };
        let ssc = unsafe { load_library!("libssc.so")? };

        // SAFETY: Handles possible errors when a symbol doesn't exist, so this is safe as long as our function &
        // type definitions are correct. The symbols should always be valid as we also keep references to the library handles.
        unsafe {
            Ok(Self {
                g_main_context_iteration: *load_symbol!(glib, "g_main_context_iteration")?,
                g_quark_to_string: *load_symbol!(glib, "g_quark_to_string")?,

                g_cancellable_new: *load_symbol!(gio, "g_cancellable_new")?,
                g_cancellable_cancel: *load_symbol!(gio, "g_cancellable_cancel")?,

                g_object_unref: *load_symbol!(gobject, "g_object_unref")?,
                g_signal_connect_data: *load_symbol!(gobject, "g_signal_connect_data")?,

                ssc_sensor_gyroscope_new_sync: *load_symbol!(ssc, "ssc_sensor_gyroscope_new_sync")?,
                ssc_sensor_gyroscope_open_sync: *load_symbol!(
                    ssc,
                    "ssc_sensor_gyroscope_open_sync"
                )?,
                ssc_sensor_accelerometer_new_sync: *load_symbol!(
                    ssc,
                    "ssc_sensor_accelerometer_new_sync"
                )?,
                ssc_sensor_accelerometer_open_sync: *load_symbol!(
                    ssc,
                    "ssc_sensor_accelerometer_open_sync"
                )?,

                _glib: glib,
                _gio: gio,
                _gobject: gobject,
                _ssc: ssc,
            })
        }
    }
}
