use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{fdo, message::Header, Connection, Proxy};

pub async fn check_polkit(hdr: Option<Header<'_>>, action_id: &str) -> fdo::Result<()> {
    let connection = Connection::system()
        .await
        .map_err(|e| fdo::Error::Failed(format!("Failed to acquire dbus connection: {}", e)))?;
    check_polkit_with_connection(hdr, action_id, connection).await
}

pub async fn check_polkit_with_connection(
    hdr: Option<Header<'_>>,
    action_id: &str,
    connection: Connection,
) -> fdo::Result<()> {
    let hdr = hdr.ok_or_else(|| {
        fdo::Error::InteractiveAuthorizationRequired(format!(
            "Authentication required for {}",
            action_id
        ))
    })?;

    let sender = hdr
        .sender()
        .ok_or_else(|| fdo::Error::Failed("Missing sender".into()))?;

    let pid: u32 = connection
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "GetConnectionUnixProcessID",
            &(sender.as_str()),
        )
        .await
        .map_err(|e| fdo::Error::Failed(format!("Failed to get process ID: {}", e)))?
        .body()
        .deserialize()?;

    let mut subj_details: HashMap<String, OwnedValue> = HashMap::new();
    subj_details.insert(
        "pid".into(),
        OwnedValue::try_from(Value::U32(pid))
            .map_err(|e| fdo::Error::Failed(format!("Failed to get OwnedValue for pid: {}", e)))?,
    );
    subj_details.insert(
        "start-time".into(),
        OwnedValue::try_from(Value::U64(0)).map_err(|e| {
            fdo::Error::Failed(format!("Failed to get OwnedValue for start: {}", e))
        })?,
    );
    let subject = ("unix-process".to_string(), subj_details);

    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .await
    .map_err(|e| fdo::Error::Failed(e.to_string()))?;

    let call_details: HashMap<String, String> = HashMap::new();
    let flags: u32 = 0x0000_0001;
    let cancellation_id = "";

    let (is_authorized, is_challenge, _details): (bool, bool, HashMap<String, String>) = proxy
        .call_method(
            "CheckAuthorization",
            &(subject, action_id, &call_details, flags, cancellation_id),
        )
        .await
        .map_err(|e| fdo::Error::Failed(e.to_string()))?
        .body()
        .deserialize()?;
    if is_authorized {
        Ok(())
    } else if is_challenge {
        Err(fdo::Error::InteractiveAuthorizationRequired(format!(
            "Authentication required for {}",
            action_id
        )))
    } else {
        Err(fdo::Error::Failed(format!(
            "Not authorized for {}",
            action_id
        )))
    }
}
