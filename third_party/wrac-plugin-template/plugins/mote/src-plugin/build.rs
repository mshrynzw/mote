//! Build script that generates host-visible plugin descriptors and bundles
//! the design-mock screen into a single zip for release builds.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use wrac_build::{
    FrontendBundleConfig, PluginDescriptorCodegenConfig, build_frontend_bundle,
    generate_plugin_descriptors,
};

fn main() {
    generate_plugin_descriptors(PluginDescriptorCodegenConfig::default())
        .expect("failed to generate WRAC plugin descriptors");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let gui_dir = prepare_screen(&manifest_dir).expect("failed to prepare the Mote screen");
    build_frontend_bundle(FrontendBundleConfig {
        dist_dir: gui_dir,
        output_file_name: "wrac_gain_plugin_gui.zip",
        rerun_if_changed: &[
            "../src-gui/bridge.js",
            "../../../../../design-mock/main/index.html",
        ],
        missing_dist_build_command: "The design-mock screen was not written.",
    })
    .expect("failed to create frontend zip");
}

/// 設計図の Main に、Filter 2 基目を隠す指定と、ノブを音へ渡すスクリプトを足す。
fn prepare_screen(manifest_dir: &Path) -> std::io::Result<PathBuf> {
    let design = manifest_dir.join("../../../../../design-mock/main/index.html");
    let bridge = manifest_dir.join("../src-gui/bridge.js");
    println!("cargo:rerun-if-changed={}", design.display());
    println!("cargo:rerun-if-changed={}", bridge.display());

    let page = fs::read_to_string(&design)?;
    let script = fs::read_to_string(&bridge)?;
    let hide = "<style>.post > .pair:last-of-type { display: none !important; }</style>\n";
    let page = page.replacen("</head>", &format!("{hide}</head>"), 1);
    let page = page.replacen(
        "</body>",
        &format!("<script>\n{script}\n</script>\n</body>"),
        1,
    );

    let app_dir = manifest_dir.join("../src-gui/app");
    fs::create_dir_all(&app_dir)?;
    fs::write(app_dir.join("index.html"), &page)?;

    let mock_dir = manifest_dir.join("../src-gui/mock");
    fs::create_dir_all(&mock_dir)?;
    fs::write(mock_dir.join("index.html"), &page)?;
    Ok(app_dir)
}
