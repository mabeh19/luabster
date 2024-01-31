
fn main() {
    cc::Build::new()
        .file("src/signals.c")
        .compile("sig");
    println!("cargo:rerun-if-changed=src/signals.c");
}
