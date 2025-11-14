/// List of PE sections that have a special meaning with respect to the UKI specification.
///
/// The declaration order of these enum variants defines the canonical order for PCR 11 measurements.
/// Per the [UKI spec](https://uapi-group.org/specifications/specs/unified_kernel_image/#uki-tpm-pcr-measurements):
/// "shall measure the sections listed above, starting from the .linux section, in the order as listed
/// (which should be considered the canonical order)."
///
/// !!! DO NOT REORDER !!!
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum UnifiedSection {
    Linux = 0,
    OsRel = 1,
    CmdLine = 2,
    Initrd = 3,
    Ucode = 4,
    Splash = 5,
    Dtb = 6,
    DtbAuto = 7,
    Hwids = 8,
    Uname = 9,
    Sbat = 10,
    PcrSig = 11,
    PcrPkey = 12,
}

impl TryFrom<&str> for UnifiedSection {
    type Error = uefi::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            ".linux" => Self::Linux,
            ".osrel" => Self::OsRel,
            ".cmdline" => Self::CmdLine,
            ".initrd" => Self::Initrd,
            ".ucode" => Self::Ucode,
            ".splash" => Self::Splash,
            ".dtb" => Self::Dtb,
            ".dtbauto" => Self::DtbAuto,
            ".hwids" => Self::Hwids,
            ".uname" => Self::Uname,
            ".sbat" => Self::Sbat,
            ".pcrsig" => Self::PcrSig,
            ".pcrpkey" => Self::PcrPkey,
            _ => return Err(uefi::Status::INVALID_PARAMETER.into()),
        })
    }
}

impl UnifiedSection {
    /// Whether this section should be measured into TPM.
    pub fn should_be_measured(&self) -> bool {
        // .pcrsig is never measured per spec
        //
        // .dtbauto requires hardware matching logic during PE section parsing to select
        // which .dtbauto section matches the current hardware. Since lanzaboote doesn't
        // implement this selection logic, .dtbauto sections are not measured.
        //
        // Additionally, lanzaboote doesn't implement devicetree loading at all, making this moot.
        // Note: Measuring hardware-dependent state into PCR 11 is questionable design, as it
        // breaks the predictability of PCR values. See discussion at:
        // https://github.com/uapi-group/specifications/issues/182
        !matches!(self, UnifiedSection::PcrSig | UnifiedSection::DtbAuto)
    }

    /// Returns the PE section name for this unified section.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linux => ".linux",
            Self::OsRel => ".osrel",
            Self::CmdLine => ".cmdline",
            Self::Initrd => ".initrd",
            Self::Ucode => ".ucode",
            Self::Splash => ".splash",
            Self::Dtb => ".dtb",
            Self::DtbAuto => ".dtbauto",
            Self::Hwids => ".hwids",
            Self::Uname => ".uname",
            Self::Sbat => ".sbat",
            Self::PcrSig => ".pcrsig",
            Self::PcrPkey => ".pcrpkey",
        }
    }
}
