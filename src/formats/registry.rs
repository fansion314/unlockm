use super::{adapter::FormatAdapter, Kind};
use std::path::Path;

pub struct Adapter {
    kind: Kind,
    suffixes: &'static [&'static str],
    validate: fn(&Path, &[u8]) -> bool,
}
impl FormatAdapter for Adapter {
    fn kind(&self) -> Kind {
        self.kind
    }
    fn suffixes(&self) -> &'static [&'static str] {
        self.suffixes
    }
    fn validates(&self, path: &Path, header: &[u8]) -> bool {
        (self.validate)(path, header)
    }
}

const NCM: Adapter = Adapter {
    kind: Kind::Ncm,
    suffixes: &[".ncm"],
    validate: |_, h| h.len() >= 8 && &h[..8] == b"CTENFDAM",
};
const QMC_V1: Adapter = Adapter {
    kind: Kind::QmcV1,
    suffixes: &[
        ".qmc0",
        ".qmc2",
        ".qmc3",
        ".qmc4",
        ".qmc6",
        ".qmc8",
        ".tkm",
        ".bkcmp3",
        ".bkcm4a",
        ".bkcflac",
        ".bkcwav",
        ".bkcape",
        ".bkcogg",
        ".bkcwma",
        ".666c6163",
        ".6d7033",
        ".6f6767",
        ".6d3461",
        ".776176",
    ],
    validate: |_, h| !h.is_empty(),
};
const QMC_V2: Adapter = Adapter {
    kind: Kind::QmcV2,
    suffixes: &[
        ".qmcflac", ".qmcogg", ".mmp4", ".mgg", ".mgg0", ".mgg1", ".mgga", ".mggh", ".mggl",
        ".mggm", ".mflac", ".mflac0", ".mflac1", ".mflaca", ".mflach", ".mflacl", ".mflacm",
    ],
    validate: |_, h| !h.is_empty(),
};
const KGM: Adapter = Adapter {
    kind: Kind::Kgm,
    suffixes: &[".kgm.flac", ".vpr.flac", ".kgg", ".kgm", ".kgma", ".vpr"],
    validate: |_, h| umc_kgm::header::Header::from_buffer(h).is_ok(),
};
const KWM: Adapter = Adapter {
    kind: Kind::Kwm,
    suffixes: &[".kwm"],
    validate: |_, h| h.len() >= 0x40 && umc_kuwo::Header::from_bytes(&h[..0x40]).is_ok(),
};
const XIAMI: Adapter = Adapter {
    kind: Kind::Xiami,
    suffixes: &[".xm"],
    validate: |_, h| umc_xiami::XiamiFile::from_header(h).is_ok(),
};
const XMLY: Adapter = Adapter {
    kind: Kind::Ximalaya,
    suffixes: &[".x2m", ".x3m", ".xm"],
    validate: |p, h| {
        let n = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        (n.ends_with(".x2m") || n.ends_with(".x3m")) && h.len() >= 0x400
            || (!n.ends_with(".x2m")
                && !n.ends_with(".x3m")
                && umc_xmly::pc::Header::from_buffer(h).is_ok())
    },
};
const TM: Adapter = Adapter {
    kind: Kind::Tm,
    suffixes: &[".tm0", ".tm2", ".tm3", ".tm6"],
    validate: |_, h| h.starts_with(b"QQMU"),
};
const ADAPTERS: &[&dyn FormatAdapter] = &[&NCM, &QMC_V1, &QMC_V2, &KGM, &KWM, &XIAMI, &XMLY, &TM];

pub fn all_extensions() -> Vec<&'static str> {
    let mut values = ADAPTERS
        .iter()
        .flat_map(|adapter| adapter.suffixes().iter().copied())
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values
}

pub fn candidates(path: &Path) -> Vec<&'static dyn FormatAdapter> {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let mut found: Vec<_> = ADAPTERS
        .iter()
        .copied()
        .filter(|adapter| {
            adapter
                .suffixes()
                .iter()
                .any(|suffix| name.ends_with(suffix))
        })
        .collect();
    found.sort_by_key(|adapter| {
        std::cmp::Reverse(
            adapter
                .suffixes()
                .iter()
                .filter(|suffix| name.ends_with(**suffix))
                .map(|suffix| suffix.len())
                .max()
                .unwrap_or_default(),
        )
    });
    found
}
