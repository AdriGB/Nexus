use super::super::kinship::children_of;
use super::support::entity;

#[test]
fn mother_and_father_derive_their_biological_child() {
    let mother = entity(1, 0, 0, 0.0);
    let father = entity(2, 0, 0, 0.0);
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);
    child.father_id = Some(2);
    let entities = vec![mother, father, child];

    assert_eq!(children_of(&entities, 1), vec![3]);
    assert_eq!(children_of(&entities, 2), vec![3]);
}

#[test]
fn multiple_children_are_returned_in_entity_id_order() {
    let mut later = entity(9, 0, 0, 0.0);
    later.mother_id = Some(1);
    let mut earlier = entity(4, 0, 0, 0.0);
    earlier.father_id = Some(1);

    assert_eq!(children_of(&[later, earlier], 1), vec![4, 9]);
}

#[test]
fn caregiver_is_not_derived_as_a_biological_parent() {
    let caregiver = entity(8, 0, 0, 0.0);
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);
    child.father_id = Some(2);
    child.caregiver_id = Some(8);

    assert!(children_of(&[caregiver, child], 8).is_empty());
}

#[test]
fn kinship_survives_partnership_dissolution_and_absent_parents() {
    let mut mother = entity(1, 0, 0, 0.0);
    let mut father = entity(2, 0, 0, 0.0);
    mother.partner_id = None;
    father.partner_id = None;
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);
    child.father_id = Some(2);

    assert_eq!(children_of(&[mother, father, child.clone()], 1), vec![3]);
    assert_eq!(children_of(&[child], 2), vec![3]);
}

#[test]
fn founder_can_have_children_without_having_parents() {
    let founder = entity(1, 0, 0, 0.0);
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);

    assert!(founder.mother_id.is_none() && founder.father_id.is_none());
    assert_eq!(children_of(&[founder, child], 1), vec![3]);
}
