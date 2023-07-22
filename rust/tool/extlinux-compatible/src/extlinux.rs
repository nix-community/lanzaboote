use std::{fmt::Display, path::PathBuf};

use lanzaboote_tool::generation::Generation;

pub type ExtlinuxConfig = Vec<ExtlinuxEntry>;

pub struct ExtlinuxEntry {
    label: String,
    menu_label: String,
    kernel: PathBuf,
    initrd: Option<PathBuf>,
    extra_kernel_params: Option<String>,
    device_tree_file: Option<String>,
    device_tree_dir: Option<String>,
}

impl Display for ExtlinuxEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("LABEL {}", self.label))?;
        f.write_fmt(format_args!("MENU LABEL {}", self.menu_label))?;
        f.write_fmt(format_args!("LINUX {}", self.kernel.display()))?;
        if let Some(initrd) = &self.initrd {
            f.write_fmt(format_args!("INITRD {}", initrd.display()))?;
        }
        if let Some(extra_kernel_params) = &self.extra_kernel_params {
            f.write_fmt(format_args!("APPEND {}", extra_kernel_params))?;
        }
        if let Some(fdt) = &self.device_tree_file {
            f.write_fmt(format_args!("FDT {}", fdt))?;
        }
        if let Some(fdt_dir) = &self.device_tree_dir {
            f.write_fmt(format_args!("FDTDIR {}", fdt_dir))?;
        }
        Ok(())
    }
}

impl From<Generation> for ExtlinuxEntry {
    fn from(value: Generation) -> Self {
        ExtlinuxEntry {
            label: format!("nixos-{}", value.to_string()),
            // TODO: how to introduce version of NixOS here? read in the bootspec
            menu_label: format!("NixOS {}", value.describe()),
            kernel: value.spec.bootspec.bootspec.kernel,
            initrd: value.spec.bootspec.bootspec.initrd,
            extra_kernel_params: Some(value.spec.bootspec.bootspec.kernel_params.join(" ")),
            // TODO: for bootspec v2
            device_tree_file: None,
            device_tree_dir: None
        }
    }
}
