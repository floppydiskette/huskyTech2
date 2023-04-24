use std::ffi::CString;
use glad_gl::gl;

// REPLACED UPON INIT
static mut TEXMESH: TextureMesh = TextureMesh {
    vbo: 0,
    vao: 0,
    ebo: 0,
};
static mut SHADER_PROGRAM: u32 = 0;

// contains a quad that we use to render textures to the screen
struct TextureMesh {
    vbo: u32,
    vao: u32,
    ebo: u32,
}

fn gen_texture_mesh() -> TextureMesh {
    let vertices: [f32; 20] = [
        // positions        // texture coords
        -1.0, 1.0, 0.0, 0.0, 1.0, // top left
        1.0, 1.0, 0.0, 1.0, 1.0, // top right
        1.0, -1.0, 0.0, 1.0, 0.0, // bottom right
        -1.0, -1.0, 0.0, 0.0, 0.0, // bottom left
    ];

    let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];

    let mut vbo = 0;
    let mut vao = 0;
    let mut ebo = 0;

    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::GenBuffers(1, &mut vbo);
        gl::GenBuffers(1, &mut ebo);

        gl::BindVertexArray(vao);

        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(gl::ARRAY_BUFFER, (vertices.len() * std::mem::size_of::<f32>()) as isize, vertices.as_ptr() as *const _, gl::STATIC_DRAW);

        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
        gl::BufferData(gl::ELEMENT_ARRAY_BUFFER, (indices.len() * std::mem::size_of::<u32>()) as isize, indices.as_ptr() as *const _, gl::STATIC_DRAW);

        gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, std::ptr::null());
        gl::EnableVertexAttribArray(0);

        gl::VertexAttribPointer(1, 2, gl::FLOAT, gl::FALSE, 5 * std::mem::size_of::<f32>() as i32, (3 * std::mem::size_of::<f32>()) as *const _);
        gl::EnableVertexAttribArray(1);

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);

        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, 0);
    }

    TextureMesh {
        vbo,
        vao,
        ebo,
    }
}

fn load_shader() -> u32 {
    // read the files
    let vert_source = include_str!("texture.vert");
    let frag_source = include_str!("texture.frag");

    // convert strings to c strings
    let vert_source_c = CString::new(vert_source).unwrap();
    let frag_source_c = CString::new(frag_source).unwrap();

    // create the shaders
    let vert_shader = unsafe { gl::CreateShader(gl::VERTEX_SHADER) };
    let frag_shader = unsafe { gl::CreateShader(gl::FRAGMENT_SHADER) };

    // set the source
    unsafe {
        gl::ShaderSource(vert_shader, 1, &vert_source_c.as_ptr(), std::ptr::null_mut());
        gl::ShaderSource(frag_shader, 1, &frag_source_c.as_ptr(), std::ptr::null_mut());
    }

    // compile the shaders
    unsafe {
        gl::CompileShader(vert_shader);
        gl::CompileShader(frag_shader);
    }

    // check if the shaders compiled
    let mut status = 0;
    unsafe {
        gl::GetShaderiv(vert_shader, gl::COMPILE_STATUS, &mut status);
        if status == 0 {
            let mut len = 255;
            gl::GetShaderiv(vert_shader, gl::INFO_LOG_LENGTH, &mut len);
            let log = vec![0; len as usize + 1];
            let log_c = CString::from_vec_unchecked(log);
            let log_p = log_c.into_raw();
            gl:: GetShaderInfoLog(vert_shader, len, std::ptr::null_mut(), log_p);
            panic!("failed to compile vertex shader: {}", CString::from_raw(log_p).to_string_lossy());
        }
        gl::GetShaderiv(frag_shader, gl::COMPILE_STATUS, &mut status);
        if status == 0 {
            let mut len = 255;
            gl::GetShaderiv(frag_shader, gl::INFO_LOG_LENGTH, &mut len);
            let log = vec![0; len as usize + 1];
            let log_c = CString::from_vec_unchecked(log);
            let log_p = log_c.into_raw();
            gl::GetShaderInfoLog(frag_shader, len, std::ptr::null_mut(), log_p);
            panic!("failed to compile fragment shader: {}", CString::from_raw(log_p).to_string_lossy());
        }
    }

    // link the shaders
    let shader_program = unsafe { gl::CreateProgram() };
    unsafe {
        gl::AttachShader(shader_program, vert_shader);
        gl:: AttachShader(shader_program, frag_shader);
        gl::LinkProgram(shader_program);
    }

    // check if the shaders linked
    unsafe {
        gl::GetProgramiv(shader_program, gl::LINK_STATUS, &mut status);
        if status == 0 {
            let mut len = 0;
            gl::GetProgramiv(shader_program, gl::INFO_LOG_LENGTH, &mut len);
            let mut log = Vec::with_capacity(len as usize);
            gl::GetProgramInfoLog(shader_program, len, std::ptr::null_mut(), log.as_mut_ptr() as *mut gl::GLchar);
            panic!("failed to link shader program: {}", std::str::from_utf8(&log).unwrap());
        }
    }

    // clean up
    unsafe {
        gl::DeleteShader(vert_shader);
        gl::DeleteShader(frag_shader);
    }

    shader_program
}

pub fn init() {
    unsafe {
        TEXMESH = gen_texture_mesh();
        SHADER_PROGRAM = load_shader();
    }
}

pub fn draw_texture(texture: u32, x: f32, y: f32, width: f32, height: f32) {
    static SCREEN_SIZE_C: &str = "u_screen_size\0";
    static TEXTURE_C: &str = "u_texture\0";
    static XY_C: &str = "u_xy\0";
    static WH_C: &str = "u_wh\0";
}