use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn collect(dir: &Path, prefix: &str, out: &mut Vec<(PathBuf, String)>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = format!("{prefix}/{name}");
            if path.is_dir() {
                collect(&path, &rel, out);
            } else {
                out.push((path, rel));
            }
        }
    }
}

fn main() {
    tauri_build::build();

    // 把 PyInstaller 打好的后端目录压进 exe，安装目录就只剩一个 exe
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let backend = manifest.join("..").join("dist").join("TranslatorBotBackend");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let zip_path = out_dir.join("backend.zip");

    println!("cargo:rerun-if-changed={}", backend.display());
    if backend.is_dir() {
        let mut files = Vec::new();
        collect(&backend, "backend", &mut files);
        let file = File::create(&zip_path).expect("create backend.zip");
        let mut zw = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (path, rel) in files {
            zw.start_file(rel, options).expect("start zip entry");
            let mut f = File::open(&path).expect("open backend file");
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).expect("read backend file");
            zw.write_all(&buf).expect("write zip entry");
        }
        zw.finish().expect("finish backend.zip");
    } else if !zip_path.exists() {
        // 没有后端产物时放一个空包，保证 cargo check 能过
        let file = File::create(&zip_path).expect("create empty backend.zip");
        let mut zw = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        let _ = zw.start_file("backend/.keep", options);
        let _ = zw.finish();
    }
}
