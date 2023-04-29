/// List of PE sections that have a special meaning with respect to
/// UKI specification.
/// This is the canonical order in which they are measured into TPM
/// PCR 11.
/// !!! DO NOT REORDER !!!
pub enum UnifiedSection {
    Linux,
    OsRel,
    CmdLine,
    Initrd,
    Splash,
    DTB,
    PcrSig,
    PcrPkey
}

impl TryFrom<&str> for UnifiedSection {
    type Error = uefi::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            ".linux" => Self::Linux,
            ".osrel" => Self::OsRel,
            ".cmdline" => Self::CmdLine,
            ".initrd" => Self::Initrd,
            ".splash" => Self::Splash,
            ".dtb" => Self::DTB,
            ".pcrsig" => Self::PcrSig,
            ".pcrpkey" => Self::PcrPkey,
            _ => return Err(uefi::Status::INVALID_PARAMETER.into())
        })
    }
}

impl UnifiedSection {
    /// Whether this section should be measured into TPM.
    pub fn should_be_measured(&self) -> bool {
        match self {
            UnifiedSection::PcrSig => false,
            _ => true
        }
    }
}
