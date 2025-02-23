# Maintainer: William Edwards <shadowapex@gmail.com>
pkgname=inputplumber-bin
_pkgbase=inputplumber
pkgver=v0.48.0
pkgrel=1
pkgdesc="Open source input router and remapper daemon for Linux"
arch=('x86_64')
url="https://github.com/ShadowBlip/inputplumber"
license=('GPL')
depends=('dbus' 'libevdev' 'libiio')
provides=('inputplumber')
conflicts=('inputplumber-git')
source=(inputplumber-$pkgver.tar.gz::https://github.com/ShadowBlip/inputplumber/releases/download/$pkgver/inputplumber.tar.gz)

sha256sums=('6695333e7723171bbfd7fc6082d40368b1e89dd19de1936a56455eeefeb92d7e')

prepare() {
  cd "$srcdir/${_pkgbase}"
}

package() {
  cd "$srcdir/${_pkgbase}"
  cp -r ./* ${pkgdir}
}
