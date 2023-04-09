use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use gfx_maths::Vec2;
use glad_gl::gl::*;
use crate::ht_renderer;

pub struct Texture {
    pub dimensions: (u32, u32),
    pub material: GLMaterial,
    pub diffuse_texture: GLuint,
    pub normal_texture: GLuint,
    pub metallic_texture: GLuint,
    pub roughness_texture: GLuint,
    atomic_ref_count: Arc<AtomicUsize>,
}

impl Clone for Texture {
    fn clone(&self) -> Self {
        self.atomic_ref_count.fetch_add(1, Ordering::SeqCst);
        Texture {
            dimensions: self.dimensions,
            material: self.material,
            diffuse_texture: self.diffuse_texture,
            normal_texture: self.normal_texture,
            metallic_texture: self.metallic_texture,
            roughness_texture: self.roughness_texture,
            atomic_ref_count: self.atomic_ref_count.clone(),
        }
    }
}

#[cfg(feature = "glfw")]
#[derive(Clone, Copy)]
pub struct GLMaterial {
    pub diffuse_texture: GLuint,
    pub metallic_texture: GLuint,
    pub roughness_texture: GLuint,
    pub normal_texture: GLuint,
}

#[derive(Clone, Debug)]
pub enum TextureError {
    LoadingError(String),
    InvalidImage,
}

pub struct UiTexture {
    pub dimensions: (u32, u32),
    pub diffuse_texture: GLuint,
    atomic_ref_count: Arc<AtomicUsize>,
}

impl Clone for UiTexture {
    fn clone(&self) -> Self {
        self.atomic_ref_count.fetch_add(1, Ordering::SeqCst);
        UiTexture {
            dimensions: self.dimensions,
            diffuse_texture: self.diffuse_texture,
            atomic_ref_count: self.atomic_ref_count.clone(),
        }
    }
}

pub struct Image {
    pub dimensions: (u32, u32),
    pub data: Vec<u8>,
}

pub struct IntermidiaryTexture {
    pub diffuse: Image,
    pub normal: Image,
    pub metallic: Image,
    pub roughness: Image,
}

impl Drop for Texture {
    fn drop(&mut self) {
        if self.atomic_ref_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.unload();
        }
    }
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
        let base_file_name = format!("base/textures/{}/{}_", name, name);
        // substance painter file names
        let diffuse_file_name = base_file_name.clone() + "diff.png";
        let normal_file_name = base_file_name.clone() + "normal.png";
        let metallic_file_name = base_file_name.clone() + "metal.png";
        let roughness_file_name = base_file_name.clone() + "rough.png";

        // load the files
        let diffuse_data = load_image(diffuse_file_name.as_str()).map_err(|e| TextureError::LoadingError(e.clone()))?;

        {
            // load opengl textures
            let mut textures: [GLuint; 4] = [0; 4];
            unsafe {
                GenTextures(4, textures.as_mut_ptr());
            }
            let diffuse_texture = textures[0];
            let normal_texture = textures[1];
            let metallic_texture = textures[2];
            let roughness_texture = textures[3];

            // diffuse texture
            unsafe {
                BindTexture(TEXTURE_2D, diffuse_texture);
                // IF YOU'RE GETTING A SEGFAULT HERE, MAKE SURE THE TEXTURE HAS AN ALPHA CHANNEL
                TexImage2D(TEXTURE_2D, 0, SRGB_ALPHA as i32, diffuse_data.dimensions.0 as i32, diffuse_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, diffuse_data.data.as_ptr() as *const GLvoid);

                TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                GenerateMipmap(TEXTURE_2D);

                let normal_data = load_image(normal_file_name.as_str()).map_err(|e| TextureError::LoadingError(e.clone()))?;
                let metallic_data = load_image(metallic_file_name.as_str()).map_err(|e| TextureError::LoadingError(e.clone()))?;
                let roughness_data = load_image(roughness_file_name.as_str()).map_err(|e| TextureError::LoadingError(e.clone()))?;

                assert!(diffuse_data.dimensions.0 == metallic_data.dimensions.0 && diffuse_data.dimensions.1 == metallic_data.dimensions.1);
                assert!(diffuse_data.dimensions.0 == roughness_data.dimensions.0 && diffuse_data.dimensions.1 == roughness_data.dimensions.1);
                assert!(diffuse_data.dimensions.0 == normal_data.dimensions.0 && diffuse_data.dimensions.1 == normal_data.dimensions.1);

                // normal texture
                unsafe {
                    BindTexture(TEXTURE_2D, normal_texture);
                    TexImage2D(TEXTURE_2D, 0, RGB as i32, normal_data.dimensions.0 as i32, normal_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, normal_data.data.as_ptr() as *const GLvoid);

                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    GenerateMipmap(TEXTURE_2D);
                }

                // metallic texture
                unsafe {
                    BindTexture(TEXTURE_2D, metallic_texture);
                    TexImage2D(TEXTURE_2D, 0, RGBA as i32, metallic_data.dimensions.0 as i32, metallic_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, metallic_data.data.as_ptr() as *const GLvoid);

                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    GenerateMipmap(TEXTURE_2D);
                }

                // roughness texture
                unsafe {
                    BindTexture(TEXTURE_2D, roughness_texture);
                    TexImage2D(TEXTURE_2D, 0, RGBA as i32, roughness_data.dimensions.0 as i32, roughness_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, roughness_data.data.as_ptr() as *const GLvoid);

                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    GenerateMipmap(TEXTURE_2D);
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
                atomic_ref_count: Arc::new(AtomicUsize::new(1)),
            })
        }
    }

    /// begins loading an image on a new thread; once complete, the returned atomic bool will be set to true
    /// and the image data will be available in the returned Arc<Mutex<Option<IntermidiaryTexture>>>
    /// then, you must call `Texture::load_from_intermidiary` to load the texture into opengl
    pub fn new_from_name_asynch_begin(name: &str) -> (Arc<AtomicBool>, Arc<Mutex<Option<IntermidiaryTexture>>>) {
        let finished = Arc::new(AtomicBool::new(false));
        let finished_clone = finished.clone();
        let texture = Arc::new(Mutex::new(None));
        let texture_clone = texture.clone();
        let name_clone = name.to_string();

        thread::spawn(move || {
            let base_file_name = format!("base/textures/{}/{}_", name_clone, name_clone);
            // substance painter file names
            let diffuse_file_name = base_file_name.clone() + "diff.png";
            let normal_file_name = base_file_name.clone() + "normal.png";
            let metallic_file_name = base_file_name.clone() + "metal.png";
            let roughness_file_name = base_file_name.clone() + "rough.png";

            let diffuse_data = load_image(diffuse_file_name.as_str()).unwrap_or_else(|e| {
                println!("error loading diffuse texture: {}", e);
                panic!();
            });
            let normal_data = load_image(normal_file_name.as_str()).unwrap_or_else(|e| {
                println!("error loading normal texture: {}", e);
                panic!();
            });
            let metallic_data = load_image(metallic_file_name.as_str()).unwrap_or_else(|e| {
                println!("error loading metallic texture: {}", e);
                panic!();
            });
            let roughness_data = load_image(roughness_file_name.as_str()).unwrap_or_else(|e| {
                println!("error loading roughness texture: {}", e);
                panic!();
            });

            assert!(diffuse_data.dimensions.0 == metallic_data.dimensions.0 && diffuse_data.dimensions.1 == metallic_data.dimensions.1);
            assert!(diffuse_data.dimensions.0 == roughness_data.dimensions.0 && diffuse_data.dimensions.1 == roughness_data.dimensions.1);
            assert!(diffuse_data.dimensions.0 == normal_data.dimensions.0 && diffuse_data.dimensions.1 == normal_data.dimensions.1);

            let texture = IntermidiaryTexture {
                diffuse: diffuse_data,
                normal: normal_data,
                metallic: metallic_data,
                roughness: roughness_data,
            };

            *texture_clone.lock().unwrap() = Some(texture);
            finished_clone.store(true, Ordering::SeqCst);
        });
        (finished, texture)
    }

    /// loads a texture from an intermidiary texture
    pub fn load_from_intermidiary(inter: Option<IntermidiaryTexture>) -> Result<Self, TextureError> {
        let inter = inter.ok_or(TextureError::LoadingError("texture failed to load!".to_string()))?;
        let diffuse_data = inter.diffuse;
        let normal_data = inter.normal;
        let metallic_data = inter.metallic;
        let roughness_data = inter.roughness;

        {
            // load opengl textures
            let mut textures: [GLuint; 4] = [0; 4];
            unsafe {
                GenTextures(4, textures.as_mut_ptr());
            }
            let diffuse_texture = textures[0];
            let normal_texture = textures[1];
            let metallic_texture = textures[2];
            let roughness_texture = textures[3];

            // diffuse texture
            unsafe {
                BindTexture(TEXTURE_2D, diffuse_texture);
                // IF YOU'RE GETTING A SEGFAULT HERE, MAKE SURE THE TEXTURE HAS AN ALPHA CHANNEL
                TexImage2D(TEXTURE_2D, 0, SRGB_ALPHA as i32, diffuse_data.dimensions.0 as i32, diffuse_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, diffuse_data.data.as_ptr() as *const GLvoid);

                TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                GenerateMipmap(TEXTURE_2D);

                // normal texture
                unsafe {
                    BindTexture(TEXTURE_2D, normal_texture);
                    TexImage2D(TEXTURE_2D, 0, RGB as i32, normal_data.dimensions.0 as i32, normal_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, normal_data.data.as_ptr() as *const GLvoid);

                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    GenerateMipmap(TEXTURE_2D);
                }

                // metallic texture
                unsafe {
                    BindTexture(TEXTURE_2D, metallic_texture);
                    TexImage2D(TEXTURE_2D, 0, RGBA as i32, metallic_data.dimensions.0 as i32, metallic_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, metallic_data.data.as_ptr() as *const GLvoid);

                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    GenerateMipmap(TEXTURE_2D);
                }

                // roughness texture
                unsafe {
                    BindTexture(TEXTURE_2D, roughness_texture);
                    TexImage2D(TEXTURE_2D, 0, RGBA as i32, roughness_data.dimensions.0 as i32, roughness_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, roughness_data.data.as_ptr() as *const GLvoid);

                    TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                    TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                    GenerateMipmap(TEXTURE_2D);
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
                atomic_ref_count: Arc::new(AtomicUsize::new(1)),
            })
        }
    }

    pub fn unload(&mut self) {
        unsafe {
            DeleteTextures(4, [self.diffuse_texture, self.normal_texture, self.metallic_texture, self.roughness_texture].as_ptr());
        }
    }
}

impl Drop for UiTexture {
    fn drop(&mut self) {
        if self.atomic_ref_count.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.unload();
        }
    }
}

impl UiTexture {
    pub fn new_from_name(name: String) -> Result<UiTexture, String> {
        let base_file_name = format!("base/textures/ui/{}", name); // substance painter file names
        let diffuse_file_name = base_file_name + ".png";

        // load the files
        let diffuse_data = load_image(diffuse_file_name.as_str())?;

        #[cfg(feature = "glfw")]
        {
            // load opengl textures
            let mut textures: [GLuint; 1] = [0; 1];
            unsafe {
                GenTextures(1, textures.as_mut_ptr());
            }
            let diffuse_texture = textures[0];

            // diffuse texture
            unsafe {
                BindTexture(TEXTURE_2D, diffuse_texture);
                TexImage2D(TEXTURE_2D, 0, SRGB_ALPHA as i32, diffuse_data.dimensions.0 as i32, diffuse_data.dimensions.1 as i32, 0, RGBA, UNSIGNED_BYTE, diffuse_data.data.as_ptr() as *const GLvoid);

                TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                GenerateMipmap(TEXTURE_2D);
            }

            // todo: for now we're only using diffuse textures

            // return
            Ok(UiTexture {
                dimensions: diffuse_data.dimensions,
                diffuse_texture,
                atomic_ref_count: Arc::new(AtomicUsize::new(1)),
            })
        }
    }

    pub fn new_from_rgba_bytes(bytes: &[u8], dimensions: (u32, u32)) -> Result<UiTexture, String> {
        #[cfg(feature = "glfw")]
        {
            // load opengl textures
            let mut textures: [GLuint; 1] = [0; 1];
            unsafe {
                GenTextures(1, textures.as_mut_ptr());
            }
            let diffuse_texture = textures[0];

            // diffuse texture
            unsafe {
                BindTexture(TEXTURE_2D, diffuse_texture);

                TexImage2D(TEXTURE_2D, 0, RGBA8 as i32, dimensions.0 as i32, dimensions.1 as i32, 0, RGBA8, UNSIGNED_BYTE, bytes.as_ptr() as *const GLvoid);

                TexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR_MIPMAP_LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, REPEAT as i32);
                TexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, REPEAT as i32);
                GenerateMipmap(TEXTURE_2D);
            }

            // return
            Ok(UiTexture {
                dimensions,
                diffuse_texture,
                atomic_ref_count: Arc::new(AtomicUsize::new(1)),
            })
        }
    }

    pub fn unload(&mut self) {
        #[cfg(feature = "glfw")]
        {
            unsafe {
                DeleteTextures(1, &self.diffuse_texture);
            }
        }
    }
}

fn load_image(file_name: &str) -> Result<Image, String> { // todo: use dds
    let img_data = std::io::BufReader::new(std::fs::File::open(file_name).map_err(|e| format!("Failed to load image: {}", e))?);
    let img = image::io::Reader::new(img_data).with_guessed_format().map_err(|e| format!("Failed to load image: {}", e))?.decode().map_err(|e| format!("Failed to load image: {}", e))?;
    let rgba = img.into_rgba8();
    let dimensions = rgba.dimensions();
    let data = rgba.to_vec();
    Ok(Image { dimensions, data })
}