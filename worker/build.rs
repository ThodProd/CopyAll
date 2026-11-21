fn main() {
    println!("cargo:rerun-if-changed=../hub/assets/copyall.ico");
    embed_resource::compile("copyall.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed the CopyAll icon");
}
