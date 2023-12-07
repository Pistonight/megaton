#!/usr/bin/bash

# This script builds and installs a custom rustc toolchain "megaton"
TOOLCHAIN_NAME=megaton
if rustup toolchain list -v | grep $TOOLCHAIN_NAME; then
    if ! rustc +megaton -vV; then
        echo "Toolchain '$TOOLCHAIN_NAME' is installed but not working. Uninstalling..."
        rustup toolchain uninstall $TOOLCHAIN_NAME
    else
        echo "Toolchain '$TOOLCHAIN_NAME' already exists. Skipping build."
        echo "To rebuild, uninstall it with 'rustup toolchain uninstall $TOOLCHAIN_NAME'"
        echo "then run this script again."
        echo ""
        exit 0
    fi
fi

if [ -z "$MEGATON_HOME" ]; then
    echo "MEGATON_HOME is not set. Please set it as the absolute path to your local megaton repository"
    exit 1
fi

echo "Megaton is at '$MEGATON_HOME'"

if ! RUSTUP=$(which rustup); then
    echo "Cannot find rustup!"
    echo "Please install Rust from from https://rustup.rs/"
    exit 1
fi

if ! RUSTC=$(which rustc); then
    echo "Cannot find rustc!"
    echo "Please install Rust from from https://rustup.rs/"
    exit 1
fi

HOST_TARGET=$(rustc -vV | grep "host" | sed 's/host: //g')
echo "Host target is '$HOST_TARGET'"

if ! GIT=$(which git); then
    echo "Cannot find git! Please install it."
    exit 1
fi

if ! CMAKE=$(which cmake); then
    echo "Cannot find cmake!"
    echo "CMake is required to build llvm. Please install it."
    exit 1
fi

if ! NINJA=$(which ninja); then
    echo "Cannot find ninja!"
    echo "Ninja is required to build llvm"
    echo "Please install it from https://github.com/ninja-build/ninja"
    exit 1
fi

TOOLCHAIN_RUSTC=$MEGATON_HOME/toolchain/rustc
RUST_REPO=$TOOLCHAIN_RUSTC/rust

if ! [ -d "$RUST_REPO" ]; then
    RUST_REPO_URL="https://github.com/rust-lang/rust.git"
    echo "Cloning rust repo from '$RUST_REPO_URL'"
    git clone $RUST_REPO_URL "$RUST_REPO" --depth 1
fi

echo "Copying config.toml"
cp $TOOLCHAIN_RUSTC/config.toml $RUST_REPO/config.toml
echo "host = [\"$HOST_TARGET\"]" >> $RUST_REPO/config.toml
echo "target = [\"$HOST_TARGET\", \"aarch64-unknown-hermit\", \"aarch64-nintendo-switch-freestanding\"]" >> $RUST_REPO/config.toml

cd $RUST_REPO
echo "Entered '$(pwd)'"

./x build --stage 1 library

echo "Installing built toolchain as '$TOOLCHAIN_NAME'"
rustup toolchain link $TOOLCHAIN_NAME $RUST_REPO/build/host/stage1
echo ""
echo "Installed at:"
rustup toolchain list -v | grep $TOOLCHAIN_NAME
rustc +megaton -vV

echo ""
echo "Done!"
echo ""
echo "To remove the toolchain, run 'rustup toolchain uninstall $TOOLCHAIN_NAME'"
