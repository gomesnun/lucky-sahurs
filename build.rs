// Compiles the vendored SDL_ttf 2.24 (the same version pygame-ce ships) with HarfBuzz,
// so text metrics and glyph rasterization match the original game exactly.
//
// Release builds (LV_STATIC=1, see .github/workflows/build.yml) link everything statically so the
// game is one self-contained file, like the PyInstaller --onefile build it replaces:
//   LV_SDL2_PREFIX  SDL2 installed by cmake (include/SDL2 + lib with the static library)
//   LV_FT_SRC       FreeType source tree, compiled here
//   LV_HB_SRC       HarfBuzz source tree, compiled here (amalgamated harfbuzz.cc)
// (cargo also needs RUSTFLAGS="-L native=$LV_SDL2_PREFIX/lib" so the sdl2-sys crate finds SDL2.)
// Without LV_STATIC the system libraries are used through pkg-config.
//
// The icons/, sounds/ and fonts/ folders are embedded into the executable (src/assets.rs).
use std::path::{Path, PathBuf};

fn env_path(name: &str) -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed={name}");
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn main() {
    println!("cargo:rerun-if-changed=vendor/sdl_ttf/SDL_ttf.c");
    println!("cargo:rerun-if-changed=vendor/sdl_ttf/SDL_ttf.h");
    println!("cargo:rerun-if-env-changed=LV_BUILD_VERSION");
    embed_assets();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let is_static = env_path("LV_STATIC").is_some();

    let mut build = cc::Build::new();
    build
        .file("vendor/sdl_ttf/SDL_ttf.c")
        .include("vendor/sdl_ttf")
        .define("TTF_USE_HARFBUZZ", "1")
        .opt_level(2)
        .warnings(false);
    if is_static {
        let sdl = env_path("LV_SDL2_PREFIX").expect("LV_SDL2_PREFIX is required with LV_STATIC");
        let ft = env_path("LV_FT_SRC").expect("LV_FT_SRC is required with LV_STATIC");
        let hb = env_path("LV_HB_SRC").expect("LV_HB_SRC is required with LV_STATIC");
        build.include(sdl.join("include").join("SDL2")).include(ft.join("include")).include(hb.join("src"));
        build.compile("sdl2_ttf_vendored");
        build_harfbuzz(&hb, &ft);
        build_freetype(&ft);
        println!("cargo:rustc-link-search=native={}", sdl.join("lib").display());
        if target_os == "linux" {
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=dl");
            println!("cargo:rustc-link-lib=dylib=pthread");
        }
        if target_os == "macos" {
            for fw in ["CoreFoundation", "AppKit", "QuartzCore"] {
                println!("cargo:rustc-link-lib=framework={fw}");
            }
            // SDL's @available checks call ___isPlatformVersionAtLeast, which lives in clang's runtime
            // library; rustc doesn't link it on its own
            let out = std::process::Command::new("clang").arg("-print-resource-dir").output().expect("clang is required");
            let rt = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()).join("lib").join("darwin");
            println!("cargo:rustc-link-search=native={}", rt.display());
            println!("cargo:rustc-link-lib=static=clang_rt.osx");
        }
        if target_os == "windows" {
            for lib in ["advapi32", "shlwapi", "cfgmgr32"] {
                println!("cargo:rustc-link-lib={lib}");
            }
            // the game icon on the .exe (the PyInstaller build used --icon icons/verity.ico)
            #[cfg(windows)]
            {
                let ico = assets_dir().join("icons").join("verity.ico");
                let mut res = winresource::WindowsResource::new();
                res.set_icon(ico.to_str().unwrap());
                res.compile().expect("could not embed the .exe icon");
            }
        }
    } else {
        for lib in ["sdl2", "freetype2", "harfbuzz"] {
            let out = std::process::Command::new("pkg-config")
                .args(["--cflags-only-I", lib])
                .output()
                .expect("pkg-config is required");
            for flag in String::from_utf8_lossy(&out.stdout).split_whitespace() {
                if let Some(path) = flag.strip_prefix("-I") {
                    build.include(path);
                }
            }
        }
        build.compile("sdl2_ttf_vendored");
        println!("cargo:rustc-link-lib=freetype");
        println!("cargo:rustc-link-lib=harfbuzz");
    }
}

/// The folder with icons/ sounds/ fonts/: the repository root (shared with python/ and the
/// website), or assets/ next to Cargo.toml.
fn assets_dir() -> PathBuf {
    if let Some(p) = env_path("LV_ASSETS_DIR") {
        return p;
    }
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let own = manifest.join("assets");
    if own.join("icons").is_dir() { own } else { manifest }
}

fn collect(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let path = e.path();
        let r = format!("{rel}/{name}");
        if path.is_dir() {
            collect(&path, &r, out);
        } else {
            out.push((r, path));
        }
    }
}

fn embed_assets() {
    let root = assets_dir();
    let mut files = Vec::new();
    for folder in ["icons", "sounds", "fonts"] {
        let dir = root.join(folder);
        assert!(dir.is_dir(), "assets folder not found: {}", dir.display());
        println!("cargo:rerun-if-changed={}", dir.display());
        collect(&dir, folder, &mut files);
    }
    files.retain(|(r, _)| {
        let l = r.to_lowercase();
        [".png", ".ogg", ".ttf", ".otf", ".lvm"].iter().any(|e| l.ends_with(e))
    });
    files.sort();
    let mut code = String::from("pub static EMBEDDED: &[(&str, &[u8])] = &[\n");
    for (rel, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
        code.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", rel, path.canonicalize().unwrap()));
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded_assets.rs");
    std::fs::write(out, code).unwrap();
}

/// FreeType with its default module set (include/freetype/config/ftmodule.h).
fn build_freetype(ft: &Path) {
    let files = [
        "src/autofit/autofit.c",
        "src/base/ftbase.c",
        "src/base/ftbbox.c",
        "src/base/ftbdf.c",
        "src/base/ftbitmap.c",
        "src/base/ftcid.c",
        "src/base/ftdebug.c",
        "src/base/ftfstype.c",
        "src/base/ftgasp.c",
        "src/base/ftglyph.c",
        "src/base/ftgxval.c",
        "src/base/ftinit.c",
        "src/base/ftmm.c",
        "src/base/ftotval.c",
        "src/base/ftpatent.c",
        "src/base/ftpfr.c",
        "src/base/ftstroke.c",
        "src/base/ftsynth.c",
        "src/base/ftsystem.c",
        "src/base/fttype1.c",
        "src/base/ftwinfnt.c",
        "src/bdf/bdf.c",
        "src/cache/ftcache.c",
        "src/cff/cff.c",
        "src/cid/type1cid.c",
        "src/gzip/ftgzip.c",
        "src/lzw/ftlzw.c",
        "src/pcf/pcf.c",
        "src/pfr/pfr.c",
        "src/psaux/psaux.c",
        "src/pshinter/pshinter.c",
        "src/psnames/psnames.c",
        "src/raster/raster.c",
        "src/sdf/sdf.c",
        "src/sfnt/sfnt.c",
        "src/smooth/smooth.c",
        "src/svg/svg.c",
        "src/truetype/truetype.c",
        "src/type1/type1.c",
        "src/type42/type42.c",
        "src/winfonts/winfnt.c",
    ];
    let mut b = cc::Build::new();
    b.include(ft.join("include")).define("FT2_BUILD_LIBRARY", None).opt_level(2).warnings(false);
    for f in files {
        b.file(ft.join(f));
    }
    b.compile("freetype_vendored");
}

fn build_harfbuzz(hb: &Path, ft: &Path) {
    let mut b = cc::Build::new();
    b.cpp(true)
        .file(hb.join("src").join("harfbuzz.cc"))
        .include(hb.join("src"))
        .include(ft.join("include"))
        .define("HAVE_FREETYPE", "1")
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17")
        .flag_if_supported("-fno-exceptions")
        .flag_if_supported("-fno-rtti")
        .opt_level(2)
        .warnings(false);
    b.compile("harfbuzz_vendored");
}
