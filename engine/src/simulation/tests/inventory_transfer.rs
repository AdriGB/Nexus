use super::super::{ItemKind, Simulation};
use super::support::entity;

fn simulation_with_inventory_pair() -> Simulation {
    let mut source = entity(1, 1, 1, 0.0);
    source.inventory.add(ItemKind::Food, 20);
    Simulation {
        entities: vec![source, entity(2, 2, 1, 0.0)],
        next_entity_id: 3,
        ..Simulation::default()
    }
}

#[test]
fn transfer_moves_items_without_changing_the_total() {
    let mut simulation = simulation_with_inventory_pair();

    assert_eq!(simulation.transfer_item(1, 2, ItemKind::Food, 7), 7);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 13);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 7);
    assert_eq!(
        simulation
            .entities
            .iter()
            .map(|entity| entity.inventory.amount(ItemKind::Food))
            .sum::<u16>(),
        20
    );
}

#[test]
fn transfer_is_partial_when_source_or_target_is_limited() {
    let mut simulation = simulation_with_inventory_pair();
    simulation.entities[1].inventory.add(ItemKind::Stone, 45);

    assert_eq!(simulation.transfer_item(1, 2, ItemKind::Food, 20), 5);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 15);
    assert_eq!(simulation.entities[1].inventory.used_capacity(), 50);

    assert_eq!(simulation.transfer_item(2, 1, ItemKind::Food, 10), 5);
    assert_eq!(simulation.entities[0].inventory.amount(ItemKind::Food), 20);
    assert_eq!(simulation.entities[1].inventory.amount(ItemKind::Food), 0);
}

#[test]
fn invalid_and_self_transfers_are_no_ops() {
    let mut simulation = simulation_with_inventory_pair();
    let before = simulation.entities.clone();

    for (source, target, quantity) in [(1, 1, 5), (99, 2, 5), (1, 99, 5), (1, 2, 0)] {
        assert_eq!(
            simulation.transfer_item(source, target, ItemKind::Food, quantity),
            0
        );
    }
    assert_eq!(simulation.entities[0].inventory, before[0].inventory);
    assert_eq!(simulation.entities[1].inventory, before[1].inventory);
}
