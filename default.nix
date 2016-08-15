
# Rust's x11-rs package exposes THESE libraries :
#if cfg!(feature="dpms") { pkg_config::find_library("xext").unwrap(); }
#if cfg!(feature="glx") { pkg_config::find_library("gl").unwrap(); }
#if cfg!(feature="xcursor") { pkg_config::find_library("xcursor").unwrap(); }
#if cfg!(feature="xf86vmode") { pkg_config::find_library("xxf86vm").unwrap(); }
#if cfg!(feature="xft") { pkg_config::find_library("xft").unwrap(); }
#if cfg!(feature="xinerama") { pkg_config::find_library("xinerama").unwrap(); }
#if cfg!(feature="xinput") { pkg_config::find_library("xi").unwrap(); }
#if cfg!(feature="xlib") { pkg_config::find_library("x11").unwrap(); }
#if cfg!(feature="xlib_xcb") { pkg_config::find_library("x11-xcb").unwrap(); }
#if cfg!(feature="xmu") { pkg_config::find_library("xmu").unwrap(); }
#if cfg!(feature="xrandr") { pkg_config::find_library("xrandr").unwrap(); }
#if cfg!(feature="xrecord") { pkg_config::find_library("xtst").unwrap(); 
#if cfg!(feature="xrender") { pkg_config::find_library("xrender").unwrap(); 
#if cfg!(feature="xss") { pkg_config::find_library("xscrnsaver").unwrap(); }
#if cfg!(feature="xt") { pkg_config::find_library("xt").unwrap(); }
#if cfg!(feature="xtest") { pkg_config::find_library("xtst").unwrap(); }

let
  pkgs = import <nixpkgs> {};
  stdenv = pkgs.stdenv;
  x11 = pkgs.xorg.libX11;
  xCursor = pkgs.xorg.libXcursor;
  xf86vmode = pkgs.xorg.libXxf86vm;
  xi = pkgs.xorg.libXi;
  mesanoglu = pkgs.mesa_noglu;
  mesa = pkgs.mesa;
  xRandr = pkgs.xorg.libXrandr;
  udev = pkgs.udev;
  xInerama = pkgs.xorg.libXinerama;
  xProto = pkgs.xorg.xproto;
  xMu = pkgs.xorg.libXmu;
  xFt = pkgs.xorg.libXft;
  xExt = pkgs.xorg.libXext;
in rec {
  devEnvRustGestalt = stdenv.mkDerivation rec {
    name = "gestalt";
    buildInputs = with pkgs; [
      mesa
      mesa_noglu
      gtk
      libvpx
      SDL2
      SDL2_mixer
      xorg.libX11
      xorg.libXft
      xorg.libXinerama
      xorg.libXcursor
      xorg.xproto
      xorg.libXxf86vm
      xorg.libXi
      xorg.libXrandr
      xorg.libXmu
      xorg.libXft
      xorg.libXext
    ];
    /*configureFlags = "--x-includes=${libX11.dev}/include --x-libraries=${libX11.out}/lib";
    postBuild = ''
      find . -name 'config' -type f | while read i; do
      sed -i "s@libX11.so.6@${libX11.out}/lib/libX11.so.6@g" $i
      done
    '';/*/
    LD_LIBRARY_PATH="/run/opengl-driver/lib:${pkgs.xorg.libX11.out}/lib:${xCursor.out}/lib:
    ${xf86vmode.out}/lib:${xi.out}/lib:${mesa}/lib:${mesanoglu}/lib:${xRandr.out}/lib:
    ${xInerama.out}/lib:${xProto.out}/lib:${xMu.out}/lib:${xFt.out}/lib:${xExt.out}/lib:${pkgs.xorg.libXxf86vm.out}/lib";
  };
}
