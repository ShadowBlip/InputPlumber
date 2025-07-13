use std::{collections::HashMap, convert::TryInto};

use thiserror::Error;
use zbus::{names::InterfaceName, object_server::Interface, zvariant::ObjectPath, Connection};

pub mod composite_device;
pub mod force_feedback;
pub mod manager;
pub mod performance;
pub mod source;
pub mod target;

#[derive(Error, Debug)]
pub enum InterfaceError {
    #[error("invalid object path")]
    PathError,
}

/// Manages dbus interface registration and deregistration. When the interface
/// manager goes out of scope, all registered interfaces are automatically
/// unregistered.
pub struct DBusInterfaceManager {
    dbus: Connection,
    dbus_path: String,
    dbus_ifaces: HashMap<InterfaceName<'static>, UnregisterFn>,
}

impl DBusInterfaceManager {
    /// Creates a new dbus interface manager to manage one or more dbus interfaces
    /// for the given dbus path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let dbus = Connection::system();
    /// let mut registry = InterfaceRegistry::new(dbus, "/org/shadowblip/InputPlumber/Manager").unwrap();
    ///
    /// let iface = MyIface::new();
    /// registry.register(iface);
    /// ```
    pub fn new<'p, P>(dbus: Connection, path: P) -> Result<Self, InterfaceError>
    where
        P: TryInto<ObjectPath<'p>>,
        P::Error: Into<zbus::Error>,
    {
        let dbus_path: ObjectPath<'p> = match path.try_into() {
            Ok(path) => path,
            Err(_) => {
                return Err(InterfaceError::PathError);
            }
        };
        Ok(Self {
            dbus,
            dbus_path: dbus_path.to_string(),
            dbus_ifaces: Default::default(),
        })
    }

    /// Returns the dbus path used for all managed interfaces.
    pub fn path(&self) -> &str {
        self.dbus_path.as_str()
    }

    /// Returns the dbus connection used by this manager.
    pub fn connection(&self) -> &Connection {
        &self.dbus
    }

    /// Register and start the given dbus interface.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// registry.register::<MyIface>(iface);
    /// ```
    pub fn register<I>(&mut self, iface: I) -> Result<(), zbus::Error>
    where
        I: Interface + Unregisterable,
    {
        let iface_name = I::name();
        self.dbus_ifaces.insert(iface_name.clone(), &I::unregister);

        // Start the interface in its own task
        let dbus = self.dbus.clone();
        let dbus_path = self.dbus_path.clone();
        tokio::task::spawn(async move {
            log::debug!("Starting dbus interface `{iface_name}` on `{dbus_path}`");
            let object_server = dbus.object_server();
            if let Err(e) = object_server.at(dbus_path.as_str(), iface).await {
                log::debug!("Failed to start interface `{iface_name}` at path `{dbus_path}`: {e}");
                return;
            }
            log::debug!("Started dbus interface `{iface_name}` on `{dbus_path}`");
        });

        Ok(())
    }

    /// Unregister and remove the given dbus interface
    ///
    /// # Examples
    ///
    /// ```no_run
    /// registry.unregister(&MyIface::name());
    /// ```
    pub fn unregister(&mut self, iface_name: &InterfaceName<'static>) {
        let Some(unregister) = self.dbus_ifaces.remove(iface_name) else {
            log::trace!("Interface already unregistered: {iface_name}");
            return;
        };

        unregister(self.dbus.clone(), self.dbus_path.to_string());
    }

    /// Unregister all registered dbus interfaces. This is done automatically when
    /// the [InterfaceRegistry] falls out of scope.
    pub fn unregister_all(&mut self) {
        let ifaces: Vec<InterfaceName<'static>> = self.dbus_ifaces.keys().cloned().collect();
        for iface in ifaces {
            self.unregister(&iface);
        }
    }
}

impl Drop for DBusInterfaceManager {
    /// Unregister all dbus interfaces when this goes out of scope
    fn drop(&mut self) {
        self.unregister_all();
    }
}

/// This trait is used to define how to remove a particular dbus interface. In
/// most cases, you simply need to implement this trait on an existing dbus
/// interface with the default implementation to automatically handle dbus deregistration.
///
/// # Examples
///
/// ```no_run
/// struct MyIface(u32);
///
/// #[interface(name = "org.myiface.MyIface")]
/// impl MyIface {
///      #[zbus(property)]
///      async fn count(&self) -> u32 {
///          self.0
///      }
/// }
///
/// impl Unregisterable for MyIface {}
/// ```
pub trait Unregisterable {
    fn unregister(dbus: Connection, path: String)
    where
        Self: Interface,
        Self: Sized,
    {
        tokio::task::spawn(async move {
            let iface_name = Self::name();
            log::debug!("Stopping dbus interface `{iface_name}` on `{path}`");
            let object_server = dbus.object_server();
            object_server
                .remove::<Self, String>(path.clone())
                .await
                .unwrap_or_default();
            log::debug!("Stopped dbus interface `{iface_name}` on `{path}`");
        });
    }
}

type UnregisterFn = &'static dyn Fn(Connection, String);
