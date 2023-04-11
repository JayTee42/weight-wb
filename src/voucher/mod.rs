#[derive(Copy, Clone)]
pub struct Spacing {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

#[derive(Copy, Clone)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

#[derive(Copy, Clone)]
pub struct Layout {
    pub spacing: Spacing,
    pub alignment: Alignment,
    pub max_height: u32,
}

pub trait Component {
    fn draw_into(&self, image: &image::GrayImage, vert_offset: u32);
}

pub mod text;
