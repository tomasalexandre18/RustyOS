pub static FONT: &[u8] = include_bytes!("default8x16.psf");

const MAGIC_V1: u16 = 0x0436;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct PSF1Header {
    pub magic: u16,
    pub mode: u8,
    pub charsize: u8,
}

pub struct PSF1Font {
    pub header: PSF1Header,
    pub glyphs: &'static [u8],
}

pub(crate) fn load_font() -> PSF1Font {
    let header = unsafe { &*(FONT.as_ptr() as *const PSF1Header) };
    if header.magic != MAGIC_V1 {
        panic!("Invalid PSF1 font magic");
    }
    let glyphs = &FONT[core::mem::size_of::<PSF1Header>()..];
    PSF1Font { header: *header, glyphs }
}

fn ascii_to_glyph_index(c: char) -> char {
    if c.is_ascii() {
        c
    } else {
        '?' // fallback for non-ASCII chars
    }
}

fn get_glyph(font: &PSF1Font, c: char) -> &[u8] {
    let index = ascii_to_glyph_index(c) as usize;
    let glyph_size = font.header.charsize as usize;
    &font.glyphs[index * glyph_size..(index + 1) * glyph_size]
}

impl PSF1Font {
    pub fn get_glyph(&self, c: char) -> &[u8] {
        get_glyph(self, ascii_to_glyph_index(c))
    }

    pub fn char_width(&self) -> usize {
        8 // PSF1 fixed width
    }

    pub fn char_height(&self) -> usize {
        self.header.charsize as usize
    }
}