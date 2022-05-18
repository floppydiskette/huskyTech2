// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use std::time::SystemTime;
use dae_parser::Document;
use crate::animation::Animation2D;
use crate::helpers::gen_rainbow;
use crate::renderer::ht_renderer;

/*
this doesn't work with the new system (:
struct SunlustLine {
    pub pointA: loc,
    pub pointB: loc,
}

static points: [loc; 21] = [
    loc{x: 640, y: 104},
    loc{x: 640-445, y: 104}, // 445 = 320 (centre screen) + 13 (half of the T bar) + 30 (length of one T arm) + 26 (one H bar) + 30 (length of one H arm) + 26 (one H bar)
    loc{x: 640-445, y: 208}, // go down to the bottom of the H
    loc{x: 640-419, y: 208}, // to the right side of the H bar
    loc{x: 640-419, y: 208-39}, // go to the bottom side of the H arm
    loc{x: 640-389, y: 208-39}, // go to the right side of the H arm
    loc{x: 640-389, y: 208}, // go to the bottom side of the right H arm (btw my math fucked up after this one, so improvising it (: )
    loc{x: 640-363, y: 208}, // go to the right side of the right H arm
    loc{x: 640-363, y: 104+26}, // go just below the connector of the T to the H
    loc{x: 640-350, y: 104+26}, // go to the side of the T bar
    loc{x: 640-350, y: 208}, // go to the bottom of the T bar
    loc{x: 640-324, y: 208}, // go to the right side of the T bar
    loc{x: 640-324, y: 104+26}, // go to the bottom side of the T arm
    loc{x: 640-(221+26), y: 104+26}, // WE NEED TO ARC FROM THE LAST POINT TO THIS ONE, THIS IS THE 2
    loc{x: 640-298, y: 208-26}, // go to the slight bend in the bottom left of the 2
    loc{x: 640-298, y: 208}, // go to the bottom of the 2
    loc{x: 640-221, y: 208}, // go to the right side of the 2
    loc{x: 640-221, y: 208-26}, // go to the top part of the right side of the 2
    loc{x: 640-272, y: 208-26}, // go to the little crack in the left side of the 2
    loc{x: 640-221, y: 104+26}, // go to the top right of the 2
    loc{x: 640-324, y: 104}, // DO AN ARC ON THIS ONE TOO
];

pub fn animate(mut renderer: ht_renderer) {
    // first things first, figure out the number to multiply by so that the points get scaled up from 640x480 to the current resolution
    let mut scale_factor_x = 1.0;
    let mut scale_factor_y = 1.0;
    let mut current_resolution = renderer.window_size;
    if current_resolution.x > 640 {
        scale_factor_x = current_resolution.x as f32 / 640.0;
    }
    if current_resolution.y > 480 {
        scale_factor_y = current_resolution.y as f32 / 480.0;
    }

    // time for the rainbow outline animation
    let rainbow_length = 1122.0; // in milliseconds

    let mut time = 0.0; // used for working out how far a line should be drawn
    let mut last_time = SystemTime::now();

    let mut previous_lines: Vec<SunlustLine> = Vec::new();

    let mut i = 0; // index of the current point
    let mut time_of_each_line = rainbow_length / 21.0; // this is how long we'll allow for the drawing of a line before moving on to the next one
    while i < 20 { // loop through the points
        // we need to draw the line from the previous point to however far we are depending on the time
        // we also need to handle drawing all of the previous lines (if any)

        // for each of the previous lines, we need to draw them
        let mut j = 0;
        for line in previous_lines.iter() {
            let colour = gen_rainbow(time + j as f64 * time_of_each_line);
            renderer.put_line(line.pointA, line.pointB, colour);
            j += 1;
        }

        // i will be the starting point of the line
        // i+1 will be the end point of the line

        let mul_pointA = loc { x: (points[i].x as f32 * scale_factor_x) as i32, y: (points[i].y as f32 * scale_factor_y) as i32 };
        let mul_pointB = loc { x: (points[i + 1].x as f32 * scale_factor_x) as i32, y: (points[i + 1].y as f32 * scale_factor_y) as i32 };

        let pointA = mul_pointA;
        let pointB = mul_pointB;

        // we need to work out how far we should draw the line
        // for this, we can use the get_point_at_time function from the Animation2D struct
        let animation = Animation2D::new(pointA, pointB, time_of_each_line as f32);
        let pointB = animation.get_point_at_time(time);

        // get a nice rainbow colour for the line
        let colour = gen_rainbow(time + i as f64 * time_of_each_line);

        // draw the line
        renderer.put_line(pointA, pointB, colour);


        // add delta time to the time
        let now = SystemTime::now();
        let delta_time = now.duration_since(last_time).unwrap().as_millis() as f64;
        last_time = now;
        time += delta_time;

        renderer.swap_buffers();

        // if the time is greater than the time we need for the current line, we need to move on to the next line
        if time > time_of_each_line as f64 {
            // add the previous line to the list of previous lines
            previous_lines.push(SunlustLine { pointA: mul_pointA, pointB: mul_pointB });

            time = 0.0;
            i += 1;
        }
    }
    println!("done 2");
}
 */

pub fn animate(mut renderer: ht_renderer) {
    // load the huskyTech2 logo mesh
    let document = Document::from_file("base/models/ht2.dae").expect("failed to load dae file");
    let mesh = renderer.initMesh(document, "Cube_001-mesh", 1).unwrap();
}