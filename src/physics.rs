use std::collections::HashMap;
use std::ptr::{null, null_mut};
use gfx_maths::{Quaternion, Vec3};
use physx_sys::*;

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
            y: -9.81,
            z: 0.0,
        };

        let dispatcher = unsafe { phys_PxDefaultCpuDispatcherCreate(2, std::ptr::null_mut()) };

        scene_desc.cpuDispatcher = dispatcher as *mut _;
        scene_desc.filterShader = phys_PxDefaultSimulationFilterShader as *mut _;

        let scene = unsafe { PxPhysics_createScene_mut(physics, &scene_desc) };

        let controller_manager = unsafe { phys_PxCreateControllerManager(scene, true) };

        let physics_materials = Self::init_materials(physics);

        Self { foundation, physics, dispatcher, scene, controller_manager, physics_materials }
    }

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
            y: -9.81,
            z: 0.0,
        };

        scene_desc.cpuDispatcher = self.dispatcher as *mut _;
        scene_desc.filterShader = phys_PxDefaultSimulationFilterShader as *mut _;

        let scene = unsafe { PxPhysics_createScene_mut(self.physics, &scene_desc) };

        let controller_manager = unsafe { phys_PxCreateControllerManager(scene, true) };

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

                let filters = PxControllerFilters_new(
                    null(),
                    null::<PxQueryFilterCallback> as *mut _,
                    null::<PxControllerFilterCallback> as *mut _,
                );

                Some(PhysicsCharacterController { controller, filters, flags: CollisionFlags::default() })
            } else {
                None
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum ClimbingMode {
    Easy,
    Constrained,
    Last
}

#[derive(Clone, Debug)]
pub struct CollisionFlags {
    pub colliding_side: bool,
    pub colliding_top: bool,
    pub colliding_bottom: bool,
}

impl Default for CollisionFlags {
    fn default() -> Self {
        Self {
            colliding_side: false,
            colliding_top: false,
            colliding_bottom: false,
        }
    }
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
    pub filters: PxControllerFilters,
    pub flags: CollisionFlags,
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

        unsafe {
            let filters_ptr = &mut self.filters as *mut _;
            let flags = PxController_move_mut(self.controller, &mut displacement, 0.0, delta_time, filters_ptr, null_mut());
            self.flags = CollisionFlags::from_bits(flags.mBits);
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
        let mut position = PxExtendedVec3 {
            x: position.x as f64,
            y: position.y as f64,
            z: position.z as f64,
        };
        unsafe {
            PxController_setPosition_mut(self.controller, &mut position);
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