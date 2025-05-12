# Maintainer: William Edwards <shadowapex@gmail.com>
pkgname=inputplumber-bin
_pkgbase=inputplumber
pkgver=v0.57.2
pkgrel=1
pkgdesc="Open source input router and remapper daemon for Linux"
arch=('x86_64')
url="https://github.com/ShadowBlip/inputplumber"
license=('GPL')
depends=('dbus' 'libevdev' 'libiio')
provides=('inputplumber')
conflicts=('inputplumber-git')
source=(inputplumber-$pkgver.tar.gz::https://github.com/ShadowBlip/inputplumber/releases/download/$pkgver/inputplumber-x86_64.tar.gz)

sha256sums=('071e0b16afa8a6fc3a65bc8a72faa0dcf5e91ce4e9df9491fb0ee6f02e2159dc')

prepare() {
  cd "$srcdir/${_pkgbase}"
}

package() {
  cd "$srcdir/${_pkgbase}"
  cp -r ./* ${pkgdir}
}
