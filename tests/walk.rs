use std::error::Error;
use std::fs::{self, File};
use std::io::Write;

use tempdir::TempDir;

use diskus::{CountType, DiskUsage};

#[test]
fn size_of_files_in_nested_directories() -> Result<(), Box<dyn Error>> {
    let tmp_dir = TempDir::new("diskus-tests")?;

    // Create a 100-byte file at the root
    let file1_path = tmp_dir.path().join("file-100-byte");
    File::create(&file1_path)?.write_all(&[0u8; 100])?;

    // Create two nested directories and a 200-byte file inside
    let nested_dir = tmp_dir.path().join("dir1").join("dir2");
    fs::create_dir_all(&nested_dir)?;
    let file2_path = nested_dir.join("file-200-byte");
    File::create(&file2_path)?.write_all(&[0u8; 200])?;

    let root_directories = vec![tmp_dir.path().to_path_buf()];
    let (size_in_bytes, errors) = DiskUsage::new(&root_directories)
        .count_type(CountType::ApparentSize)
        .count();

    assert!(errors.is_empty());
    assert_eq!(size_in_bytes, 300);

    Ok(())
}
