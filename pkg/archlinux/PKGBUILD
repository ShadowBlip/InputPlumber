# Maintainer: William Edwards <shadowapex@gmail.com>
pkgname=inputplumber-bin
_pkgbase=inputplumber
pkgver=v0.55.0
pkgrel=1
pkgdesc="Open source input router and remapper daemon for Linux"
arch=('x86_64')
url="https://github.com/ShadowBlip/inputplumber"
license=('GPL')
depends=('dbus' 'libevdev' 'libiio')
provides=('inputplumber')
conflicts=('inputplumber-git')
source=(inputplumber-$pkgver.tar.gz::https://github.com/ShadowBlip/inputplumber/releases/download/$pkgver/inputplumber.tar.gz)

sha256sums=('16cbcf6534ac6a7d0735d0bfdcf49706114d4a3548c4453e07b0b7583b8f78cc')

prepare() {
  cd "$srcdir/${_pkgbase}"
}

package() {
  cd "$srcdir/${_pkgbase}"
  cp -r ./* ${pkgdir}
}
