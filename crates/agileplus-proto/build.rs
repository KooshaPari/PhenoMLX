// SPDX-License-Identifier: MIT OR Apache-2.0
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Declare the custom cfg flag so rustc doesn't warn about it.
    println!("cargo::rustc-check-cfg=cfg(agileplus_proto_stubs)");

    // Skip proto compilation when protoc is unavailable (e.g. CI check-only runs).
    if std::env::var("SKIP_PROTO_BUILD").is_ok() || which_protoc().is_none() {
        if std::env::var("SKIP_PROTO_BUILD").is_ok() {
            println!("cargo:warning=SKIP_PROTO_BUILD set — skipping protoc codegen");
        } else {
            println!(
                "cargo:warning=protoc not found on PATH — using hand-written stubs. \
                Install protobuf-compiler or set PROTOC env var for a full build."
            );
        }
        // Signal to lib.rs that we are in stub mode.
        println!("cargo:rustc-cfg=agileplus_proto_stubs");
        return Ok(());
    }

    let protos = &[
        "../../agileplus-agents/proto/agileplus/v1/common.proto",
        "../../agileplus-agents/proto/agileplus/v1/core.proto",
        "../../agileplus-agents/proto/agileplus/v1/agents.proto",
        "../../agileplus-agents/proto/agileplus/v1/integrations.proto",
        "../../agileplus-agents/proto/agileplus/v1/work_items.proto",
    ];
    let includes = &["../../agileplus-agents/proto"];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(protos, includes)?;

    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }

    Ok(())
}

fn which_protoc() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("PROTOC") {
        let path = std::path::PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let name = if cfg!(windows) {
        "protoc.exe"
    } else {
        "protoc"
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let candidate = dir.join(name);
            candidate.is_file().then_some(candidate)
        })
    })
}
