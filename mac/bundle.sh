#!/bin/sh
# Builds Roam.app around target/release/roam: mac/bundle.sh <version>
set -e
app=target/release/Roam.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/Fonts"
cp target/release/roam "$app/Contents/MacOS/roam"
cp mac/roam.icns "$app/Contents/Resources/roam.icns"
cp assets/fonts/*.ttf "$app/Contents/Resources/Fonts/"
sed "s/VERSION/$1/g" mac/Info.plist > "$app/Contents/Info.plist"
