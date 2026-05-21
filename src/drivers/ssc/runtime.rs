use std::{error::Error, ptr::null_mut};

use super::bindings::{
    glib::{glib_error, gpointer, GError, GlibDylib},
    gobject::{FnGObjectUnref, GObjectDylib},
    ssc::{
        closure_destroy_handler, measurement_handler, FnSscSensorNewSync, FnSscSensorOpenSync,
        SscDylib,
    },
};

/* Wrapper for libssc sensor objects (mostly just a wrapper for GObject) */
pub struct SscObject {
    pub ptr: *mut std::ffi::c_void,
    g_object_unref: FnGObjectUnref,
}

impl Drop for SscObject {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                (self.g_object_unref)(self.ptr as *mut _);
            }
        }
    }
}

impl SscObject {
    pub fn set_measurement_handler<FnMeasure: FnMut(f32, f32, f32) + 'static>(
        &self,
        runtime: &SscRuntime,
        callback: FnMeasure,
    ) {
        let boxed: Box<Box<dyn FnMut(f32, f32, f32)>> = Box::new(Box::new(callback));
        let user_data = Box::into_raw(boxed) as gpointer;

        unsafe {
            (runtime.libgobject.g_signal_connect_data)(
                self.ptr as *mut _,
                b"measurement\0".as_ptr() as *const _,
                Some(std::mem::transmute(measurement_handler as *const ())),
                user_data,
                Some(closure_destroy_handler),
                0,
            );
        }
    }
}

/// This contains the dynamic libraries and all method pointers needed for libssc to work.
/// This currently consists of: glib-2.0, gobject-2.0, libssc
pub struct SscRuntime {
    pub(crate) libssc: SscDylib,
    pub(crate) libglib: GlibDylib,
    pub(crate) libgobject: GObjectDylib,
}

impl SscRuntime {
    pub fn load() -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self {
            libssc: SscDylib::load()?,
            libglib: GlibDylib::load()?,
            libgobject: GObjectDylib::load()?,
        })
    }

    /// The signatures for gyroscope_(open/new)_sync and accelerometer_(open/new)_sync are the same, so we can reuse some code
    fn create_measurement_sensor(
        &self,
        new_fn: FnSscSensorNewSync,
        open_fn: FnSscSensorOpenSync,
    ) -> Result<SscObject, Box<dyn Error + Send + Sync>> {
        let mut err: *mut GError = null_mut();
        let ptr = unsafe { (new_fn)(std::ptr::null_mut(), &mut err) };

        // Instantiate the sensor and get our GObject ptr from it
        if let Some(v) = glib_error(err) {
            return Err(format!("Failed to instantiate SSC sensor: {v}").into());
        }

        if ptr.is_null() {
            return Err("Failed to instantiate SSC sensor: (got a null pointer)".into());
        }

        // "Open" the sensor (make it start doing stuff)
        // note: We set the data callback later using set_measurement_handler, so this is just new sensor -> open sensor
        unsafe {
            err = std::ptr::null_mut();
            (open_fn)(ptr, std::ptr::null_mut(), &mut err)
        };

        if let Some(v) = glib_error(err) {
            return Err(format!("Failed to open SSC sensor: {v}").into());
        }

        Ok(SscObject {
            ptr,

            // note: SscObject should keep the SscRuntime alive
            // This should be dropped before the SscRuntime, so it's probably fine
            g_object_unref: self.libgobject.g_object_unref,
        })
    }

    pub fn create_gyroscope(&self) -> Result<SscObject, Box<dyn Error + Send + Sync>> {
        self.create_measurement_sensor(
            self.libssc.ssc_sensor_gyroscope_new_sync,
            self.libssc.ssc_sensor_gyroscope_open_sync,
        )
    }

    pub fn create_accelerometer(&self) -> Result<SscObject, Box<dyn Error + Send + Sync>> {
        self.create_measurement_sensor(
            self.libssc.ssc_sensor_accelerometer_new_sync,
            self.libssc.ssc_sensor_accelerometer_open_sync,
        )
    }
}
