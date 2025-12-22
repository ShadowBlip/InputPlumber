
# CompositeDevice DBus Interface API

## org.shadowblip.Input.CompositeDevice

### Properties


| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
| **Capabilities** | *read* | *as* |  |
| **DbusDevices** | *read* | *as* |  |
| **InterceptMode** | *readwrite* | *u* |  |
| **Name** | *read* | *s* |  |
| **ProfileName** | *read* | *s* |  |
| **SourceDevicePaths** | *read* | *as* |  |
| **TargetDevices** | *read* | *as* |  |

### Methods

#### LoadProfilePath



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **path** | *in* | *s* |  |
  


### Signals

## org.freedesktop.DBus.Introspectable

### Methods

#### Introspect



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **** | *out* | *s* |  |
  


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
  

## org.freedesktop.DBus.Peer

### Methods

#### Ping




#### GetMachineId



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **** | *out* | *s* |  |
  


### Signals
