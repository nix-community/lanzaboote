use std::fmt::Display;

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum UnsignedGenerationsPolicy {
    ResignEverything,
    ResignPreviousGenerationOnly,
    IgnoreEverything
}

pub struct LanzabootPolicy {
    unsigned_generations_policy: UnsignedGenerationsPolicy,
}

impl Display for UnsignedGenerationsPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResignEverything => write!(f, "resign everything policy"),
            Self::ResignPreviousGenerationOnly => write!(f, "resign only the previous generation policy"),
            Self::IgnoreEverything => write!(f, "ignore everything policy"),
        }
    }
}

impl TryFrom<String> for UnsignedGenerationsPolicy {
    fn try_from(value: String) {
        match value.lower() {
            "resign" => Ok(Self::ResignEverything),
            "resign-last-only" => Ok(Self::ResignPreviousGenerationOnly),
            "ignore" => Ok(Self::IgnoreEverything),
            _ => Err("expected `resign`, `resign-last-only` or `ignore` for unsigned generations policy")
        }
    }
}
