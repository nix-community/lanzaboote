use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use tempfile::tempdir;

use crate::common::read_genlist;

#[test]
fn test_read_genlist() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a test genlist.json file with multiple entries
    let mut file = File::create(&genlist_path)?;
    writeln!(
        file,
        "{{\"version\":1,\"build_time\":1000,\"outpath\":\"/nix/store/abc123\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":2,\"build_time\":2000,\"outpath\":\"/nix/store/def456\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":3,\"outpath\":\"/nix/store/ghi789\"}}"
    )?;
    
    // Read the genlist
    let entries = read_genlist(&genlist_path)?;
    
    // Verify we got 3 entries
    assert_eq!(entries.len(), 3);
    
    // Verify the entries
    assert_eq!(entries[0].version, 1);
    assert_eq!(entries[0].outpath, PathBuf::from("/nix/store/abc123"));
    
    assert_eq!(entries[1].version, 2);
    assert_eq!(entries[1].outpath, PathBuf::from("/nix/store/def456"));
    
    assert_eq!(entries[2].version, 3);
    assert_eq!(entries[2].outpath, PathBuf::from("/nix/store/ghi789"));
    
    Ok(())
}

#[test]
fn test_read_empty_genlist() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create an empty genlist.json file
    File::create(&genlist_path)?;
    
    // Read the genlist
    let entries = read_genlist(&genlist_path)?;
    
    // Verify we got 0 entries
    assert_eq!(entries.len(), 0);
    
    Ok(())
}

#[test]
fn test_read_invalid_genlist() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a genlist.json file with invalid JSON
    let mut file = File::create(&genlist_path)?;
    writeln!(file, "{{\"invalid\": json")?;
    
    // Reading should fail
    let result = read_genlist(&genlist_path);
    assert!(result.is_err());
    
    Ok(())
}



#[test]
fn test_genlist_duplicate_versions_returns_error() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a test genlist.json file with duplicate version 1
    let mut file = File::create(&genlist_path)?;
    writeln!(
        file,
        "{{\"version\":1,\"build_time\":1000,\"outpath\":\"/nix/store/first\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":2,\"build_time\":2000,\"outpath\":\"/nix/store/second\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":1,\"outpath\":\"/nix/store/duplicate\"}}"
    )?;
    
    // Read the genlist - should fail because of duplicate version
    let result = read_genlist(&genlist_path);
    assert!(result.is_err(), "Should return error for duplicate versions");
    
    Ok(())
}

#[test]
fn test_genlist_with_optional_build_time() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a test genlist.json file with optional build_time
    let mut file = File::create(&genlist_path)?;
    writeln!(
        file,
        "{{\"version\":1,\"build_time\":1000,\"outpath\":\"/nix/store/first\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":2,\"outpath\":\"/nix/store/second\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":3,\"build_time\":3000,\"outpath\":\"/nix/store/third\"}}"
    )?;
    
    // Read the genlist - should succeed with optional build_time
    let entries = read_genlist(&genlist_path)?;
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].build_time, Some(1000));
    assert_eq!(entries[1].build_time, None);
    assert_eq!(entries[2].build_time, Some(3000));
    
    Ok(())
}

#[test]
fn test_genlist_label_with_newline_returns_error() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a test genlist.json file with a label containing newline
    let mut file = File::create(&genlist_path)?;
    writeln!(
        file,
        "{{\"version\":1,\"build_time\":1000,\"outpath\":\"/nix/store/first\",\"label\":\"test\\nlabel\"}}"
    )?;
    
    // Read the genlist - should fail because label contains newline
    let result = read_genlist(&genlist_path);
    assert!(result.is_err(), "Should return error for label with newline");
    
    Ok(())
}

#[test]
fn test_genlist_ordered_by_version() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a test genlist.json file with entries ordered from smallest to highest version
    let mut file = File::create(&genlist_path)?;
    writeln!(
        file,
        "{{\"version\":1,\"build_time\":1000,\"outpath\":\"/nix/store/first\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":3,\"build_time\":3000,\"outpath\":\"/nix/store/third\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":5,\"build_time\":5000,\"outpath\":\"/nix/store/fifth\"}}"
    )?;
    
    // Read the genlist
    let entries = read_genlist(&genlist_path)?;
    
    // Verify the entries are in order from smallest to highest version
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].version, 1);
    assert_eq!(entries[1].version, 3);
    assert_eq!(entries[2].version, 5);
    
    Ok(())
}

#[test]
fn test_genlist_unordered_versions_returns_error() -> Result<()> {
    let tmpdir = tempdir()?;
    let genlist_path = tmpdir.path().join("genlist.json");
    
    // Create a test genlist.json file with entries NOT ordered by version
    let mut file = File::create(&genlist_path)?;
    writeln!(
        file,
        "{{\"version\":5,\"build_time\":5000,\"outpath\":\"/nix/store/fifth\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":1,\"build_time\":1000,\"outpath\":\"/nix/store/first\"}}"
    )?;
    writeln!(
        file,
        "{{\"version\":3,\"build_time\":3000,\"outpath\":\"/nix/store/third\"}}"
    )?;
    
    // Read the genlist - should fail because entries are not ordered by version
    let result = read_genlist(&genlist_path);
    assert!(result.is_err(), "Should return error for unordered versions");
    
    Ok(())
}
