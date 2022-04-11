use serde::Serialize;
use std::error::Error;

#[derive(Serialize, Default)]
#[serde(rename = "svg")]
pub struct Svg {
    pub g: G,
    pub width: String,
    pub height: String,
}

impl Svg {
    pub fn write_to(&self, path: &str) -> Result<(), Box<dyn Error>> {
        std::fs::write(path, quick_xml::se::to_string(&self)?)?;
        Ok(())
    }

    pub fn to_string(&self) -> String {
        quick_xml::se::to_string(&self).unwrap()
    }
}

#[derive(Serialize, Default)]
#[serde(rename = "g")]
pub struct G {
    pub rect: Vec<Rect>,
}

#[derive(Serialize, Default, Clone)]
#[serde(rename = "rect")]
pub struct Rect {
    pub style: String,
    pub id: String,
    pub width: String,
    pub height: String,
    pub x: String,
    pub y: String,
    pub rx: String,
}

impl Rect {
    pub fn to_svg(self) -> Svg {
        Svg {
            g: G {
                rect: vec![self.clone()],
            },
            width: self.width,
            height: self.height,
        }
    }
}

const RECT_SIDE: usize = 7;
const RECT_GAP: usize = 1;
const RECT_STYLE: &str = "fill:#c8ab37";
const BG_STYLE: &str = "fill:#333333";

fn adjust_horizontal(c: usize) -> usize {
    const MULTIPLIER: usize = 3;

    if c <= 3 {
        return 0 * MULTIPLIER;
    }
    if c <= 7 {
        return 1 * MULTIPLIER;
    }
    if c <= 9 {
        return 2 * MULTIPLIER;
    }
    if c <= 13 {
        return 3 * MULTIPLIER;
    }
    if c <= 17 {
        return 4 * MULTIPLIER;
    }
    if c <= 21 {
        return 5 * MULTIPLIER;
    }
    if c <= 25 {
        return 6 * MULTIPLIER;
    }

    return 7 * MULTIPLIER;
}

pub mod icon;
pub mod logo;
