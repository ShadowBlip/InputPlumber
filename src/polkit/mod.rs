//! References:
//!  - https://vwangsf.medium.com/creating-a-d-bus-service-with-dbus-python-and-polkit-authentication-4acc9bc5ed29
//!  - https://www.freedesktop.org/software/polkit/docs/master/eggdbus-interface-org.freedesktop.PolicyKit1.Authority.html#eggdbus-method-org.freedesktop.PolicyKit1.Authority.CheckAuthorization

use std::{collections::HashMap, error::Error};

use zbus::{
    names::{BusName, UniqueName},
    zvariant::Value,
    Connection,
};

pub mod authority;

/// Returns true if the given DBus sender is authorized by polkit to perform
/// the given action.
pub async fn is_polkit_authorized<'m>(
    conn: &Connection,
    sender: &UniqueName<'m>,
    action_id: &str,
) -> Result<bool, Box<dyn Error>> {
    // With the sender, we can ask DBus what the sender's process id is:
    //   busctl call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus GetConnectionUnixProcessID s org.shadowblip.InputPlumber
    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
    let bus_name = BusName::from(sender.to_owned());
    let sender_pid: u32 = dbus.get_connection_unix_process_id(bus_name).await?;
    log::debug!("Sender has PID: {sender_pid}");

    // Get the PolKit authority to validate the sender
    let polkit_auth = authority::AuthorityProxy::new(&conn).await?;

    // The subject describes subjects such as UNIX processes. It is typically
    // used to check if a given process is authorized for an action.
    let subject_kind = "unix-process";
    let mut subject_details = HashMap::new();
    let pid = Value::new(sender_pid);
    subject_details.insert("pid", &pid);
    let start_time = Value::new(0 as u64);
    subject_details.insert("start-time", &start_time);
    let subject = (subject_kind, subject_details);

    // Details describing the action.
    let mut details = HashMap::new();
    details.insert("AllowUserInteraction", "false");

    // Authorization flags
    //   0 - No flags set.
    //   1 - If the Subject can obtain the authorization through authentication,
    //       and an authentication agent is available, then attempt to do so.
    //       Note, this means that the CheckAuthorization() method will block
    //       while the user is being asked to authenticate.
    let flags = 0;

    // A unique id used to cancel the the authentication check via
    // CancelCheckAuthorization() or the empty string if cancellation is not
    // needed.
    let cancellation_id = "";

    // Use PolKit to check if the sender is authorized for this action
    let (is_authorized, _is_challenge, _details) = polkit_auth
        .check_authorization(&subject, action_id, details, flags, cancellation_id)
        .await?;

    Ok(is_authorized)
}
