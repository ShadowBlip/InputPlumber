
# Source HIDRaw DBus Interface API

## org.shadowblip.Input.Source.HIDRawDevice

### Properties


| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
| **InterfaceNumber** | *read* | *i* |  |
| **Manufacturer** | *read* | *s* |  |
| **Path** | *read* | *s* |  |
| **Product** | *read* | *s* |  |
| **ProductId** | *read* | *s* |  |
| **ReleaseNumber** | *read* | *s* |  |
| **SerialNumber** | *read* | *s* |  |
| **VendorId** | *read* | *s* |  |

### Methods


### Signals

## org.freedesktop.DBus.Properties

### Methods

#### Get



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **interface\_name** | *in* | *s* |  |
  | **property\_name** | *in* | *s* |  |
  | **** | *out* | *v* |  |
  

#### Set



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **interface\_name** | *in* | *s* |  |
  | **property\_name** | *in* | *s* |  |
  | **value** | *in* | *v* |  |
  

#### GetAll



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **interface\_name** | *in* | *s* |  |
  | **** | *out* | *a{sv}* |  |
  


### Signals

#### PropertiesChanged



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **interface\_name** | ** | *s* |  |
  | **changed\_properties** | ** | *a{sv}* |  |
  | **invalidated\_properties** | ** | *as* |  |
  

## org.freedesktop.DBus.Introspectable

### Methods

#### Introspect



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **** | *out* | *s* |  |
  


### Signals

## org.freedesktop.DBus.Peer

### Methods

#### Ping




#### GetMachineId



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **** | *out* | *s* |  |
  


### Signals
