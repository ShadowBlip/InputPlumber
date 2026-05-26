pub mod glib {
    use libloading::Library;
    use std::error::Error;

    /* Main / basic types */
    pub type GObject = std::ffi::c_void;
    pub type GMainContext = std::ffi::c_void;
    pub type GQuark = std::ffi::c_uint;
    pub type GConnectFlags = std::ffi::c_uint;

    #[allow(non_camel_case_types)]
    pub type gboolean = std::ffi::c_int;
    pub const GFALSE: std::ffi::c_int = 0;
    // pub const GTRUE: std::ffi::c_int = 1;

    #[allow(non_camel_case_types)]
    pub type gpointer = *mut std::ffi::c_void;

    #[repr(C)]
    pub struct GError {
        pub domain: GQuark,
        pub code: i32,
        pub message: *mut std::ffi::c_char,
    }

    /* Closure stuff */
    #[repr(C)]
    pub struct GClosure {
        pub ref_count: u32,
        _truncated_record_marker: std::ffi::c_void,
    }
    pub type GClosureNotify = Option<unsafe extern "C" fn(gpointer, *mut GClosure)>;
    pub type GCallback = Option<unsafe extern "C" fn()>;

    /* Function ptrs */
    /// g_main_context_iteration
    pub type FnGMainContextIteration =
        unsafe extern "C" fn(context: *mut GMainContext, may_block: gboolean) -> gboolean;

    /// g_quark_to_string
    pub type FnGQuarkToString = unsafe extern "C" fn(quark: GQuark) -> *const std::ffi::c_char;

    pub struct GlibDylib {
        _library: Library,
        pub g_main_context_iteration: FnGMainContextIteration,
        pub g_quark_to_string: FnGQuarkToString,
    }

    impl GlibDylib {
        /// Load libglib-2.0.so and methods
        pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
            let library = unsafe {
                Library::new("libglib-2.0.so.0")
                    .map_err(|e| format!("libglib-2.0 not found {e}"))?
            };

            Ok(Self {
                g_main_context_iteration: unsafe {
                    *library.get(b"g_main_context_iteration\0").map_err(|e| {
                        format!("libglib-2.0:g_main_context_iteration not found {e}")
                    })?
                },

                g_quark_to_string: unsafe {
                    *library
                        .get(b"g_quark_to_string\0")
                        .map_err(|e| format!("libglib-2.0:g_quark_to_string not found {e}"))?
                },

                _library: library,
            })
        }

        /// Converts a GLib error ptr to a basic string (or None if the error is null)
        pub fn convert_error(&self, error: *mut GError) -> Option<String> {
            if error.is_null() {
                None
            } else {
                let domain_str = unsafe {
                    let domain_cstr =
                        std::ffi::CStr::from_ptr((self.g_quark_to_string)((*error).domain));
                    domain_cstr.to_string_lossy().into_owned()
                };

                let domain = unsafe { (*error).domain };
                let code = unsafe { (*error).code };

                Some(format!(
                    "GLib error: domain = {} / {}, code = {}",
                    domain, domain_str, code
                ))
            }
        }
    }
}

pub mod gio {
    use libloading::Library;
    use std::error::Error;

    pub type GCancellable = std::ffi::c_void;

    /* Function ptrs */
    /// g_cancellable_new
    pub type FnGCancellableNew = unsafe extern "C" fn() -> *mut GCancellable;

    /// g_cancellable_cancel
    pub type FnGCancellableCancel = unsafe extern "C" fn(cancellable: *mut GCancellable);

    pub struct GioDylib {
        _library: Library,
        pub g_cancellable_new: FnGCancellableNew,
        pub g_cancellable_cancel: FnGCancellableCancel,
    }

    impl GioDylib {
        /// Load libgio-2.0.so and methods
        pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
            let library = unsafe {
                Library::new("libgio-2.0.so.0").map_err(|e| format!("libgio-2.0 not found {e}"))?
            };

            Ok(Self {
                g_cancellable_new: unsafe {
                    *library
                        .get(b"g_cancellable_new\0")
                        .map_err(|e| format!("libgio-2.0:g_cancellable_new not found {e}"))?
                },
                g_cancellable_cancel: unsafe {
                    *library
                        .get(b"g_cancellable_cancel\0")
                        .map_err(|e| format!("libgio-2.0:g_cancellable_cancel not found {e}"))?
                },
                _library: library,
            })
        }
    }
}

pub mod gobject {
    use libloading::Library;
    use std::error::Error;

    use super::glib::{gpointer, GCallback, GClosureNotify, GConnectFlags, GObject};

    /* Function ptrs */
    /// g_object_unref
    pub type FnGObjectUnref = unsafe extern "C" fn(object: *mut GObject);

    /// g_signal_connect_data
    pub type FnGSignalConnectData = unsafe extern "C" fn(
        instance: *mut GObject,
        detailed_signal: *const std::ffi::c_char,
        c_handler: GCallback,
        data: gpointer,
        destroy_data: GClosureNotify,
        connect_flags: GConnectFlags,
    );

    pub struct GObjectDylib {
        _library: Library,
        pub g_object_unref: FnGObjectUnref,
        pub g_signal_connect_data: FnGSignalConnectData,
    }

    impl GObjectDylib {
        /// Load libgobject-2.0.so and methods
        pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
            let library = unsafe {
                Library::new("libgobject-2.0.so.0")
                    .map_err(|e| format!("libgobject-2.0 not found {e}"))?
            };

            Ok(Self {
                g_object_unref: unsafe {
                    *library
                        .get(b"g_object_unref\0")
                        .map_err(|e| format!("libgobject-2.0:g_object_unref not found {e}"))?
                },
                g_signal_connect_data: unsafe {
                    *library.get(b"g_signal_connect_data\0").map_err(|e| {
                        format!("libgobject-2.0:g_signal_connect_data not found {e}")
                    })?
                },
                _library: library,
            })
        }
    }
}

pub mod ssc {
    use super::gio::GCancellable;
    use super::glib::{gpointer, GClosure, GError, GObject};
    use libloading::Library;
    use std::error::Error;

    /* Function ptrs */
    /// ssc_sensor_*_new_sync
    pub type FnSscSensorNewSync = unsafe extern "C" fn(
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
    ) -> *mut std::ffi::c_void;

    /// ssc_sensor_*_open_sync
    pub type FnSscSensorOpenSync = unsafe extern "C" fn(
        sensor: *mut GObject,
        cancellable: *mut GCancellable,
        error: *mut *mut GError,
    ) -> u32;

    pub struct SscDylib {
        _library: Library,
        pub ssc_sensor_gyroscope_new_sync: FnSscSensorNewSync,
        pub ssc_sensor_gyroscope_open_sync: FnSscSensorOpenSync,
        pub ssc_sensor_accelerometer_new_sync: FnSscSensorNewSync,
        pub ssc_sensor_accelerometer_open_sync: FnSscSensorOpenSync,
    }

    impl SscDylib {
        /// Load libssc.so and methods
        /// This should be used through SscRuntime
        pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
            let library =
                unsafe { Library::new("libssc.so").map_err(|e| format!("libssc not found {e}"))? };

            Ok(Self {
                ssc_sensor_gyroscope_new_sync: unsafe {
                    *library
                        .get(b"ssc_sensor_gyroscope_new_sync\0")
                        .map_err(|e| {
                            format!("libssc:ssc_sensor_gyroscope_new_sync not found {e}")
                        })?
                },
                ssc_sensor_gyroscope_open_sync: unsafe {
                    *library
                        .get(b"ssc_sensor_gyroscope_open_sync\0")
                        .map_err(|e| {
                            format!("libssc:ssc_sensor_gyroscope_open_sync not found {e}")
                        })?
                },
                ssc_sensor_accelerometer_new_sync: unsafe {
                    *library
                        .get(b"ssc_sensor_accelerometer_new_sync\0")
                        .map_err(|e| {
                            format!("libssc:ssc_sensor_accelerometer_new_sync not found {e}")
                        })?
                },
                ssc_sensor_accelerometer_open_sync: unsafe {
                    *library
                        .get(b"ssc_sensor_accelerometer_open_sync\0")
                        .map_err(|e| {
                            format!("libssc:ssc_sensor_accelerometer_open_sync not found {e}")
                        })?
                },
                _library: library,
            })
        }
    }

    pub type MeasurementHandlerCb = Box<dyn FnMut(f32, f32, f32)>;

    pub unsafe extern "C" fn measurement_handler(
        _obj: *mut GObject,
        x: f32,
        y: f32,
        z: f32,
        data: gpointer,
    ) {
        let cb: &mut Box<dyn FnMut(f32, f32, f32) + 'static> =
            unsafe { &mut *(data as *mut MeasurementHandlerCb) };
        cb(x, y, z);
    }

    pub unsafe extern "C" fn closure_destroy_handler(data: gpointer, _: *mut GClosure) {
        drop(unsafe { Box::from_raw(data as *mut Box<dyn FnMut(f32, f32, f32)>) });
    }
}
