use super::super::genealogy::Genealogy;
use super::super::kinship::{children_of, siblings_of};
use super::support::entity;

fn genealogy(mut entities: Vec<super::super::Entity>) -> Genealogy {
    entities.sort_unstable_by_key(|entity| entity.id);
    let mut genealogy = Genealogy::default();
    for entity in entities {
        genealogy.register(entity.id, entity.mother_id, entity.father_id);
    }
    genealogy
}

#[test]
fn mother_and_father_derive_their_biological_child() {
    let mother = entity(1, 0, 0, 0.0);
    let father = entity(2, 0, 0, 0.0);
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);
    child.father_id = Some(2);
    let genealogy = genealogy(vec![mother, father, child]);

    assert_eq!(children_of(&genealogy, 1), vec![3]);
    assert_eq!(children_of(&genealogy, 2), vec![3]);
}

#[test]
fn multiple_children_are_returned_in_entity_id_order() {
    let mut later = entity(9, 0, 0, 0.0);
    later.mother_id = Some(1);
    let mut earlier = entity(4, 0, 0, 0.0);
    earlier.father_id = Some(1);

    assert_eq!(children_of(&genealogy(vec![later, earlier]), 1), vec![4, 9]);
}

#[test]
fn caregiver_is_not_derived_as_a_biological_parent() {
    let caregiver = entity(8, 0, 0, 0.0);
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);
    child.father_id = Some(2);
    child.caregiver_id = Some(8);

    assert!(children_of(&genealogy(vec![caregiver, child]), 8).is_empty());
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

    assert_eq!(
        children_of(&genealogy(vec![mother, father, child.clone()]), 1),
        vec![3]
    );
    assert_eq!(children_of(&genealogy(vec![child]), 2), vec![3]);
}

#[test]
fn founder_can_have_children_without_having_parents() {
    let founder = entity(1, 0, 0, 0.0);
    let mut child = entity(3, 0, 0, 0.0);
    child.mother_id = Some(1);

    assert!(founder.mother_id.is_none() && founder.father_id.is_none());
    assert_eq!(children_of(&genealogy(vec![founder, child]), 1), vec![3]);
}

fn biological_child(
    id: u32,
    mother_id: Option<u32>,
    father_id: Option<u32>,
) -> super::super::Entity {
    let mut child = entity(id, 0, 0, 0.0);
    child.mother_id = mother_id;
    child.father_id = father_id;
    child
}

#[test]
fn full_siblings_are_derived_from_shared_parents() {
    let first = biological_child(10, Some(1), Some(2));
    let second = biological_child(11, Some(1), Some(2));

    assert_eq!(siblings_of(&genealogy(vec![first, second]), 10), vec![11]);
}

#[test]
fn maternal_and_paternal_half_siblings_are_derived() {
    let focal = biological_child(10, Some(1), Some(2));
    let maternal = biological_child(12, Some(1), Some(3));
    let paternal = biological_child(14, Some(4), Some(2));

    assert_eq!(
        siblings_of(&genealogy(vec![paternal, focal, maternal]), 10),
        vec![12, 14]
    );
}

#[test]
fn unrelated_entities_and_unknown_founders_are_not_siblings() {
    let first = biological_child(10, Some(1), Some(2));
    let unrelated = biological_child(11, Some(3), Some(4));
    let founder_a = entity(20, 0, 0, 0.0);
    let founder_b = entity(21, 0, 0, 0.0);
    let genealogy = genealogy(vec![first, unrelated, founder_a, founder_b]);

    assert!(siblings_of(&genealogy, 10).is_empty());
    assert!(siblings_of(&genealogy, 20).is_empty());
}

#[test]
fn entity_is_never_its_own_sibling() {
    let only_child = biological_child(10, Some(1), Some(2));

    assert!(siblings_of(&genealogy(vec![only_child]), 10).is_empty());
}

#[test]
fn caregiver_does_not_create_sibling_kinship() {
    let mut first = biological_child(10, Some(1), Some(2));
    let mut second = biological_child(11, Some(3), Some(4));
    first.caregiver_id = Some(8);
    second.caregiver_id = Some(8);

    assert!(siblings_of(&genealogy(vec![first, second]), 10).is_empty());
}

#[test]
fn siblings_are_returned_in_deterministic_id_order() {
    let focal = biological_child(10, Some(1), Some(2));
    let later = biological_child(19, Some(1), Some(5));
    let earlier = biological_child(12, Some(6), Some(2));

    assert_eq!(
        siblings_of(&genealogy(vec![later, focal, earlier]), 10),
        vec![12, 19]
    );
}
