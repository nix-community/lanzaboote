use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use goblin;

pub fn assemble(
    lanzaboote_bin: &Path,
    os_release: &Path,
    kernel_cmdline: &[String],
    kernel_path: &Path,
    initrd_path: &Path,
) -> Result<()> {
    // objcopy copies files into the PE binary. That's why we have to write the contents
    // of some bootspec properties to disk
    let kernel_cmdline_file = Path::new("/tmp/kernel_cmdline");
    fs::write(kernel_cmdline_file, kernel_cmdline.join(" "))?;
    let kernel_path_file = Path::new("/tmp/kernel_path");
    fs::write(kernel_path_file, kernel_path.to_str().unwrap())?;
    let initrd_path_file = Path::new("/tmp/initrd_path");
    fs::write(initrd_path_file, initrd_path.to_str().unwrap())?;

    let pe_binary = fs::read(lanzaboote_bin)?;
    let pe = goblin::pe::PE::parse(&pe_binary)?;

    let os_release_offs = u64::from(
        pe.sections
            .iter()
            .find(|s| s.name().unwrap() == ".sdmagic")
            .and_then(|s| Some(s.size_of_raw_data + s.virtual_address))
            .unwrap(),
    );

    let kernel_cmdline_offs = os_release_offs + file_size(os_release)?;
    let initrd_path_offs = kernel_cmdline_offs + file_size(kernel_cmdline_file)?;
    let kernel_path_offs = initrd_path_offs + file_size(initrd_path_file)?;

    let args = vec![
        String::from("--add-section"),
        format!(".osrel={}", path_to_string(os_release)),
        String::from("--change-section-vma"),
        format!(".osrel={:#x}", os_release_offs),
        String::from("--add-section"),
        format!(".cmdline={}", path_to_string(kernel_cmdline_file)),
        String::from("--change-section-vma"),
        format!(".cmdline={:#x}", kernel_cmdline_offs),
        String::from("--add-section"),
        format!(".initrdp={}", path_to_string(initrd_path_file)),
        String::from("--change-section-vma"),
        format!(".initrdp={:#x}", initrd_path_offs),
        String::from("--add-section"),
        format!(".kernelp={}", path_to_string(kernel_path_file)),
        String::from("--change-section-vma"),
        format!(".kernelp={:#x}", kernel_path_offs),
        lanzaboote_bin.to_str().unwrap().to_owned(),
        String::from("stub.efi"),
    ];

    let status = Command::new("objcopy").args(&args).status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("Failed to build stub with args `{:?}`", &args).into());
    }

    Ok(())
}

// All Linux file paths should be convertable to strings
fn path_to_string(path: &Path) -> String {
    path.to_owned().into_os_string().into_string().unwrap()
}

fn file_size(path: &Path) -> Result<u64> {
    Ok(fs::File::open(path)?.metadata()?.size())
}
