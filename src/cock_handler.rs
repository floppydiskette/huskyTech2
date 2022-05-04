// file for handling cocks

trait CockElement {
    fn get_x(&self) -> i32;
    fn get_y(&self) -> i32;
    fn get_width(&self) -> i32;
    fn get_height(&self) -> i32;
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