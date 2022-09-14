use gfx_maths::Vec2;
use libsex::bindings::*;
use crate::ht_renderer;

#[cfg(feature = "glfw")]
#[derive(Clone, Copy)]
pub struct Texture {
    pub dimensions: (u32, u32),
    pub material: GLMaterial,
    pub diffuse_texture: GLuint,
    pub normal_texture: GLuint,
    pub metallic_texture: GLuint,
    pub roughness_texture: GLuint,
}

#[cfg(feature = "glfw")]
#[derive(Clone, Copy)]
pub struct GLMaterial {
    pub diffuse_texture: GLuint,
    pub metallic_texture: GLuint,
    pub roughness_texture: GLuint,
    pub normal_texture: GLuint,
}

#[derive(Clone, Copy, Debug)]
pub enum TextureError {
    FileNotFound,
    InvalidImage,
}

#[derive(Clone, Copy)]
pub struct UiTexture {
    pub dimensions: (u32, u32),
    pub diffuse_texture: GLuint,
}

pub struct Image {
    pub dimensions: (u32, u32),
    pub data: Vec<u8>,
}

impl Texture {
    pub fn load_texture(name: &str, path: &str, renderer: &mut ht_renderer) -> Result<(), TextureError> {
        let texture = Texture::new_from_name(path);
        if let Ok(texture) = texture {
            renderer.textures.insert(name.to_string(), texture);
            Ok(())
        } else {
            error!("failed to load texture: {}", name);
            Err(texture.err().unwrap())
        }
    }

    pub fn new_from_name(name: &str) -> Result<Texture, TextureError> {
        let base_file_name = format!("base/textures/{}_", name);
        // substance painter file names
        let diffuse_file_name = base_file_name.clone() + "diff.png";
        let normal_file_name = base_file_name.clone() + "normal.png";
        let metallic_file_name = base_file_name.clone() + "metal.png";
        let roughness_file_name = base_file_name.clone() + "rough.png";

        // load the files
        let diffuse_data = load_image(diffuse_file_name.as_str())?;

        {
            // load opengl textures
            let mut textures: [GLuint; 4] = [0; 4];
            unsafe {
                glGenTextures(4, textures.as_mut_ptr());
            }
            let diffuse_texture = textures[0];
            let normal_texture = textures[1];
            let metallic_texture = textures[2];
            let roughness_texture = textures[3];

            // diffuse texture
            unsafe {
                glBindTexture(GL_TEXTURE_2D, diffuse_texture);
                // IF YOU'RE GETTING A SEGFAULT HERE, MAKE SURE THE TEXTURE HAS AN ALPHA CHANNEL
                glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, diffuse_data.dimensions.0 as i32, diffuse_data.dimensions.1 as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, diffuse_data.data.as_ptr() as *const GLvoid);

                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as i32);
                glGenerateMipmap(GL_TEXTURE_2D);

                let normal_data = load_image(normal_file_name.as_str())?;
                let metallic_data = load_image(metallic_file_name.as_str())?;
                let roughness_data = load_image(roughness_file_name.as_str())?;

                assert!(diffuse_data.dimensions.0 == metallic_data.dimensions.0 && diffuse_data.dimensions.1 == metallic_data.dimensions.1);
                assert!(diffuse_data.dimensions.0 == roughness_data.dimensions.0 && diffuse_data.dimensions.1 == roughness_data.dimensions.1);
                assert!(diffuse_data.dimensions.0 == normal_data.dimensions.0 && diffuse_data.dimensions.1 == normal_data.dimensions.1);

                // normal texture
                unsafe {
                    glBindTexture(GL_TEXTURE_2D, normal_texture);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, normal_data.dimensions.0 as i32, normal_data.dimensions.1 as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, normal_data.data.as_ptr() as *const GLvoid);

                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as i32);
                    glGenerateMipmap(GL_TEXTURE_2D);
                }

                // metallic texture
                unsafe {
                    glBindTexture(GL_TEXTURE_2D, metallic_texture);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, metallic_data.dimensions.0 as i32, metallic_data.dimensions.1 as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, metallic_data.data.as_ptr() as *const GLvoid);

                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as i32);
                    glGenerateMipmap(GL_TEXTURE_2D);
                }

                // roughness texture
                unsafe {
                    glBindTexture(GL_TEXTURE_2D, roughness_texture);
                    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, roughness_data.dimensions.0 as i32, roughness_data.dimensions.1 as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, roughness_data.data.as_ptr() as *const GLvoid);

                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as i32);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as i32);
                    glGenerateMipmap(GL_TEXTURE_2D);
                }
            }

            let material = GLMaterial {
                    diffuse_texture,
                    metallic_texture,
                    roughness_texture,
                    normal_texture
                };

            // return
            Ok(Texture {
                dimensions: diffuse_data.dimensions,
                material,
                diffuse_texture,
                normal_texture,
                metallic_texture,
                roughness_texture,
            })
        }
    }
}

impl UiTexture {
    pub fn new_from_name(name: String) -> Result<UiTexture, String> {
        let base_file_name = format!("base/textures/ui/{}", name); // substance painter file names
        let diffuse_file_name = base_file_name.clone() + ".png";

        // load the files
        let diffuse_data = load_image(diffuse_file_name.as_str())?;

        #[cfg(feature = "glfw")]
        {
            // load opengl textures
            let mut textures: [GLuint; 1] = [0; 1];
            unsafe {
                glGenTextures(1, textures.as_mut_ptr());
            }
            let diffuse_texture = textures[0];

            // diffuse texture
            unsafe {
                glBindTexture(GL_TEXTURE_2D, diffuse_texture);
                glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA as i32, diffuse_data.dimensions.0 as i32, diffuse_data.dimensions.1 as i32, 0, GL_RGBA, GL_UNSIGNED_BYTE, diffuse_data.data.as_ptr() as *const GLvoid);

                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_REPEAT as i32);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_REPEAT as i32);
                glGenerateMipmap(GL_TEXTURE_2D);
            }

            // todo: for now we're only using diffuse textures

            // return
            Ok(UiTexture {
                dimensions: diffuse_data.dimensions,
                diffuse_texture,
            })
        }
    }
}

fn load_image(file_name: &str) -> Result<Image, String> { // todo: use dds
    let decoder = png::Decoder::new(std::fs::File::open(file_name).map_err(|e| format!("failed to open file: {}", e))?);
    let mut reader = decoder.read_info().map_err(|e| format!("failed to read file: {}", e))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("failed to read file: {}", e))?;
    let dimensions = (info.width, info.height);
    let bytes = &buf[0..info.buffer_size()];
    let data = bytes.to_vec();
    Ok(Image { dimensions, data })
}