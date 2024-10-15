# Maintainer: William Edwards <shadowapex@gmail.com>
pkgname=inputplumber-bin
_pkgbase=inputplumber
pkgver=v0.36.3
pkgrel=1
pkgdesc="Open source input router and remapper daemon for Linux"
arch=('x86_64')
url="https://github.com/ShadowBlip/inputplumber"
license=('GPL')
depends=('dbus' 'libevdev' 'libiio')
provides=('inputplumber')
conflicts=('inputplumber-git')
source=(inputplumber-$pkgver.tar.gz::https://github.com/ShadowBlip/inputplumber/releases/download/$pkgver/inputplumber.tar.gz)

sha256sums=('dbb31b9303ad54ccc00c060f94673cf6a8dd804ecbc1e8b2a0e2aacf3069dc34')

prepare() {
  cd "$srcdir/${_pkgbase}"
}

package() {
  cd "$srcdir/${_pkgbase}"
  cp -r ./* ${pkgdir}
}
