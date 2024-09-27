# Maintainer: William Edwards <shadowapex@gmail.com>
pkgname=inputplumber-bin
_pkgbase=inputplumber
pkgver=v0.35.4
pkgrel=1
pkgdesc="Open source input router and remapper daemon for Linux"
arch=('x86_64')
url="https://github.com/ShadowBlip/inputplumber"
license=('GPL')
depends=('dbus' 'libevdev' 'libiio')
provides=('inputplumber')
conflicts=('inputplumber-git')
source=(inputplumber-$pkgver.tar.gz::https://github.com/ShadowBlip/inputplumber/releases/download/$pkgver/inputplumber.tar.gz)

sha256sums=('a0938b5b9ea823863de5a5b8bff1d749148359bd6c90039ddfcdb0acfbcfa1a4')

prepare() {
  cd "$srcdir/${_pkgbase}"
}

package() {
  cd "$srcdir/${_pkgbase}"
  cp -r ./* ${pkgdir}
}
