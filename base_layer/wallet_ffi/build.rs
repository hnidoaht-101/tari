// Copyright 2022. The Tari Project
// SPDX-License-Identifier: BSD-3-Clause

use std::{env, path::PathBuf};

use cbindgen::{Config, ExportConfig, Language, LineEndingStyle, ParseConfig, Style};
use tari_common::build::StaticApplicationInfo;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    // generate version info
    let gen = StaticApplicationInfo::initialize().unwrap();
    gen.write_consts_to_outdir("consts.rs").unwrap();

    let output_file = PathBuf::from(&crate_dir).join("wallet.h").display().to_string();

    let config = Config {
        language: Language::C,
        header: Some("// Copyright 2022. The Tari Project\n// SPDX-License-Identifier: BSD-3-Clause".to_string()),
        parse: ParseConfig {
            parse_deps: true,
            include: Some(vec![
                "tari_core".to_string(),
                "tari_common_types".to_string(),
                "tari_crypto".to_string(),
                "tari_p2p".to_string(),
                "minotari_wallet".to_string(),
                "tari_contacts".to_string(),
            ]),
            ..Default::default()
        },
        autogen_warning: Some("// This file was generated by cargo-bindgen. Please do not edit manually.".to_string()),
        style: Style::Tag,
        cpp_compat: true,
        export: ExportConfig {
            include: vec!["TariUtxo".to_string()],
            ..Default::default()
        },
        line_endings: LineEndingStyle::Native,
        ..Default::default()
    };

    cbindgen::generate_with_config(&crate_dir, config)
        .unwrap()
        .write_to_file(output_file);
}
