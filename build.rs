fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.ends_with("-apple-darwin") {
        println!("cargo:rustc-link-search=framework=/Library/Frameworks");
    }
}
