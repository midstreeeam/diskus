use std::error::Error;
use std::fs::{self, File};
use std::io::Write;

use tempfile::tempdir;

use diskus::DiskUsage;

#[test]
fn size_of_files_in_nested_directories() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempdir()?;

    // Create a 100-byte file at the root
    let file1_path = tmp_dir.path().join("file-100-byte");
    File::create(&file1_path)?.write_all(&[0u8; 100])?;

    // Create two nested directories and a 200-byte file inside
    let nested_dir = tmp_dir.path().join("dir1").join("dir2");
    fs::create_dir_all(&nested_dir)?;
    let file2_path = nested_dir.join("file-200-byte");
    File::create(&file2_path)?.write_all(&[0u8; 200])?;

    let result = DiskUsage::new(&[tmp_dir]).apparent_size().count();

    assert_eq!(result.size_in_bytes().expect("no errors"), 300);

    Ok(())
}

#[test]
fn size_of_direct_children() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempdir()?;

    let file_path = tmp_dir.path().join("file-100-byte");
    File::create(&file_path)?.write_all(&[0u8; 100])?;

    let child_dir = tmp_dir.path().join("dir");
    fs::create_dir_all(child_dir.join("nested"))?;
    File::create(child_dir.join("nested").join("file-200-byte"))?.write_all(&[0u8; 200])?;

    let result = DiskUsage::new([tmp_dir.path()])
        .apparent_size()
        .count_direct_children();

    assert!(result.is_ok());

    let mut entries: Vec<_> = result
        .entries()
        .iter()
        .map(|entry| {
            (
                entry
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                entry.result().size_in_bytes().expect("entry has no errors"),
            )
        })
        .collect();
    entries.sort();

    assert_eq!(
        entries,
        vec![("dir".to_string(), 200), ("file-100-byte".to_string(), 100)]
    );

    Ok(())
}

#[cfg(not(windows))]
#[test]
fn direct_children_count_hardlinks_independently() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempdir()?;

    let first_dir = tmp_dir.path().join("first");
    let second_dir = tmp_dir.path().join("second");
    fs::create_dir_all(&first_dir)?;
    fs::create_dir_all(&second_dir)?;

    let original = first_dir.join("shared-100-byte");
    let linked = second_dir.join("shared-100-byte");
    File::create(&original)?.write_all(&[0u8; 100])?;
    fs::hard_link(&original, &linked)?;

    let result = DiskUsage::new([tmp_dir.path()])
        .apparent_size()
        .count_direct_children();

    assert!(result.is_ok());

    let mut entries: Vec<_> = result
        .entries()
        .iter()
        .map(|entry| {
            (
                entry
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                entry.result().size_in_bytes().expect("entry has no errors"),
            )
        })
        .collect();
    entries.sort();

    assert_eq!(
        entries,
        vec![("first".to_string(), 100), ("second".to_string(), 100)]
    );

    Ok(())
}

#[cfg(not(windows))]
#[test]
fn short_list_flag_outputs_direct_children() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempdir()?;

    File::create(tmp_dir.path().join("alpha-50-byte"))?.write_all(&[0u8; 50])?;
    File::create(tmp_dir.path().join("beta-70-byte"))?.write_all(&[0u8; 70])?;

    let child_dir = tmp_dir.path().join("dir");
    fs::create_dir_all(child_dir.join("nested"))?;
    File::create(child_dir.join("nested").join("gamma-200-byte"))?.write_all(&[0u8; 200])?;
    File::create(child_dir.join("delta-30-byte"))?.write_all(&[0u8; 30])?;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_diskus"))
        .args(["-b", "-l"])
        .arg(tmp_dir.path())
        .output()?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout)?;
    let lines: Vec<_> = stdout.lines().collect();

    assert_eq!(lines.len(), 6);
    assert!(lines[0].contains("Size"));
    assert!(lines[0].contains("Share"));
    assert!(lines[0].contains("Usage"));
    assert!(lines[0].contains("Path"));

    let dir = lines
        .iter()
        .find(|line| line.ends_with("/dir"))
        .expect("dir entry is shown");
    assert!(dir.contains("230 B"));
    assert!(dir.contains("65.7%"));

    let beta = lines
        .iter()
        .find(|line| line.ends_with("/beta-70-byte"))
        .expect("beta entry is shown");
    assert!(beta.contains("70 B"));
    assert!(beta.contains("20.0%"));

    let alpha = lines
        .iter()
        .find(|line| line.ends_with("/alpha-50-byte"))
        .expect("alpha entry is shown");
    assert!(alpha.contains("50 B"));
    assert!(alpha.contains("14.3%"));

    assert_eq!(lines[5], "Total size: 350 B (350 bytes)");

    Ok(())
}

#[cfg(not(windows))]
#[test]
fn ku_outputs_the_same_direct_child_chart() -> Result<(), Box<dyn Error>> {
    let tmp_dir = tempdir()?;

    File::create(tmp_dir.path().join("small-50-byte"))?.write_all(&[0u8; 50])?;
    File::create(tmp_dir.path().join("large-100-byte"))?.write_all(&[0u8; 100])?;

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ku"))
        .args(["-b"])
        .arg(tmp_dir.path())
        .output()?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let stdout = String::from_utf8(output.stdout)?;
    let lines: Vec<_> = stdout.lines().collect();

    assert_eq!(lines.len(), 5);

    let large = lines
        .iter()
        .find(|line| line.ends_with("/large-100-byte"))
        .expect("large entry is shown");
    assert!(large.contains("100 B"));
    assert!(large.contains("66.7%"));

    let small = lines
        .iter()
        .find(|line| line.ends_with("/small-50-byte"))
        .expect("small entry is shown");
    assert!(small.contains("50 B"));
    assert!(small.contains("33.3%"));

    assert_eq!(lines[4], "Total size: 150 B (150 bytes)");

    Ok(())
}
