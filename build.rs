
fn main() {
    cc::Build::new()
        .file("src/signals.c")
        .file("src/lua_runner.c")
        .compile("sig");
    println!("cargo:rerun-if-changed=src/signals.c");
    println!("cargo:rerun-if-changed=src/lua_runner.c");
}
