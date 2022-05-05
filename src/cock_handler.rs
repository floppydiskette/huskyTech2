// file for handling cocks

use crate::Thingy;

trait CockElement {
    fn get_x(&self) -> i32;
    fn get_y(&self) -> i32;
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
}

pub struct CockBlock { // a collection of cock elements
    x: i32,
    y: i32,
    cached_width: i32,
    cached_height: i32,
    elements: Vec<&'static dyn CockElement>,
}

impl Thingy for CockBlock {
    fn get_x(&self) -> i32 {
        self.x
    }

    fn get_y(&self) -> i32 {
        self.y
    }

    fn get_z(&self) -> i32 {
        0
    }

    fn get_width(&self) -> i32 {
        self.cached_width
    }

    fn get_height(&self) -> i32 {
        self.cached_height
    }

    fn get_depth(&self) -> i32 {
        0
    }
}

impl CockElement for CockBlock {
    fn get_x(&self) -> i32 {
        self.x
    }

    fn get_y(&self) -> i32 {
        self.y
    }

    fn get_width(&self) -> i32 {
        self.cached_width
    }
    fn get_height(&self) -> i32 {
        self.cached_height
    }
}

pub struct CockBasicText {
    x: i32,
    y: i32,
    text: String,
}

impl CockElement for CockBasicText {
    fn get_x(&self) -> i32 {
        self.x
    }

    fn get_y(&self) -> i32 {
        self.y
    }

    fn get_width(&self) -> i32 {
        // character width is 8 pixels, so multiply text length by 8
        self.text.len() as i32 * 8
    }

    fn get_height(&self) -> i32 {
        // character height is 8 pixels, so return 8
        8
    }
}

impl CockBasicText {
    pub fn new(x: i32, y: i32, text: String) -> CockBasicText {
        CockBasicText {
            x,
            y,
            text,
        }
    }

    pub fn set_text(&mut self, text: String) {
        self.text = text;
    }

    pub fn get_text(&self) -> String {
        self.text.clone()
    }
}

pub fn parse_cock(cock: &str) -> Result<&'static dyn CockElement, String> {
    // if the line begins with two forward slashes, it's a comment
    if cock.starts_with("//") {
        return Err("comment".to_string());
    }

    return Err("no cock ):".to_string());
}