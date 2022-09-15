use physx_sys::*;

pub struct PhysicsSystem {
    pub foundation: *mut PxFoundation,
    pub physics: *mut PxPhysics,
    pub scene: *mut PxScene,
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

        Self { foundation, physics, scene }
    }
}