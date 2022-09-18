use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use gfx_maths::{Quaternion, Vec3};
use physx_sys::*;

pub const GRAVITY: f32 = -9.81;
pub const PLAYER_GRAVITY: f32 = -11.81;
pub const PLAYER_TERMINAL_VELOCITY: f32 = -90.0;
pub const PLAYER_JUMP_VELOCITY: f32 = 1.0;

#[derive(Clone)]
pub struct PhysicsSystem {
    pub foundation: *mut PxFoundation,
    pub physics: *mut PxPhysics,
    pub dispatcher: *mut PxDefaultCpuDispatcher,
    pub scene: *mut PxScene,
    pub controller_manager: *mut PxControllerManager,
    pub physics_materials: HashMap<Materials, PhysicsMaterial>,
}

unsafe impl Send for PhysicsSystem {
}

unsafe impl Sync for PhysicsSystem {
}

impl PhysicsSystem {
    pub fn init() -> Self {
        let foundation = unsafe { physx_create_foundation() };
        let physics = unsafe { physx_create_physics(foundation) };
        let mut scene_desc = unsafe { PxSceneDesc_new(PxPhysics_getTolerancesScale(physics)) };
        scene_desc.gravity = PxVec3 {
            x: 0.0,
            y: GRAVITY,
            z: 0.0,
        };

        let dispatcher = unsafe { phys_PxDefaultCpuDispatcherCreate(2, std::ptr::null_mut()) };

        scene_desc.cpuDispatcher = dispatcher as *mut _;
        scene_desc.filterShader = phys_PxDefaultSimulationFilterShader as *mut _;

        let scene = unsafe { PxPhysics_createScene_mut(physics, &scene_desc) };

        let controller_manager = unsafe { phys_PxCreateControllerManager(scene, true) };

        unsafe {
            PxControllerManager_setOverlapRecoveryModule_mut(controller_manager, true);
        }

        let physics_materials = Self::init_materials(physics);

        Self { foundation, physics, dispatcher, scene, controller_manager, physics_materials }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn init_materials(physics: *mut PxPhysics) -> HashMap<Materials, PhysicsMaterial> {
        let mut physics_materials = HashMap::new();

        let player_material = unsafe { PxPhysics_createMaterial_mut(physics, 0.0, 0.0, 0.0) };

        physics_materials.insert(Materials::Player, PhysicsMaterial { material: player_material });

        physics_materials
    }

    pub fn copy_with_new_scene(&self) -> Self {
        let mut scene_desc = unsafe { PxSceneDesc_new(PxPhysics_getTolerancesScale(self.physics)) };
        scene_desc.gravity = PxVec3 {
            x: 0.0,
            y: GRAVITY,
            z: 0.0,
        };

        scene_desc.cpuDispatcher = self.dispatcher as *mut _;
        scene_desc.filterShader = phys_PxDefaultSimulationFilterShader as *mut _;

        let scene = unsafe { PxPhysics_createScene_mut(self.physics, &scene_desc) };

        let controller_manager = unsafe { phys_PxCreateControllerManager(scene, true) };

        unsafe {
            PxControllerManager_setOverlapRecoveryModule_mut(controller_manager, true);
        }

        Self { foundation: self.foundation, physics: self.physics, dispatcher: self.dispatcher, scene, controller_manager, physics_materials: self.physics_materials.clone() }
    }

    pub fn tick(&self, delta_time: f32) {
        assert!(delta_time > 0.0);
        unsafe { PxScene_simulate_mut(self.scene, delta_time, null_mut(), null_mut(), 0, true) };
        let mut error = 0u32;
        unsafe { PxScene_fetchResults_mut(self.scene, true, &mut error) };
        assert_eq!(error, 0, "physx error: {}", error);
    }

    pub fn create_character_controller(&self, radius: f32, height: f32, step_offset: f32, material: Materials) -> Option<PhysicsCharacterController> {
        let mut controller_desc = unsafe { PxCapsuleControllerDesc_new_alloc() };
        unsafe { PxCapsuleControllerDesc_setToDefault_mut(controller_desc) };
        let material = self.physics_materials.get(&material).unwrap();
        unsafe {
            (*controller_desc).height = height;
            (*controller_desc).radius = radius;
            (*controller_desc).stepOffset = step_offset;
            (*controller_desc).material = material.material;

            if PxCapsuleControllerDesc_isValid(controller_desc) {
                let mut controller = PxControllerManager_createController_mut(self.controller_manager, controller_desc as *mut _);

                Some(PhysicsCharacterController {
                    controller,
                    flags: Arc::new(Mutex::new(CollisionFlags::default())),
                    y_velocity: Arc::new(UnsafeCell::new(0.0)),
                    jumping: Arc::new(UnsafeCell::new(false))
                })
            } else {
                None
            }
        }
    }

    pub fn create_box_collider_static(&self, position: Vec3, size: Vec3, material: Materials) -> Option<PhysicsBoxColliderStatic> {
        // physx defines the center of the box as the center of the bottom face
        // ht2 defines the center of the box as the top right of the bottom face
        let position = position + Vec3::new(size.x / 2.0, size.y / 2.0, -size.z / 2.0);
        let size = size;

        let transform = PxTransform {
            p: PxVec3 {
                x: position.x,
                y: position.y,
                z: position.z,
            },
            q: PxQuat {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        };

        let mut geometry = unsafe { PxBoxGeometry_new() };
        geometry.halfExtents = PxVec3 {
            x: size.x / 2.0,
            y: size.y / 2.0,
            z: size.z / 2.0,
        };

        let material = self.physics_materials.get(&material).unwrap();

        let box_actor = unsafe { PxPhysics_createRigidStatic_mut(self.physics, &transform) };
        let shape_flags = PxShapeFlag::eSIMULATION_SHAPE | PxShapeFlag::eSCENE_QUERY_SHAPE;
        let shape_flags = unsafe { PxShapeFlags {
            mBits: shape_flags as u8,
        } };
        let box_shape = unsafe {
            PxRigidActorExt_createExclusiveShape_mut(
                box_actor as *mut PxRigidActor,
                &geometry as *const PxBoxGeometry as *const PxGeometry,
                &material.material, 1, shape_flags) };
        Some(PhysicsBoxColliderStatic {
            actor: box_actor,
            shape: box_shape,
        })
    }
}

#[derive(Clone, Debug)]
pub enum ClimbingMode {
    Easy,
    Constrained,
    Last
}

#[derive(Clone, Debug, Default)]
pub struct CollisionFlags {
    pub colliding_side: bool,
    pub colliding_top: bool,
    pub colliding_bottom: bool,
}

impl CollisionFlags {
    pub fn from_bits(bits: u8) -> Self {
        Self {
            colliding_side: (bits & 1) != 0,
            colliding_top: (bits & 2) != 0,
            colliding_bottom: (bits & 4) != 0,
        }
    }
}

#[derive(Clone)]
pub struct PhysicsCharacterController {
    pub controller: *mut PxController,
    pub flags: Arc<Mutex<CollisionFlags>>,
    y_velocity: Arc<UnsafeCell<f32>>,
    jumping: Arc<UnsafeCell<bool>>,
}

unsafe impl Send for PhysicsCharacterController {
}

unsafe impl Sync for PhysicsCharacterController {
}

impl PhysicsCharacterController {
    pub fn move_by(&mut self, displacement: Vec3, delta_time: f32) {
        let mut displacement = PxVec3 {
            x: displacement.x,
            y: displacement.y,
            z: displacement.z,
        };

        if self.is_jumping() {
            unsafe {
                *self.y_velocity.get() = PLAYER_JUMP_VELOCITY;
            }
        } else {
            let gravity = GRAVITY * delta_time;
            let mut velocity = unsafe { *self.y_velocity.get() };
            velocity += gravity;
            velocity = velocity.max(PLAYER_TERMINAL_VELOCITY);
            unsafe {
                *self.y_velocity.get() = velocity;
            }
        }

        if self.is_jumping() {
            self.stop_jump();
        }

        displacement.y = unsafe { *self.y_velocity.get() };

        unsafe {
            let flags = PxController_move_mut(self.controller,
                                              &displacement,
                                              0.0,
                                              delta_time,
                                              &PxControllerFilters_new(null_mut(), null_mut(), null_mut()), null_mut());
            *self.flags.lock().unwrap() = CollisionFlags::from_bits(flags.mBits);
        }
    }

    pub fn start_jump(&self) {
        let mut jumping = self.jumping.get();
        unsafe {
            if !*jumping {
                debug!("start jump");
                *jumping = true;
                *self.y_velocity.get() = PLAYER_JUMP_VELOCITY;
            }
        }
    }

    pub fn is_jumping(&self) -> bool {
        let jumping = self.jumping.get();
        unsafe {
            *jumping
        }
    }

    pub fn stop_jump(&self) {
        debug!("stop jump");
        let mut jumping = self.jumping.get();
        unsafe {
            *jumping = false;
        }
    }

    pub fn get_position(&self) -> Vec3 {
        let mut position = unsafe {
            PxController_getPosition(self.controller)
        };
        let x = unsafe { (*position).x };
        let y = unsafe { (*position).y };
        let z = unsafe { (*position).z };
        Vec3::new(x as f32, y as f32, z as f32)
    }

    pub fn set_position(&self, position: Vec3) {
        let position = PxExtendedVec3 {
            x: position.x as f64,
            y: position.y as f64,
            z: position.z as f64,
        };
        unsafe {
            PxController_setPosition_mut(self.controller, &position);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Materials {
    Player,
}

#[derive(Copy, Clone, Debug)]
pub struct PhysicsMaterial {
    pub material: *mut PxMaterial,
}

unsafe impl Send for PhysicsMaterial {
}

unsafe impl Sync for PhysicsMaterial {
}

#[derive(Clone)]
pub struct PhysicsBoxColliderStatic {
    pub actor: *mut PxRigidStatic,
    pub shape: *mut PxShape,
}

impl PhysicsBoxColliderStatic {
    pub fn add_self_to_scene(&self, physics: PhysicsSystem) {
        unsafe {
            PxScene_addActor_mut(physics.scene, self.actor as *mut PxActor, null_mut());
        }
    }
}