use iced_core::Font;
use iced_core::font::Error;
use iced_runtime::font::{self as iced_font};
use iced_runtime::Task;

pub const MATERIAL_FONT: &[u8] = include_bytes!("../assets/MaterialSymbolsRounded-VariableFont_FILL,GRAD,opsz,wght.ttf");
pub const MATERIAL_FAMILY: Font = Font::new("Material Symbols Rounded");

pub fn load(){
    // Load the font into ice system
    {
        use iced_graphics::text::font_system;
        let mut system = font_system().write().unwrap();
        system.raw().db_mut().load_font_data(MATERIAL_FONT.to_vec());
    }
}

