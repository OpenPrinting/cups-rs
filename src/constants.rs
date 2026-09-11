//! Constants for CUPS options and values

use crate::bindings;

// Destination flags
#[cfg(cups3)]
pub const DEST_FLAGS_NONE: u32 = bindings::cups_dest_flags_e_CUPS_DEST_FLAGS_NONE;
#[cfg(cups2)]
pub const DEST_FLAGS_NONE: u32 = bindings::CUPS_DEST_FLAGS_NONE;
#[cfg(cups3)]
pub const DEST_FLAGS_UNCONNECTED: u32 = bindings::cups_dest_flags_e_CUPS_DEST_FLAGS_UNCONNECTED;
#[cfg(cups2)]
pub const DEST_FLAGS_UNCONNECTED: u32 = bindings::CUPS_DEST_FLAGS_UNCONNECTED;
#[cfg(cups3)]
pub const DEST_FLAGS_MORE: u32 = bindings::cups_dest_flags_e_CUPS_DEST_FLAGS_MORE;
#[cfg(cups2)]
pub const DEST_FLAGS_MORE: u32 = bindings::CUPS_DEST_FLAGS_MORE;
#[cfg(cups3)]
pub const DEST_FLAGS_REMOVED: u32 = bindings::cups_dest_flags_e_CUPS_DEST_FLAGS_REMOVED;
#[cfg(cups2)]
pub const DEST_FLAGS_REMOVED: u32 = bindings::CUPS_DEST_FLAGS_REMOVED;
#[cfg(cups3)]
pub const DEST_FLAGS_ERROR: u32 = bindings::cups_dest_flags_e_CUPS_DEST_FLAGS_ERROR;
#[cfg(cups2)]
pub const DEST_FLAGS_ERROR: u32 = bindings::CUPS_DEST_FLAGS_ERROR;
#[cfg(cups3)]
pub const DEST_FLAGS_DEVICE: u32 = bindings::cups_dest_flags_e_CUPS_DEST_FLAGS_DEVICE;
#[cfg(cups2)]
pub const DEST_FLAGS_DEVICE: u32 = bindings::CUPS_DEST_FLAGS_DEVICE;

// Printer types
#[cfg(cups3)]
pub const PRINTER_CLASS: u32 = bindings::cups_ptype_e_CUPS_PTYPE_CLASS;
#[cfg(cups2)]
pub const PRINTER_CLASS: u32 = 0x00000001;
#[cfg(cups3)]
pub const PRINTER_FAX: u32 = bindings::cups_ptype_e_CUPS_PTYPE_FAX;
#[cfg(cups2)]
pub const PRINTER_FAX: u32 = 0x00000002;
#[cfg(cups3)]
pub const PRINTER_LOCAL: u32 = bindings::cups_ptype_e_CUPS_PTYPE_LOCAL;
#[cfg(cups2)]
pub const PRINTER_LOCAL: u32 = 0;
#[cfg(cups3)]
pub const PRINTER_REMOTE: u32 = bindings::cups_ptype_e_CUPS_PTYPE_REMOTE;
#[cfg(cups2)]
pub const PRINTER_REMOTE: u32 = 0x00000004;
#[cfg(cups3)]
pub const PRINTER_DISCOVERED: u32 = bindings::cups_ptype_e_CUPS_PTYPE_DISCOVERED;
#[cfg(cups2)]
pub const PRINTER_DISCOVERED: u32 = 0x00000008;
#[cfg(cups3)]
pub const PRINTER_BW: u32 = bindings::cups_ptype_e_CUPS_PTYPE_BW;
#[cfg(cups2)]
pub const PRINTER_BW: u32 = 0x00000010;
#[cfg(cups3)]
pub const PRINTER_COLOR: u32 = bindings::cups_ptype_e_CUPS_PTYPE_COLOR;
#[cfg(cups2)]
pub const PRINTER_COLOR: u32 = 0x00000020;
#[cfg(cups3)]
pub const PRINTER_DUPLEX: u32 = bindings::cups_ptype_e_CUPS_PTYPE_DUPLEX;
#[cfg(cups2)]
pub const PRINTER_DUPLEX: u32 = 0x00000040;
#[cfg(cups3)]
pub const PRINTER_STAPLE: u32 = bindings::cups_ptype_e_CUPS_PTYPE_STAPLE;
#[cfg(cups2)]
pub const PRINTER_STAPLE: u32 = 0x00000080;
#[cfg(cups3)]
pub const PRINTER_COLLATE: u32 = bindings::cups_ptype_e_CUPS_PTYPE_COLLATE;
#[cfg(cups2)]
pub const PRINTER_COLLATE: u32 = 0x00000100;
#[cfg(cups3)]
pub const PRINTER_PUNCH: u32 = bindings::cups_ptype_e_CUPS_PTYPE_PUNCH;
#[cfg(cups2)]
pub const PRINTER_PUNCH: u32 = 0x00000200;
#[cfg(cups3)]
pub const PRINTER_COVER: u32 = bindings::cups_ptype_e_CUPS_PTYPE_COVER;
#[cfg(cups2)]
pub const PRINTER_COVER: u32 = 0x00000400;
#[cfg(cups3)]
pub const PRINTER_BIND: u32 = bindings::cups_ptype_e_CUPS_PTYPE_BIND;
#[cfg(cups2)]
pub const PRINTER_BIND: u32 = 0x00000800;
#[cfg(cups3)]
pub const PRINTER_SORT: u32 = bindings::cups_ptype_e_CUPS_PTYPE_SORT;
#[cfg(cups2)]
pub const PRINTER_SORT: u32 = 0x00001000;
#[cfg(cups3)]
pub const PRINTER_SMALL: u32 = bindings::cups_ptype_e_CUPS_PTYPE_SMALL;
#[cfg(cups2)]
pub const PRINTER_SMALL: u32 = 0x00002000;
#[cfg(cups3)]
pub const PRINTER_MEDIUM: u32 = bindings::cups_ptype_e_CUPS_PTYPE_MEDIUM;
#[cfg(cups2)]
pub const PRINTER_MEDIUM: u32 = 0x00004000;
#[cfg(cups3)]
pub const PRINTER_LARGE: u32 = bindings::cups_ptype_e_CUPS_PTYPE_LARGE;
#[cfg(cups2)]
pub const PRINTER_LARGE: u32 = 0x00008000;
#[cfg(cups3)]
pub const PRINTER_VARIABLE: u32 = bindings::cups_ptype_e_CUPS_PTYPE_VARIABLE;
#[cfg(cups2)]
pub const PRINTER_VARIABLE: u32 = 0x00010000;

// Media flags
pub const MEDIA_FLAGS_DEFAULT: u32 = 0;
pub const MEDIA_FLAGS_BORDERLESS: u32 = 1 << 0;
pub const MEDIA_FLAGS_DUPLEX: u32 = 1 << 1;
pub const MEDIA_FLAGS_EXACT: u32 = 1 << 2;
pub const MEDIA_FLAGS_READY: u32 = 1 << 3;

// Option names
pub const COPIES: &str = "copies";
pub const FINISHINGS: &str = "finishings";
pub const MEDIA: &str = "media";
pub const MEDIA_SOURCE: &str = "media-source";
pub const MEDIA_TYPE: &str = "media-type";
pub const NUMBER_UP: &str = "number-up";
pub const ORIENTATION: &str = "orientation-requested";
pub const PRINT_COLOR_MODE: &str = "print-color-mode";
pub const PRINT_QUALITY: &str = "print-quality";
pub const SIDES: &str = "sides";

// Media values
pub const MEDIA_3X5: &str = "na_index-3x5_3x5in";
pub const MEDIA_4X6: &str = "na_index-4x6_4x6in";
pub const MEDIA_5X7: &str = "na_5x7_5x7in";
pub const MEDIA_8X10: &str = "na_govt-letter_8x10in";
pub const MEDIA_A3: &str = "iso_a3_297x420mm";
pub const MEDIA_A4: &str = "iso_a4_210x297mm";
pub const MEDIA_A5: &str = "iso_a5_148x210mm";
pub const MEDIA_A6: &str = "iso_a6_105x148mm";
pub const MEDIA_ENV10: &str = "na_number-10_4.125x9.5in";
pub const MEDIA_ENVDL: &str = "iso_dl_110x220mm";
pub const MEDIA_LEGAL: &str = "na_legal_8.5x14in";
pub const MEDIA_LETTER: &str = "na_letter_8.5x11in";
pub const MEDIA_PHOTO_L: &str = "oe_photo-l_3.5x5in";
pub const MEDIA_SUPERBA3: &str = "na_super-b_13x19in";
pub const MEDIA_TABLOID: &str = "na_ledger_11x17in";

// Media source values
pub const MEDIA_SOURCE_AUTO: &str = "auto";
pub const MEDIA_SOURCE_MANUAL: &str = "manual";

// Media type values
pub const MEDIA_TYPE_AUTO: &str = "auto";
pub const MEDIA_TYPE_ENVELOPE: &str = "envelope";
pub const MEDIA_TYPE_LABELS: &str = "labels";
pub const MEDIA_TYPE_LETTERHEAD: &str = "stationery-letterhead";
pub const MEDIA_TYPE_PHOTO: &str = "photographic";
pub const MEDIA_TYPE_PHOTO_GLOSSY: &str = "photographic-glossy";
pub const MEDIA_TYPE_PHOTO_MATTE: &str = "photographic-matte";
pub const MEDIA_TYPE_PLAIN: &str = "stationery";
pub const MEDIA_TYPE_TRANSPARENCY: &str = "transparency";

// Orientation values
pub const ORIENTATION_PORTRAIT: &str = "3";
pub const ORIENTATION_LANDSCAPE: &str = "4";

// Print color mode values
pub const PRINT_COLOR_MODE_AUTO: &str = "auto";
pub const PRINT_COLOR_MODE_MONOCHROME: &str = "monochrome";
pub const PRINT_COLOR_MODE_COLOR: &str = "color";

// Print quality values
pub const PRINT_QUALITY_DRAFT: &str = "3";
pub const PRINT_QUALITY_NORMAL: &str = "4";
pub const PRINT_QUALITY_HIGH: &str = "5";

// Sides values
pub const SIDES_ONE_SIDED: &str = "one-sided";
pub const SIDES_TWO_SIDED_PORTRAIT: &str = "two-sided-long-edge";
pub const SIDES_TWO_SIDED_LANDSCAPE: &str = "two-sided-short-edge";

pub const WHICHJOBS_ALL: i32 = -1;
pub const WHICHJOBS_ACTIVE: i32 = 0;
pub const WHICHJOBS_COMPLETED: i32 = 1;
