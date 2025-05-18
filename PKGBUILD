pkgname=watermark-cli
pkgver=1.0.0
pkgrel=1
pkgdesc="A command-line tool for adding watermarks to images and PDFs with support for batch processing and various watermark patterns."
arch=('x86_64')
url="https://github.com/chianti-ga/watermark-cli"
license=('GPL3')
depends=('gcc-libs')
#makedepends=('rust' 'cargo' 'git')
source=("$pkgname::git+$url.git#tag=v$pkgver")
sha256sums=('SKIP')

build() {
  cd "$srcdir/$pkgname"
  cargo build --release
}

package() {
  cd "$srcdir/$pkgname"
  install -Dm755 "target/release/watermark-cli" "$pkgdir/usr/bin/watermark-cli"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}

# vim:set ts=2 sw=2 et: