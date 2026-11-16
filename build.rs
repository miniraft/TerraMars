use std::{env, fs::File, path::PathBuf};

fn main() {
    let source = "assets/icons/app_logo/logo_small.png";
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let logo = image::open(source).expect("Valid app logo");
    let frames: Vec<_> = [16, 24, 32, 48, 64, 128, 256]
        .into_iter()
        .map(|size| {
            let pixels = logo
                .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                .into_rgba8();
            image::codecs::ico::IcoFrame::as_png(
                pixels.as_raw(),
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
            .expect("Encode app icon frame")
        })
        .collect();
    let icon = output.join("terramars.ico");
    image::codecs::ico::IcoEncoder::new(File::create(&icon).expect("Create Windows icon"))
        .encode_images(&frames)
        .expect("Encode Windows icon");

    // Version information identifies TerraMars in Explorer and in the Windows
    // microphone privacy list, where unpackaged desktop apps appear by name.
    let version = env::var("CARGO_PKG_VERSION").expect("Cargo package version");
    let numeric: Vec<&str> = version.split(['.', '-', '+']).collect();
    let field = |index: usize| {
        numeric
            .get(index)
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(0)
    };
    let numbers = format!("{},{},{},0", field(0), field(1), field(2));
    let some_path = |value: &PathBuf| value.display().to_string().replace('\\', "/");

    // GPUI loads the executable's icon resource with ID 1.
    let resource = output.join("terramars.rc");
    std::fs::write(
        &resource,
        format!(
            "1 ICON \"{icon}\"\n\
             \n\
             1 VERSIONINFO\n\
             FILEVERSION {numbers}\n\
             PRODUCTVERSION {numbers}\n\
             FILEOS 0x40004L\n\
             FILETYPE 0x1L\n\
             {{\n\
               BLOCK \"StringFileInfo\"\n\
               {{\n\
                 BLOCK \"040904B0\"\n\
                 {{\n\
                   VALUE \"CompanyName\", \"TerraMars\"\n\
                   VALUE \"FileDescription\", \"TerraMars Mars surface explorer\"\n\
                   VALUE \"FileVersion\", \"{version}\"\n\
                   VALUE \"InternalName\", \"terramars\"\n\
                   VALUE \"OriginalFilename\", \"TerraMars.exe\"\n\
                   VALUE \"ProductName\", \"TerraMars\"\n\
                   VALUE \"ProductVersion\", \"{version}\"\n\
                 }}\n\
               }}\n\
               BLOCK \"VarFileInfo\"\n\
               {{\n\
                 VALUE \"Translation\", 0x409, 1200\n\
               }}\n\
             }}\n",
            icon = some_path(&icon),
            numbers = numbers,
            version = version,
        ),
    )
    .expect("Write Windows icon and version resources");
    embed_resource::compile_for(&resource, ["terramars"], embed_resource::NONE)
        .manifest_required()
        .expect("Compile Windows icon resource");
}
