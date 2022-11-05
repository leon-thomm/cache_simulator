use bevy_ecs::prelude::*;
use bevy_ecs::world::World;

struct Addr(u64);

enum Inst {
    Load(Addr),
    Store(Addr),
    Other(u64),
}

#[derive(Component)]
struct Instructions(Vec<Inst>);

fn fetch_next_instr(mut query: Query<(Entity, &mut Instructions)>) -> Option<(Entity, Inst)> {
    for (entity, mut instructions) in &mut query {
        instructions.0.remove(0);
        if let Some(inst) = instructions.0.pop() {
            return Some((entity, inst));
        }
    }
    None
}

#[derive(Component)]
enum ProcState {
    Ready,
    ExecOther,
    WaitForCache,
}

// schedule stages

#[derive(StageLabel)]
struct UpdateProcs;

#[derive(StageLabel)]
struct UpdateCaches;

#[derive(StageLabel)]
struct UpdateBus;




#[derive(Component)]
struct Position { x: f32, y: f32 }

fn print_position(mut query: Query<(Entity, &mut Instructions)>) -> Option<(Entity, Inst)> {
    for (entity, mut instructions) in &mut query {
        // instructions.0.remove(0);
        // if let Some(inst) = instructions.0.pop() {
        //     return Some((entity, inst));
        // }
    }
    None
}




// simulation

fn main() {

    let mut world = World::default();

    let proc0 = world.spawn()
        .insert_bundle((
            Instructions(vec![
                Inst::Load(Addr(0)), 
                Inst::Other(5),
                Inst::Store(Addr(1)),
            ]),
        ))
        .id();
    
    let mut sim_sched = Schedule::default();
    sim_sched.add_stage(UpdateProcs, SystemStage::parallel()
        .with_system(print_position)
    );

    println!("Hello, world!");
}
