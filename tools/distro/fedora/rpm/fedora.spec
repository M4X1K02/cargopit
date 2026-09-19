#dnf install pulseaudio-libs-devel argtable-devel libconfig-devel hidapi-devel libserialport-devel lua-devel libuv-devel libxdg-basedir-devel libxml2-devel procps-ng-devel
Summary: A device manager for racing sims
Name: cargopit
Version: 0.0.5
Release: 1
License: GPLv3+
Group: Applications/Sound
Source: https://github.com/M4X1K02/cargopit
URL: https://spacefreak18.github.io/simapi
Distribution: Fedora Linux
Vendor: spacefreak18
Packager: Paul Jones <paul@spacefreak18.xyz>
Requires: pulseaudio-libs argtable libconfig hidapi libserialport libuv libxdg-basedir lua-libs libxml2 procps-ng gtk3 libcurl mesa-libGL

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
cmake -B build -DBUILD_GUI=on
cd build
make

%install
mkdir -p $RPM_BUILD_ROOT/usr/bin
cp $RPM_BUILD_DIR/cargopit/build/cargopit $RPM_BUILD_ROOT/usr/bin/cargopit
cp $RPM_BUILD_DIR/cargopit/build/gcargopit $RPM_BUILD_ROOT/usr/bin/gcargopit

%files
/usr/bin/cargopit
/usr/bin/gcargopit
