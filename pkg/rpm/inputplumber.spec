Name:           inputplumber
Version:        0.74.0
Release:        0%{?dist}
Summary:        InputPlumber is an open source input routing and control daemon for Linux. It can be used to combine any number of input devices (like gamepads, mice, and keyboards) and translate their input to a variety of virtual device formats.
License:        GPLv3+
URL:            https://github.com/ShadowBlip/InputPlumber
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz

BuildRequires: libevdev-devel
BuildRequires: libiio-devel
BuildRequires: git
BuildRequires: make
BuildRequires: cargo
BuildRequires: libudev-devel
BuildRequires: llvm-devel
BuildRequires: clang-devel
BuildRequires:  systemd-rpm-macros
Requires:       libevdev
Requires:       libiio
Requires:       polkit
Recommends:     linuxconsoletools
Provides:       inputplumber
Conflicts:      hhd

%description
InputPlumber is an open source input routing and control daemon for Linux. It can be used to combine any number of input devices (like gamepads, mice, and keyboards) and translate their input to a variety of virtual device formats.

%prep
%autosetup -n InputPlumber-%{version} -p1

%build
make build

%install
make install PREFIX=%{buildroot}/usr NO_RELOAD=true

%post
%systemd_post inputplumber.service

%preun
%systemd_preun inputplumber.service

%postun
%systemd_postun inputplumber.service

%files
%doc README.md
%{_bindir}/inputplumber
%{_datadir}/dbus-1/system.d/org.shadowblip.InputPlumber.conf
%{_unitdir}/inputplumber.service
%{_unitdir}/inputplumber-suspend.service
%{_udevhwdbdir}/*.hwdb
%{_udevrulesdir}/*.rules
%{_datadir}/inputplumber/capability_maps/*.yaml
%{_datadir}/inputplumber/devices/*.yaml
%{_datadir}/inputplumber/profiles/*.yaml
%{_datadir}/inputplumber/schema/*.json
%{_datadir}/polkit-1/actions/org.shadowblip.InputPlumber.policy
%{_datadir}/polkit-1/rules.d/org.shadowblip.InputPlumber.rules

%changelog
* Tue Aug 6 2024 William Edwards [0.33.1-0]
- Initial spec file for Fedora based testing and distribution (please refer to https://github.com/ShadowBlip/InputPlumber)
