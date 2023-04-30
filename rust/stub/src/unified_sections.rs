use goblin::pe::{section_table::SectionTable, PE};

use crate::pe_section::pe_section_data;

/// List of PE sections that have a special meaning with respect to
/// UKI specification.
/// This is the canonical order in which they are measured into TPM
/// PCR 11.
/// !!! DO NOT REORDER !!!
pub enum UnifiedSection<'a> {
    Linux,
    OsRel,
    CmdLine,
    Initrd,
    Splash,
    DTB,
    // We only need to store the data for those for now,
    // because we need to pack them as CPIOs.
    PcrSig(&'a [u8]),
    PcrPkey(&'a [u8])
}

impl<'a> UnifiedSection<'a> {
    /// Whether this section should be measured into TPM.
    pub fn should_be_measured(&self) -> bool {
        match self {
            UnifiedSection::PcrSig(_) => false,
            _ => true
        }
    }

    pub fn from_section_table(pe: &'a [u8], section: &SectionTable) -> uefi::Result<Self> {
        if let Some(data) = pe_section_data(pe, &section) {
            Ok(match section.name().unwrap() {
                ".linux" => Self::Linux,
                ".osrel" => Self::OsRel,
                ".cmdline" => Self::CmdLine,
                ".initrd" => Self::Initrd,
                ".splash" => Self::Splash,
                ".dtb" => Self::DTB,
                ".pcrsig" => Self::PcrSig(data),
                ".pcrpkey" => Self::PcrPkey(data),
                _ => return Err(uefi::Status::INVALID_PARAMETER.into())
            })
        } else {
            // No data in the section is equivalent to missing section.
            Err(uefi::Status::INVALID_PARAMETER.into())
        }
    }

}
