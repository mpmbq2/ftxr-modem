// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 ftxr-modem Contributors
// Based on WSJT-X algorithms (Copyright (C) 2001-2026 WSJT Development Group).

//! Embedded GPL-3.0-only legal notices, upstream WSJT-X attribution, and trademark metadata.

pub const PACKAGE_NAME: &str = env!("CARGO_PKG_NAME");
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");
pub const LICENSE_SPDX: &str = env!("CARGO_PKG_LICENSE");

pub const SHORT_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (ipc-v1)");

pub const SHORT_LEGAL_NOTICE: &str = concat!(
    "ftxr-modem ",
    env!("CARGO_PKG_VERSION"),
    " (ipc-v1)\n",
    "License: GNU General Public License v3.0 only (GPL-3.0-only) <https://www.gnu.org/licenses/gpl-3.0.html>\n",
    "Copyright (C) 2026 ftxr-modem Contributors.\n",
    "This program comes with ABSOLUTELY NO WARRANTY; run `ftxr-modem --license` for details.\n",
    "Based on FT8/FT4 algorithms from WSJT-X (Copyright (C) 2001-2026 Joe Taylor, K1JT, et al.).\n",
    "Corresponding Source: ",
    env!("CARGO_PKG_REPOSITORY"),
    "\n",
);

pub fn tagged_source_url() -> String {
    format!("{REPOSITORY_URL}/tree/v{PACKAGE_VERSION}")
}

pub fn full_license_notice() -> String {
    format!(
        "{SHORT_LEGAL_NOTICE}\
         Tagged Release Source: {source_url}\n\n\
         This program is free software: you can redistribute it and/or modify\n\
         it under the terms of the GNU General Public License as published by\n\
         the Free Software Foundation, version 3 only (GPLv3).\n\n\
         Upstream Attribution:\n\
         Derived from WSJT-X classic FT8, FT4, and 77-bit message routines\n\
         (Copyright (C) 2001-2026 Joe Taylor, K1JT, and the WSJT Development Group;\n\
         https://wsjt.sourceforge.io/wsjtx.html).\n\n\
         Trademark Notice:\n\
         WSJT, WSJT-X, MAP65, and QMAP are trademarks of the WSJT Development Team.\n\
         ftxr-modem is an independent project based on WSJT-X algorithms and is\n\
         neither affiliated with nor endorsed by the WSJT Development Team.\n",
        source_url = tagged_source_url(),
    )
}
