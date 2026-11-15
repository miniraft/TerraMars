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

    // GPUI loads the executable's icon resource with ID 1.
    let resource = output.join("terramars.rc");
    std::fs::write(
        &resource,
        format!(
            "1 ICON \"{}\"\n",
            icon.display().to_string().replace('\\', "/")
        ),
    )
    .expect("Write Windows icon resource");
    embed_resource::compile_for(&resource, ["terramars"], embed_resource::NONE)
        .manifest_required()
        .expect("Compile Windows icon resource");
}
