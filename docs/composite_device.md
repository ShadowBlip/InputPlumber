# CompositeDevice DBus Interface API

## org.shadowblip.Input.CompositeDevice

### Properties

| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
| **DbusDevices** | *read* | *as* |  |
| **InterceptMode** | *readwrite* | *u* |  |
| **Name** | *read* | *s* |  |
| **SourceDevicePaths** | *read* | *as* |  |
| **TargetDevices** | *read* | *as* |  |

### Methods

### Signals

## org.freedesktop.DBus.Introspectable

### Methods

#### Introspect

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *s* |  |

### Signals

## org.freedesktop.DBus.Peer

### Methods

#### Ping

#### GetMachineId

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| \*\*\*\* | *out* | *s* |  |

### Signals

## org.freedesktop.DBus.Properties

### Methods

#### Get

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | *in* | *s* |  |
| **property_name** | *in* | *s* |  |
| \*\*\*\* | *out* | *v* |  |

#### Set

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | *in* | *s* |  |
| **property_name** | *in* | *s* |  |
| **value** | *in* | *v* |  |

#### GetAll

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | *in* | *s* |  |
| \*\*\*\* | *out* | *a{sv}* |  |

### Signals

#### PropertiesChanged

##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
| **interface_name** | \*\* | *s* |  |
| **changed_properties** | \*\* | *a{sv}* |  |
| **invalidated_properties** | \*\* | *as* |  |
