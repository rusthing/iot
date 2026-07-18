use std::fs;
use std::path::Path;

/// # 该函数将配置文件从源位置复制到目标目录中
///
/// 主要用于构建过程中确保配置文件被正确地包含在输出目录里
///
/// ## Panics
/// - 当环境变量"OUT_DIR"不存在时会panic
/// - 当路径操作失败时会panic
/// - 当文件复制失败时会panic
fn main() {
    // println!("cargo:warning=🔍 正在运行 build.rs");
    // 获取项目根目录
    let project_root = env!("CARGO_MANIFEST_DIR");
    // 获取编译脚本文件的输出目录路径
    let out_dir = std::env::var("OUT_DIR").unwrap();
    // 构造目标文件路径，通过向上回溯OUT_DIR的父级目录来定位
    let dest_dir_path = Path::new(&out_dir)
        .ancestors()
        .nth(3)
        .expect("Failed to get parent directory");
    // println!(
    //     "cargo:warning=PROJECT_ROOT: {project_root:?} OUT_DIR: {out_dir} dest_path: {dest_dir_path:?}"
    // );

    // 复制应用的配置文件到输出目录
    let file_name = env!("CARGO_PKG_NAME");
    copy_file(project_root, file_name, "toml", dest_dir_path);
    copy_file(project_root, file_name, "yml", dest_dir_path);
    copy_file(project_root, file_name, "json", dest_dir_path);
    copy_file(project_root, file_name, "ini", dest_dir_path);
    copy_file(project_root, file_name, "ron", dest_dir_path);

    // 复制日志的配置文件到输出目录
    let file_name = "log";
    copy_file(project_root, file_name, "toml", dest_dir_path);
    copy_file(project_root, file_name, "yml", dest_dir_path);
    copy_file(project_root, file_name, "json", dest_dir_path);
    copy_file(project_root, file_name, "ini", dest_dir_path);
    copy_file(project_root, file_name, "ron", dest_dir_path);
}

fn copy_dir(project_root: &str, src_dir: &str, dest_dir_path: &Path) {
    let src_dir_path = Path::new(project_root).join(src_dir);
    if !src_dir_path.exists() || !src_dir_path.is_dir() {
        return;
    }

    let dest_dir = dest_dir_path.join(src_dir);
    copy_dir_recursive(&src_dir_path, &dest_dir);
}

fn copy_dir_recursive(src_path: &Path, dest_path: &Path) {
    if src_path.is_dir() {
        fs::create_dir_all(dest_path).expect("Failed to create directory");

        for entry in fs::read_dir(src_path).expect("Failed to read directory") {
            let entry = entry.expect("Failed to read directory entry");
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let dest_entry_path = dest_path.join(entry_name);

            if entry_path.is_dir() {
                copy_dir_recursive(&entry_path, &dest_entry_path);
            } else if entry_path.is_file() {
                // 告诉 Cargo 当文件变化时重新运行 build.rs
                println!("cargo:rerun-if-changed={}", entry_path.display());
                // 复制文件到目的地
                fs::copy(&entry_path, &dest_entry_path).expect("Failed to copy file");
            }
        }
    }
}

/// 复制指定扩展名的配置文件到输出目录
///
/// 该函数会查找与当前包同名的配置文件（如 `oss-svr.toml`），
/// 并将其从项目根目录复制到构建输出目录中。
///
/// # 参数
///
/// - `out_dir`: 构建输出目录路径
/// - `file_ext`: 配置文件的扩展名（如 "toml", "yml" 等）
///
/// # 行为
///
/// 1. 构造配置文件名：`{包名}.{扩展名}`
/// 2. 在项目根目录查找该配置文件
/// 3. 计算目标路径（输出目录向上三级目录）
/// 4. 如果源文件存在，则复制到目标路径
///
/// # Panics
///
/// - 当无法访问环境变量时
/// - 当路径操作失败时
/// - 当文件复制失败时
fn copy_file(project_root: &str, src_file_name: &str, src_file_ext: &str, dest_path: &Path) {
    // 获取源配置文件路径
    let src_file_name = format!("{src_file_name}.{src_file_ext}");

    let src_file_path = Path::new(project_root).join(&src_file_name);

    // 如果源文件存在，则执行复制操作
    if src_file_path.exists() {
        let dest_path = dest_path.join(&src_file_name);
        // println!("cargo:warning=copy {:?} to {:?}", src_file_path, dest_path);

        // 告诉 Cargo 当文件变化时重新运行 build.rs
        println!("cargo:rerun-if-changed={src_file_name}");
        // 复制文件到目的地
        fs::copy(src_file_path, dest_path).expect("Failed to copy app file");
    }
}
