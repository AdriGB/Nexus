use super::super::autonomy::Mind;
use super::super::config::MAX_HEALTH;
use super::super::entity::{Entity, EntityActivity, Sex};
use super::super::lifecycle::personality_for;
use super::super::spatial::EntitySnapshot;
use super::super::time::TICKS_PER_YEAR;
use super::super::Simulation;
use crate::world::{Grid, RenewableResource, ResourceDeposit, ResourceKind, Terrain, Tile};

pub(super) fn linear_visible_entities(
    entity_id: u32,
    position: (u32, u32),
    radius: u32,
    population: &[EntitySnapshot],
) -> Vec<u32> {
    let mut visible: Vec<u32> = population
        .iter()
        .filter(|other| {
            other.id != entity_id
                && position.0.abs_diff(other.x) + position.1.abs_diff(other.y) <= radius
        })
        .map(|other| other.id)
        .collect();
    visible.sort_unstable();
    visible
}

pub(super) fn grid_from_rows(rows: &[&str]) -> Grid {
    let height = rows.len() as u32;
    let width = rows.first().map_or(0, |row| row.len()) as u32;
    let tiles = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|symbol| Tile {
            terrain: match symbol {
                'P' | 'F' => Terrain::Plains,
                'M' => Terrain::Mountain,
                '#' => Terrain::DeepWater,
                _ => panic!("unknown terrain symbol: {symbol}"),
            },
            altitude: 0.0,
            moisture: 0.5,
            temperature: 0.5,
        })
        .collect();
    let resources = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|symbol| {
            (symbol == 'F').then_some(ResourceDeposit {
                kind: ResourceKind::Food,
                amount: 20,
            })
        })
        .collect();
    let renewable_resources = rows
        .iter()
        .flat_map(|row| row.chars())
        .enumerate()
        .filter_map(|(index, symbol)| {
            (symbol == 'F').then_some(RenewableResource {
                index,
                kind: ResourceKind::Food,
                capacity: 20,
            })
        })
        .collect();
    Grid {
        width,
        height,
        tiles,
        region_ids: Vec::new(),
        regions: Vec::new(),
        resources,
        renewable_resources,
    }
}

pub(super) fn plain_grid(width: u32, height: u32) -> Grid {
    let row = "P".repeat(width as usize);
    let rows: Vec<_> = (0..height).map(|_| row.as_str()).collect();
    grid_from_rows(&rows)
}

pub(super) fn entity(id: u32, x: u32, y: u32, hunger: f32) -> Entity {
    Entity {
        id,
        x,
        y,
        sex: Sex::Female,
        lifespan_ticks: 800_000,
        hunger,
        health: MAX_HEALTH,
        age_ticks: 0,
        path: Vec::new(),
        path_index: 0,
        activity: EntityActivity::Idle,
        mind: Mind::default(),
        pregnancy: None,
        postpartum_until_tick: 0,
        movement_credit: 0.0,
        mother_id: None,
        father_id: None,
        caregiver_id: None,
        partner_id: None,
        personality: personality_for(0, id),
        inventory: super::super::Inventory::default(),
        action_tick: 0,
    }
}

pub(super) fn simulation_with_entity(x: u32, y: u32, hunger: f32) -> Simulation {
    Simulation {
        entities: vec![entity(1, x, y, hunger)],
        next_entity_id: 2,
        ..Simulation::default()
    }
}

pub(super) fn fertile_entity(id: u32, sex: Sex, x: u32, y: u32) -> Entity {
    let mut entity = entity(id, x, y, 0.0);
    entity.sex = sex;
    entity.age_ticks = 25 * TICKS_PER_YEAR;
    entity
}
