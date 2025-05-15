use std::fs;

use anyhow::{Context, Result};
use expect_test::expect;
use tempfile::tempdir;

use crate::common;

#[test]
fn generate_expected_os_release() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let boot_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let generation_link =
        common::setup_generation_link_from_toplevel(&toplevel, profiles.path(), 1)
            .expect("Failed to setup generation link");

    let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), boot_mountpoint.path(), vec![generation_link])?;
    assert!(output0.status.success());

    let stub_data = fs::read(common::image_path(&esp_mountpoint, 1, &toplevel)?)?;
    let os_release_section = pe_section(&stub_data, ".osrel")
        .context("Failed to read .osrelease PE section.")?
        .to_owned();

    let expected = expect![[r#"
        ID=lanzaboote
        PRETTY_NAME=LanzaOS (Generation 1, 1970-01-01)
        VERSION_ID=Generation 1, 1970-01-01
    "#]];

    expected.assert_eq(&String::from_utf8(os_release_section)?);

    Ok(())
}

fn pe_section<'a>(file_data: &'a [u8], section_name: &str) -> Option<&'a [u8]> {
    let pe_binary = goblin::pe::PE::parse(file_data).ok()?;

    pe_binary
        .sections
        .iter()
        .find(|s| s.name().unwrap() == section_name)
        .and_then(|s| {
            let section_start: usize = s.pointer_to_raw_data.try_into().ok()?;
            assert!(s.virtual_size <= s.size_of_raw_data);
            let section_end: usize = section_start + usize::try_from(s.virtual_size).ok()?;
            Some(&file_data[section_start..section_end])
        })
}
