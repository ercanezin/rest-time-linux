# Maintainer: Autonomous Engineer <engineer@local>
pkgname=rest-time-linux
pkgver=1.0.0
pkgrel=1
pkgdesc="Enterprise native break and micro-pause daemon for Arch Linux/CachyOS"
arch=('x86_64')
url="https://github.com/ercanezin/rest-time-linux"
license=('MIT' 'Apache-2.0')
depends=(
    'gtk4'
    'gtk4-layer-shell'
    'cairo'
    'libpipewire'
    'alsa-lib'
    'dbus'
)
makedepends=(
    'cargo'
    'rust'
    'pkgconf'
)
source=("$pkgname-$pkgver.tar.gz")
sha256sums=('SKIP')

prepare() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --frozen --release --all-features
}

check() {
    cd "$pkgname-$pkgver"
    cargo test --frozen --all-targets
}

package() {
    cd "$pkgname-$pkgver"
    install -Dm755 target/release/rest-time-linux "$pkgdir/usr/bin/rest-time-linux"
    install -Dm644 resources/rest-time.service "$pkgdir/usr/lib/systemd/user/rest-time.service"
    install -Dm644 resources/rest-time.desktop "$pkgdir/usr/share/applications/rest-time.desktop"
    install -Dm644 resources/icons/rest-time-active.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/rest-time-active.svg"
    install -Dm644 resources/icons/rest-time-paused.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/rest-time-paused.svg"
    install -Dm644 resources/icons/rest-time-break.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/rest-time-break.svg"
    install -Dm644 LICENSE-MIT "$pkgdir/usr/share/licenses/$pkgname/LICENSE" 2>/dev/null || true
}
