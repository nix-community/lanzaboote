use std::fs;

use anyhow::{Context, Result};
use expect_test::{expect, Expect};
use tempfile::tempdir;

use crate::common;

#[test]
fn generate_expected_os_release() -> Result<()> {
    let esp_mountpoint = tempdir()?;
    let tmpdir = tempdir()?;
    let profiles = tempdir()?;
    let toplevel = common::setup_toplevel(tmpdir.path())?;

    let gen_exp_os_rel = |prof: Option<String>, ver: u64, exp: Expect| -> Result<()> {
        let generation_link = common::setup_generation_link_from_toplevel(
            &toplevel,
            profiles.path(),
            ver,
            prof.clone(),
        )
        .expect("Failed to setup generation link");

        let output0 = common::lanzaboote_install(0, esp_mountpoint.path(), vec![generation_link])?;
        assert!(output0.status.success());

        let stub_data = fs::read(common::image_path(&esp_mountpoint, ver, prof, &toplevel)?)?;
        let os_release_section = pe_section(&stub_data, ".osrel")
            .context("Failed to read .osrelease PE section.")?
            .to_owned();

        exp.assert_eq(&String::from_utf8(os_release_section)?);

        Ok(())
    };

    // Without profile
    let expected = expect![[r#"
        ID=lanzaboote
        PRETTY_NAME=LanzaOS (Generation 1, 1970-01-01)
        VERSION_ID=Generation 1, 1970-01-01
    "#]];
    let _ = gen_exp_os_rel(None, 1u64, expected);

    // With profile
    let expected_prof = expect![[r#"
        ID=lanzaboote
        PRETTY_NAME=LanzaOS [My W#@cky_Yet_L3g!t profile-name -3] (Generation 1, 1970-01-01)
        VERSION_ID=Generation 1, 1970-01-01
    "#]];
    let _ = gen_exp_os_rel(
        Some("My W#@cky_Yet_L3g!t profile-name -3".to_string()),
        1u64,
        expected_prof,
    );

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
