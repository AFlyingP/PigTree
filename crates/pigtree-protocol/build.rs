use std::env;

fn main() {
    println!("cargo:rerun-if-changed=proto/session.proto");
    let protoc_path =
        protoc_bin_vendored::protoc_bin_path().expect("failed to locate vendored protoc binary");
    env::set_var("PROTOC", protoc_path);

    let mut config = prost_build::Config::new();
    config
        .compile_protos(&["proto/session.proto"], &["proto"])
        .expect("failed to compile protobuf definitions");
}
