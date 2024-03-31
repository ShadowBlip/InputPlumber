
# Keyboard DBus Interface API

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
  

## org.shadowblip.Input.Keyboard

### Properties


| Name | Access | Type | Description |
| --- | :---: | :---: | --- |
| **Name** | *read* | *s* |  |

### Methods

#### SendKey



##### Arguments

| Name | Direction | Type | Description |
| --- | :---: | :---: | --- |
  | **key** | *in* | *s* |  |
  | **value** | *in* | *b* |  |
  


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
