// array of 2d points showing the path that the cool looking rainbow line will take
// to draw the HT2 logo


use std::ffi::CString;
use std::ptr::null;
use std::time::SystemTime;
use dae_parser::Document;
use gfx_maths::Vec2;
use libsex::bindings::*;
use crate::animation::Animation2D;
use crate::helpers::gen_rainbow;
use crate::renderer::{Colour, ht_renderer};

// todo: needs to be eventually converted to use vec2
#[derive(Clone, Copy)]
struct loc {
    x: i32,
    y: i32,
}

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


    let shader_index  = renderer.load_shader("rainbow").expect("failed to load rainbow shader");

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
            put_line(line, colour, shader_index, &mut renderer);
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
        let animation = Animation2D::new(Vec2::new(pointA.x as f32, pointA.y as f32), Vec2::new(pointB.x as f32, pointB.y as f32), time_of_each_line as f32);
        let pointB = animation.get_point_at_time(time);
        let pointB = loc { x: pointB.x as i32, y: pointB.y as i32 };

        // get a nice rainbow colour for the line
        let colour = gen_rainbow(time + i as f64 * time_of_each_line);

        let mut line = SunlustLine {
            pointA: pointA,
            pointB: pointB,
        };

        // draw the line
        put_line(&line, colour, shader_index, &mut renderer);


        // add delta time to the time
        let now = SystemTime::now();
        let delta_time = now.duration_since(last_time).unwrap().as_millis() as f64;
        last_time = now;
        time += delta_time;

        renderer.swap_buffers();

        // if the time is greater than the time we need for the current line, we need to move on to the next line
        if time > time_of_each_line as f64 {
            // add the previous line to the list of previous lines
            previous_lines.push(line);

            time = 0.0;
            i += 1;
        }
    }
    println!("done 2");
}

fn put_line(line: &SunlustLine, c: Colour, shader_index: usize, renderer: &mut ht_renderer) {
    // we need to map these coordinates from 640x480 to -1.0 to 1.0

    let pointA = Vec2::new(line.pointA.x as f32 / (640.0 / 2.0) - 1.0, line.pointA.y as f32 / (480.0 / 2.0) - 1.0);
    let pointB = Vec2::new(line.pointB.x as f32 / (640.0 / 2.0) - 1.0, line.pointB.y as f32 / (480.0 / 2.0) - 1.0);

    // we're using the core pipeline now, so we need to put these into a vertex array (and add a z value of 0.0)

    let verts: [f32; 6] = [pointA.x, pointA.y, 1.0, pointB.x, pointB.y, 1.0];

    unsafe {
        let mut vao = 0;
        glGenVertexArrays(1, &mut vao);
        glBindVertexArray(vao);

        let mut vbo = 0;
        glGenBuffers(1, &mut vbo);
        glBindBuffer(GL_ARRAY_BUFFER, vbo);
        glBufferData(GL_ARRAY_BUFFER, (6 * std::mem::size_of::<f32>()) as GLsizeiptr, verts.as_ptr() as *const GLvoid, GL_STATIC_DRAW);
        let pos = glGetAttribLocation(renderer.backend.shaders.as_mut().unwrap()[shader_index].program, CString::new("in_pos").unwrap().as_ptr());
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null());

        // colours
        let mut cbo = 0;
        glGenBuffers(1, &mut cbo);
        glBindBuffer(GL_ARRAY_BUFFER, cbo);

        let mut colours: [f32; 6] = [c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0, c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0];
        glBufferData(GL_ARRAY_BUFFER, (6 * std::mem::size_of::<f32>()) as GLsizeiptr, colours.as_ptr() as *const GLvoid, GL_STATIC_DRAW);

        glEnableVertexAttribArray(1); // colour
        glVertexAttribPointer(1, 3, GL_FLOAT, GL_FALSE as GLboolean, 0, null());
        glDrawArrays(GL_LINES, 0, 2);

        // clean up
        glDisableVertexAttribArray(0);
        glDisableVertexAttribArray(1);
        glBindBuffer(GL_ARRAY_BUFFER, 0);
        glBindVertexArray(0);

        glDeleteVertexArrays(1, &mut vao);
        glDeleteBuffers(1, &mut vbo);
        glDeleteBuffers(1, &mut cbo);
    }
}