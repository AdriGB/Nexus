use super::super::entity::{Pregnancy, Sex};
use super::super::genealogy::Genealogy;
use super::super::{children_of, siblings_of, Simulation};
use super::support::{entity, plain_grid};

#[test]
fn founders_are_registered_without_known_parents() {
    let mut simulation = Simulation::default();
    simulation.push_entity((0, 0), 20).unwrap();
    simulation.push_entity((1, 0), 20).unwrap();

    let first = simulation.genealogy().get(1).expect("first founder");
    let second = simulation.genealogy().get(2).expect("second founder");
    assert_eq!((first.mother_id, first.father_id), (None, None));
    assert_eq!((second.mother_id, second.father_id), (None, None));
}

#[test]
fn newborn_genealogy_matches_living_entity_parentage() {
    let world = plain_grid(4, 4);
    let mut mother = entity(1, 1, 1, 0.0);
    mother.sex = Sex::Female;
    mother.pregnancy = Some(Pregnancy {
        father_id: 2,
        conceived_tick: 0,
        due_tick: 1,
    });
    let father = entity(2, 2, 1, 0.0);
    let mut simulation = Simulation {
        tick: 1,
        entities: vec![mother, father],
        next_entity_id: 3,
        ..Simulation::default()
    };
    simulation.genealogy.register(1, None, None);
    simulation.genealogy.register(2, None, None);

    simulation.update_pregnancies(&world);

    let child = simulation
        .entities()
        .iter()
        .find(|entity| entity.id == 3)
        .unwrap();
    let lineage = simulation.genealogy().get(3).expect("child lineage");
    assert_eq!(lineage.mother_id, child.mother_id);
    assert_eq!(lineage.father_id, child.father_id);
    assert_eq!((lineage.mother_id, lineage.father_id), (Some(1), Some(2)));
}

#[test]
fn genealogy_and_kinship_survive_entity_death() {
    let mut parent = entity(1, 0, 0, 0.0);
    let mut living_child = entity(3, 0, 0, 0.0);
    living_child.father_id = Some(1);
    let mut dead_child = entity(2, 0, 0, 0.0);
    dead_child.father_id = Some(1);
    parent.health = 0.0;
    dead_child.health = 0.0;
    let mut simulation = Simulation {
        entities: vec![parent, dead_child, living_child],
        next_entity_id: 4,
        ..Simulation::default()
    };
    simulation.genealogy.register(1, None, None);
    simulation.genealogy.register(2, None, Some(1));
    simulation.genealogy.register(3, None, Some(1));

    simulation.remove_dead_entities();

    assert!(simulation.genealogy().get(1).is_some());
    assert!(simulation.genealogy().get(2).is_some());
    assert_eq!(children_of(simulation.genealogy(), 1), vec![2, 3]);
    assert_eq!(siblings_of(simulation.genealogy(), 3), vec![2]);
}

#[test]
fn three_generation_lineage_survives_intermediate_death() {
    let mut genealogy = Genealogy::default();
    genealogy.register(1, None, None);
    genealogy.register(2, None, Some(1));
    genealogy.register(3, None, Some(2));

    let parent = genealogy.get(3).and_then(|record| record.father_id);
    let grandparent = parent.and_then(|id| genealogy.get(id)?.father_id);

    assert_eq!(parent, Some(2));
    assert_eq!(grandparent, Some(1));
}

#[test]
fn identical_registration_produces_identical_genealogy() {
    let build = || {
        let mut genealogy = Genealogy::default();
        genealogy.register(1, None, None);
        genealogy.register(2, Some(1), None);
        genealogy
    };

    assert_eq!(build(), build());
}
