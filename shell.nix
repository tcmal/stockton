{ pkgs ? import <nixpkgs> {}, lib ? pkgs.stdenv.lib }:

pkgs.mkShell rec {
    buildInputs = with pkgs;[
        # Rust
        rustup

        # shaderc
        shaderc.bin
        shaderc.lib

        # Build script dependencies
        gcc
        pkg-config

        # Necessary X11 libraries
        xorg.libX11
        xorg.libXcursor
        xorg.libXrandr
        xorg.libXi

        # Vulkan
        vulkan-loader
        vulkan-validation-layers

        # Optional, but useful for debugging
        renderdoc
    ];

    LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";

    SHADERC_LIB_DIR = "${pkgs.shaderc.lib}/lib/";
}