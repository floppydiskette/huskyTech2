use gfx_maths::*;

pub struct Animation {
    pub loc1: Vec3,
    pub loc2: Vec3,
    pub time_to_animate: f32, // in milliseconds
}

impl Animation {
    pub fn new(loc1: Vec3, loc2: Vec3, time_to_animate: f32) -> Animation {
        Animation {
            loc1,
            loc2,
            time_to_animate,
        }
    }

    pub fn get_point_at_time(&self, time: f64) -> Vec3 {
        // time is in milliseconds
        let time_ratio = time / self.time_to_animate as f64;
        let x = self.loc1.x as f64 + ((self.loc2.x - self.loc1.x) as f64 * time_ratio);
        let y = self.loc1.y as f64 + ((self.loc2.y - self.loc1.y) as f64 * time_ratio);
        let z = self.loc1.z as f64 + ((self.loc2.z - self.loc1.z) as f64 * time_ratio);
        Vec3::new(x as f32, y as f32, z as f32)
    }
}