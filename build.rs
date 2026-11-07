fn main() {
    println!("cargo:rerun-if-changed=assets/wholocks.rc");
    println!("cargo:rerun-if-changed=assets/wholocks.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("assets/wholocks.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to embed the WhoLocks icon");
    }
}
