#dnf install pulseaudio-libs-devel argtable-devel libconfig-devel hidapi-devel libserialport-devel lua-devel libuv-devel libxdg-basedir-devel libxml2-devel procps-ng-devel
Summary: A device manager for racing sims
Name: cargopit
Version: 0.4.0
Release: 1
License: GPLv3+
Group: Applications/Sound
Source: https://github.com/M4X1K02/cargopit
URL: https://github.com/M4X1K02/cargopit
Distribution: Fedora Linux
Vendor: M4X1K02
Packager: M4X1K02 <maxik.secure@tutanota.com>
Requires: pulseaudio-libs argtable libconfig hidapi libserialport libuv libxdg-basedir lua-libs libxml2 procps-ng
BuildRequires: cmake gcc gcc-c++ make git cargo rust

%description
A device manager for Racing sims

# Builds whatever tree has been staged at %{_sourcedir}/cargopit, cloning it
# from upstream master only if nothing is staged. The unconditional clone this
# replaced meant an rpm's contents tracked master rather than the tag being
# built -- so a fix on the branch being released was absent from its own
# release. CI stages the checked-out tree; a bare `rpmbuild -ba` on a
# workstation still works exactly as before.
%prep
rm -rf $RPM_BUILD_DIR/cargopit
if [ ! -d $RPM_SOURCE_DIR/cargopit ]; then
    cd $RPM_SOURCE_DIR
    git clone https://github.com/M4X1K02/cargopit cargopit
    cd cargopit
    git submodule update --init --recursive
    cd ..
fi
cp -r $RPM_SOURCE_DIR/cargopit $RPM_BUILD_DIR/

%build
cd $RPM_BUILD_DIR/cargopit
cmake -B build
cd build
make
cd ..
cargo build --release -p cargopit --manifest-path Cargo.toml

%install
mkdir -p $RPM_BUILD_ROOT/usr/bin
cp $RPM_BUILD_DIR/cargopit/target/release/cargopit $RPM_BUILD_ROOT/usr/bin/cargopit
cp $RPM_BUILD_DIR/cargopit/build/cargopit-legacy $RPM_BUILD_ROOT/usr/bin/cargopit-legacy
if [ -x $RPM_BUILD_DIR/cargopit/build/tui/release/cargopit-tui ]; then
    cp $RPM_BUILD_DIR/cargopit/build/tui/release/cargopit-tui $RPM_BUILD_ROOT/usr/bin/cargopit-tui
elif [ -x $RPM_BUILD_DIR/cargopit/build/tui/debug/cargopit-tui ]; then
    cp $RPM_BUILD_DIR/cargopit/build/tui/debug/cargopit-tui $RPM_BUILD_ROOT/usr/bin/cargopit-tui
fi
install -D -m644 $RPM_BUILD_DIR/cargopit/tools/cargopit.desktop \
    $RPM_BUILD_ROOT/usr/share/applications/cargopit.desktop
install -D -m644 $RPM_BUILD_DIR/cargopit/tools/cargopit.svg \
    $RPM_BUILD_ROOT/usr/share/icons/hicolor/scalable/apps/cargopit.svg

%files
/usr/bin/cargopit
/usr/bin/cargopit-legacy
/usr/bin/cargopit-tui
/usr/share/applications/cargopit.desktop
/usr/share/icons/hicolor/scalable/apps/cargopit.svg
