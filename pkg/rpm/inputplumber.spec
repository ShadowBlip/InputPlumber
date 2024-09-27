%global _name   inputplumber

Name:           inputplumber
Version:        0.35.3
Release:        0%{?dist}
Summary:        InputPlumber is an open source input routing and control daemon for Linux. It can be used to combine any number of input devices (like gamepads, mice, and keyboards) and translate their input to a variety of virtual device formats.

License:        GPLv3+
URL:            https://github.com/ShadowBlip/InputPlumber

BuildRequires:  libevdev-devel libiio-devel git make cargo libudev-devel llvm-devel clang-devel
Requires:       libevdev libiio
Recommends:     steam gamescope-session linuxconsoletools
Provides:       inputplumber
Conflicts:      hhd

%description
InputPlumber is an open source input routing and control daemon for Linux. It can be used to combine any number of input devices (like gamepads, mice, and keyboards) and translate their input to a variety of virtual device formats.

%prep
rm -rf %{_builddir}/InputPlumber
cd %{_builddir}
git clone --branch v%{version} --depth 1 %{url}

%build
cd %{_builddir}/InputPlumber
make build

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/share/dbus-1/system.d
mkdir -p %{buildroot}/usr/lib/systemd/system
mkdir -p %{buildroot}/usr/lib/udev/hwdb.d
mkdir -p %{buildroot}/usr/share/inputplumber/capability_maps
mkdir -p %{buildroot}/usr/share/inputplumber/devices
mkdir -p %{buildroot}/usr/share/inputplumber/profiles
mkdir -p %{buildroot}/usr/share/inputplumber/schema

install -D -m 755 %{_builddir}/InputPlumber/target/release/inputplumber %{buildroot}/usr/bin/inputplumber
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/share/dbus-1/system.d/org.shadowblip.InputPlumber.conf %{buildroot}/usr/share/dbus-1/system.d/org.shadowblip.InputPlumber.conf
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/lib/systemd/system/* %{buildroot}/usr/lib/systemd/system/
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/lib/udev/hwdb.d/59-inputplumber.hwdb %{buildroot}/usr/lib/udev/hwdb.d/59-inputplumber.hwdb
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/share/inputplumber/capability_maps/* %{buildroot}/usr/share/inputplumber/capability_maps/
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/share/inputplumber/devices/* %{buildroot}/usr/share/inputplumber/devices/
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/share/inputplumber/profiles/* %{buildroot}/usr/share/inputplumber/profiles/
install -D -m 644 %{_builddir}/InputPlumber/rootfs/usr/share/inputplumber/schema/* %{buildroot}/usr/share/inputplumber/schema/

%post
udevadm control --reload-rules
udevadm trigger
systemctl daemon-reload
systemctl enable inputplumber.service
systemctl start inputplumber.service

%preun
systemctl stop inputplumber.servce
systemctl disable inputplumber.servce
%systemd_preun inputplumber.service

%files
/usr/bin/inputplumber
/usr/share/dbus-1/system.d/org.shadowblip.InputPlumber.conf
/usr/lib/systemd/system/inputplumber.service
/usr/lib/udev/hwdb.d/59-inputplumber.hwdb
/usr/share/inputplumber/capability_maps/ally_type1.yaml
/usr/share/inputplumber/capability_maps/anbernic_type1.yaml
/usr/share/inputplumber/capability_maps/ayaneo_type1.yaml
/usr/share/inputplumber/capability_maps/ayaneo_type2.yaml
/usr/share/inputplumber/capability_maps/ayaneo_type3.yaml
/usr/share/inputplumber/capability_maps/ayaneo_type4.yaml
/usr/share/inputplumber/capability_maps/ayaneo_type5.yaml
/usr/share/inputplumber/capability_maps/ayaneo_type6.yaml
/usr/share/inputplumber/capability_maps/ayn_type1.yaml
/usr/share/inputplumber/capability_maps/gpd_type1.yaml
/usr/share/inputplumber/capability_maps/gpd_type2.yaml
/usr/share/inputplumber/capability_maps/gpd_type3.yaml
/usr/share/inputplumber/capability_maps/msiclaw_type1.yaml
/usr/share/inputplumber/capability_maps/onexplayer_type1.yaml
/usr/share/inputplumber/capability_maps/onexplayer_type2.yaml
/usr/share/inputplumber/capability_maps/onexplayer_type3.yaml
/usr/share/inputplumber/capability_maps/onexplayer_type4.yaml
/usr/share/inputplumber/capability_maps/orangepi_type1.yaml
/usr/share/inputplumber/devices/10-ignore_unsupported.yaml
/usr/share/inputplumber/devices/50-anbernic_win600.yaml
/usr/share/inputplumber/devices/50-aokzoe_a1.yaml
/usr/share/inputplumber/devices/50-ayaneo_2021.yaml
/usr/share/inputplumber/devices/50-ayaneo_2s.yaml
/usr/share/inputplumber/devices/50-ayaneo_2.yaml
/usr/share/inputplumber/devices/50-ayaneo_air_1s.yaml
/usr/share/inputplumber/devices/50-ayaneo_air_plus_mendo.yaml
/usr/share/inputplumber/devices/50-ayaneo_air_plus.yaml
/usr/share/inputplumber/devices/50-ayaneo_air.yaml
/usr/share/inputplumber/devices/50-ayaneo_flip.yaml
/usr/share/inputplumber/devices/50-ayaneo_kun.yaml
/usr/share/inputplumber/devices/50-ayaneo_next.yaml
/usr/share/inputplumber/devices/50-ayaneo_slide.yaml
/usr/share/inputplumber/devices/50-ayn_loki_max.yaml
/usr/share/inputplumber/devices/50-ayn_loki_mini_pro.yaml
/usr/share/inputplumber/devices/50-ayn_loki_zero.yaml
/usr/share/inputplumber/devices/50-gpd_win3.yaml
/usr/share/inputplumber/devices/50-gpd_win4.yaml
/usr/share/inputplumber/devices/50-gpd_winmax2.yaml
/usr/share/inputplumber/devices/50-gpd_winmini.yaml
/usr/share/inputplumber/devices/50-legion_go.yaml
/usr/share/inputplumber/devices/50-msi_claw.yaml
/usr/share/inputplumber/devices/50-onexplayer_2.yaml
/usr/share/inputplumber/devices/50-onexplayer_amd.yaml
/usr/share/inputplumber/devices/50-onexplayer_intel.yaml
/usr/share/inputplumber/devices/50-onexplayer_mini_a07.yaml
/usr/share/inputplumber/devices/50-onexplayer_mini_pro.yaml
/usr/share/inputplumber/devices/50-onexplayer_onexfly.yaml
/usr/share/inputplumber/devices/50-orangepi_neo.yaml
/usr/share/inputplumber/devices/50-rog_ally_x.yaml
/usr/share/inputplumber/devices/50-rog_ally.yaml
/usr/share/inputplumber/devices/50-steam_deck.yaml
/usr/share/inputplumber/devices/60-ps4_gamepad.yaml
/usr/share/inputplumber/devices/60-ps5_ds-edge_gamepad.yaml
/usr/share/inputplumber/devices/60-ps5_ds_gamepad.yaml
/usr/share/inputplumber/devices/60-switch_pro.yaml
/usr/share/inputplumber/devices/60-xbox_360_gamepad.yaml
/usr/share/inputplumber/devices/60-xbox_gamepad.yaml
/usr/share/inputplumber/devices/60-xbox_one_bt_gamepad.yaml
/usr/share/inputplumber/devices/60-xbox_one_elite_gamepad.yaml
/usr/share/inputplumber/devices/60-xbox_one_gamepad.yaml
/usr/share/inputplumber/devices/69-ignore_generic.yaml
/usr/share/inputplumber/devices/70-generic_gamepad.yaml
/usr/share/inputplumber/profiles/default.yaml
/usr/share/inputplumber/profiles/mouse_keyboard_wasd.yaml
/usr/share/inputplumber/profiles/test.yaml
/usr/share/inputplumber/schema/capability_map_v1.json
/usr/share/inputplumber/schema/composite_device_v1.json
/usr/share/inputplumber/schema/device_profile_v1.json

%changelog
* Tue Aug 6 2024 William Edwards [0.33.1-0]
- Initial spec file for Fedora based testing and distribution (please refer to https://github.com/ShadowBlip/InputPlumber)
