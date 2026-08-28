use super::super::genealogy::Genealogy;
use super::super::kinship::{
    ancestors_of, children_of, descendants_of, family_tree_of, relationship_between, siblings_of,
    FamilyTreeEdge, FamilyTreeNode, KinshipGeneration, KinshipRelation,
};
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

#[test]
fn ancestors_preserve_parent_grandparent_and_great_grandparent_distance() {
    let great_grandparent = biological_child(1, None, None);
    let grandparent = biological_child(2, None, Some(1));
    let parent = biological_child(3, None, Some(2));
    let focal = biological_child(4, None, Some(3));
    let genealogy = genealogy(vec![focal, parent, great_grandparent, grandparent]);

    assert_eq!(
        ancestors_of(&genealogy, 4),
        vec![
            KinshipGeneration {
                entity_id: 3,
                generation: 1,
            },
            KinshipGeneration {
                entity_id: 2,
                generation: 2,
            },
            KinshipGeneration {
                entity_id: 1,
                generation: 3,
            },
        ]
    );
}

#[test]
fn descendants_preserve_child_grandchild_and_great_grandchild_distance() {
    let root = biological_child(1, None, None);
    let child = biological_child(2, None, Some(1));
    let grandchild = biological_child(3, None, Some(2));
    let great_grandchild = biological_child(4, None, Some(3));
    let genealogy = genealogy(vec![great_grandchild, grandchild, child, root]);

    assert_eq!(
        descendants_of(&genealogy, 1),
        vec![
            KinshipGeneration {
                entity_id: 2,
                generation: 1,
            },
            KinshipGeneration {
                entity_id: 3,
                generation: 2,
            },
            KinshipGeneration {
                entity_id: 4,
                generation: 3,
            },
        ]
    );
}

#[test]
fn founders_and_childless_entities_have_no_extended_kinship() {
    let founder = biological_child(1, None, None);
    let genealogy = genealogy(vec![founder]);

    assert!(ancestors_of(&genealogy, 1).is_empty());
    assert!(descendants_of(&genealogy, 1).is_empty());
}

#[test]
fn converging_ancestor_paths_are_deduplicated_at_minimum_distance() {
    let root = biological_child(1, None, None);
    let left = biological_child(2, Some(1), None);
    let right = biological_child(3, None, Some(1));
    let focal = biological_child(4, Some(2), Some(3));
    let genealogy = genealogy(vec![focal, right, root, left]);

    let ancestors = ancestors_of(&genealogy, 4);
    assert_eq!(
        ancestors
            .iter()
            .filter(|relative| relative.entity_id == 1)
            .copied()
            .collect::<Vec<_>>(),
        vec![KinshipGeneration {
            entity_id: 1,
            generation: 2,
        }]
    );
}

#[test]
fn incomplete_parentage_keeps_known_ancestors_and_stops_cleanly() {
    let focal = biological_child(2, None, Some(99));
    let genealogy = genealogy(vec![focal]);

    assert_eq!(
        ancestors_of(&genealogy, 2),
        vec![KinshipGeneration {
            entity_id: 99,
            generation: 1,
        }]
    );
}

#[test]
fn malformed_cycle_terminates_without_returning_the_focal_entity() {
    let first = biological_child(1, None, Some(2));
    let second = biological_child(2, None, Some(1));
    let genealogy = genealogy(vec![first, second]);

    assert_eq!(
        ancestors_of(&genealogy, 1),
        vec![KinshipGeneration {
            entity_id: 2,
            generation: 1,
        }]
    );
    assert_eq!(
        descendants_of(&genealogy, 1),
        vec![KinshipGeneration {
            entity_id: 2,
            generation: 1,
        }]
    );
}

#[test]
fn direct_and_multigeneration_relations_are_directional() {
    let grandparent = biological_child(1, None, None);
    let parent = biological_child(2, Some(1), None);
    let child = biological_child(3, Some(2), None);
    let great_grandchild = biological_child(4, Some(3), None);
    let genealogy = genealogy(vec![grandparent, parent, child, great_grandchild]);

    assert_eq!(
        relationship_between(&genealogy, 2, 3),
        KinshipRelation::Parent
    );
    assert_eq!(
        relationship_between(&genealogy, 3, 2),
        KinshipRelation::Child
    );
    assert_eq!(
        relationship_between(&genealogy, 1, 3),
        KinshipRelation::Ancestor { generations: 2 }
    );
    assert_eq!(
        relationship_between(&genealogy, 4, 1),
        KinshipRelation::Descendant { generations: 3 }
    );
}

#[test]
fn full_and_half_siblings_are_distinguished() {
    let first = biological_child(10, Some(1), Some(2));
    let full = biological_child(11, Some(1), Some(2));
    let half = biological_child(12, Some(1), Some(3));
    let genealogy = genealogy(vec![first, full, half]);

    assert_eq!(
        relationship_between(&genealogy, 10, 11),
        KinshipRelation::FullSibling
    );
    assert_eq!(
        relationship_between(&genealogy, 10, 12),
        KinshipRelation::HalfSibling
    );
}

#[test]
fn aunt_uncle_and_niece_nephew_are_directional() {
    let root = biological_child(1, None, None);
    let aunt = biological_child(2, Some(1), None);
    let parent = biological_child(3, Some(1), None);
    let child = biological_child(4, Some(3), None);
    let grandchild = biological_child(5, Some(4), None);
    let genealogy = genealogy(vec![root, aunt, parent, child, grandchild]);

    assert_eq!(
        relationship_between(&genealogy, 2, 4),
        KinshipRelation::AuntUncle {
            generations_removed: 0
        }
    );
    assert_eq!(
        relationship_between(&genealogy, 4, 2),
        KinshipRelation::NieceNephew {
            generations_removed: 0
        }
    );
    assert_eq!(
        relationship_between(&genealogy, 2, 5),
        KinshipRelation::AuntUncle {
            generations_removed: 1
        }
    );
}

#[test]
fn cousin_degree_and_removal_are_derived() {
    let root = biological_child(1, None, None);
    let left = biological_child(2, Some(1), None);
    let right = biological_child(3, Some(1), None);
    let first_cousin = biological_child(5, Some(3), None);
    let left_child = biological_child(4, Some(2), None);
    let left_grandchild = biological_child(6, Some(4), None);
    let right_grandchild = biological_child(7, Some(5), None);
    let right_great_grandchild = biological_child(8, Some(7), None);
    let genealogy = genealogy(vec![
        root,
        left,
        right,
        left_child,
        first_cousin,
        left_grandchild,
        right_grandchild,
        right_great_grandchild,
    ]);

    assert_eq!(
        relationship_between(&genealogy, 4, 5),
        KinshipRelation::Cousin {
            degree: 1,
            removed: 0
        }
    );
    assert_eq!(
        relationship_between(&genealogy, 6, 7),
        KinshipRelation::Cousin {
            degree: 2,
            removed: 0
        }
    );
    assert_eq!(
        relationship_between(&genealogy, 6, 8),
        KinshipRelation::Cousin {
            degree: 2,
            removed: 1
        }
    );
}

#[test]
fn same_unrelated_and_incomplete_entities_are_safe() {
    let founder = biological_child(1, None, None);
    let unrelated = biological_child(2, None, None);
    let incomplete = biological_child(3, Some(99), None);
    let genealogy = genealogy(vec![founder, unrelated, incomplete]);

    assert_eq!(
        relationship_between(&genealogy, 1, 1),
        KinshipRelation::SamePerson
    );
    assert_eq!(
        relationship_between(&genealogy, 1, 2),
        KinshipRelation::Unrelated
    );
    assert_eq!(
        relationship_between(&genealogy, 1, 3),
        KinshipRelation::Unrelated
    );
    assert_eq!(
        relationship_between(&genealogy, 99, 3),
        KinshipRelation::Parent
    );
}

#[test]
fn malformed_cycles_terminate_during_classification() {
    let first = biological_child(1, Some(2), None);
    let second = biological_child(2, Some(1), None);
    let unrelated = biological_child(3, None, None);
    let genealogy = genealogy(vec![first, second, unrelated]);

    assert_eq!(
        relationship_between(&genealogy, 1, 3),
        KinshipRelation::Unrelated
    );
}

#[test]
fn multiple_common_ancestors_use_distance_then_id_tiebreakers() {
    let first_common = biological_child(1, None, None);
    let second_common = biological_child(2, None, None);
    let first_parent = biological_child(10, Some(1), None);
    let first = biological_child(100, Some(10), Some(2));
    let second_path_a_2 = biological_child(31, Some(1), None);
    let second_path_a_1 = biological_child(30, Some(31), None);
    let second_path_b_3 = biological_child(42, Some(2), None);
    let second_path_b_2 = biological_child(41, Some(42), None);
    let second_path_b_1 = biological_child(40, Some(41), None);
    let second = biological_child(200, Some(30), Some(40));
    let genealogy = genealogy(vec![
        first_common,
        second_common,
        first_parent,
        first,
        second_path_a_2,
        second_path_a_1,
        second_path_b_3,
        second_path_b_2,
        second_path_b_1,
        second,
    ]);

    // Both common ancestors have total distance five. Ancestor #1 wins because
    // its maximum branch distance is three instead of four.
    assert_eq!(
        relationship_between(&genealogy, 100, 200),
        KinshipRelation::Cousin {
            degree: 1,
            removed: 1,
        }
    );
}

#[test]
fn family_tree_contains_focal_parents_and_children_with_liveness() {
    let dead_parent = biological_child(1, None, None);
    let focal = biological_child(2, Some(1), None);
    let child = biological_child(3, Some(2), None);
    let genealogy = genealogy(vec![dead_parent, focal.clone(), child.clone()]);

    let tree = family_tree_of(&genealogy, &[focal, child], 2, 2, 2);

    assert_eq!(tree.focal_id, 2);
    assert_eq!(
        tree.nodes,
        vec![
            FamilyTreeNode {
                entity_id: 1,
                generation: -1,
                alive: false
            },
            FamilyTreeNode {
                entity_id: 2,
                generation: 0,
                alive: true
            },
            FamilyTreeNode {
                entity_id: 3,
                generation: 1,
                alive: true
            },
        ]
    );
    assert_eq!(
        tree.edges,
        vec![
            FamilyTreeEdge {
                parent_id: 1,
                child_id: 2
            },
            FamilyTreeEdge {
                parent_id: 2,
                child_id: 3
            },
        ]
    );
}

#[test]
fn family_tree_respects_bounded_generation_depth() {
    let first = biological_child(1, None, None);
    let second = biological_child(2, Some(1), None);
    let third = biological_child(3, Some(2), None);
    let focal = biological_child(4, Some(3), None);
    let child = biological_child(5, Some(4), None);
    let grandchild = biological_child(6, Some(5), None);
    let great_grandchild = biological_child(7, Some(6), None);
    let genealogy = genealogy(vec![
        first,
        second,
        third,
        focal,
        child,
        grandchild,
        great_grandchild,
    ]);

    let tree = family_tree_of(&genealogy, &[], 4, 2, 2);
    assert_eq!(
        tree.nodes
            .iter()
            .map(|node| (node.entity_id, node.generation))
            .collect::<Vec<_>>(),
        vec![(2, -2), (3, -1), (4, 0), (5, 1), (6, 2)]
    );
}

#[test]
fn family_tree_deduplicates_converging_branches_and_orders_output() {
    let root = biological_child(1, None, None);
    let left = biological_child(2, Some(1), None);
    let right = biological_child(3, Some(1), None);
    let focal = biological_child(4, Some(2), Some(3));
    let genealogy = genealogy(vec![focal, right, root, left]);

    let tree = family_tree_of(&genealogy, &[], 4, 2, 0);
    assert_eq!(
        tree.nodes.iter().filter(|node| node.entity_id == 1).count(),
        1
    );
    assert_eq!(
        tree.nodes
            .iter()
            .map(|node| node.entity_id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    assert_eq!(
        tree.edges,
        vec![
            FamilyTreeEdge {
                parent_id: 1,
                child_id: 2
            },
            FamilyTreeEdge {
                parent_id: 1,
                child_id: 3
            },
            FamilyTreeEdge {
                parent_id: 2,
                child_id: 4
            },
            FamilyTreeEdge {
                parent_id: 3,
                child_id: 4
            },
        ]
    );
}

#[test]
fn unknown_and_malformed_family_trees_are_safe() {
    let first = biological_child(1, Some(2), None);
    let second = biological_child(2, Some(1), None);
    let genealogy = genealogy(vec![first, second]);

    let unknown = family_tree_of(&genealogy, &[], 99, 2, 2);
    assert!(unknown.nodes.is_empty() && unknown.edges.is_empty());

    let cyclic = family_tree_of(&genealogy, &[], 1, 10, 10);
    assert_eq!(cyclic.nodes.len(), 2);
    assert_eq!(
        cyclic
            .nodes
            .iter()
            .filter(|node| node.entity_id == 1)
            .count(),
        1
    );
}

#[test]
fn children_index_matches_a_full_record_scan() {
    let genealogy = genealogy(vec![
        biological_child(1, None, None),
        biological_child(2, None, None),
        biological_child(3, Some(1), Some(2)),
        biological_child(4, Some(1), Some(2)),
        biological_child(5, Some(1), None),
        biological_child(6, None, Some(2)),
        biological_child(7, Some(3), Some(6)),
        biological_child(8, None, None),
    ]);

    for candidate in 0..=10u32 {
        let scanned: Vec<u32> = genealogy
            .records()
            .iter()
            .filter(|record| {
                record.mother_id == Some(candidate) || record.father_id == Some(candidate)
            })
            .map(|record| record.entity_id)
            .collect();
        assert_eq!(
            children_of(&genealogy, candidate),
            scanned,
            "index disagrees with a record scan for parent {candidate}"
        );
    }
}

#[test]
fn mother_and_father_collapse_to_one_parent_when_they_are_the_same_entity() {
    let genealogy = genealogy(vec![
        biological_child(1, None, None),
        biological_child(2, Some(1), Some(1)),
    ]);

    assert_eq!(children_of(&genealogy, 1), vec![2]);
}

#[test]
fn childless_entities_resolve_to_no_children() {
    let genealogy = genealogy(vec![
        biological_child(1, None, None),
        biological_child(2, Some(1), None),
    ]);

    assert!(children_of(&genealogy, 2).is_empty());
    assert!(children_of(&genealogy, 99).is_empty());
}

#[test]
fn descendant_traversal_walks_the_index_across_generations() {
    let genealogy = genealogy(vec![
        biological_child(1, None, None),
        biological_child(2, None, None),
        biological_child(3, Some(1), Some(2)),
        biological_child(4, Some(3), Some(2)),
    ]);

    assert_eq!(
        descendants_of(&genealogy, 1),
        vec![
            KinshipGeneration {
                entity_id: 3,
                generation: 1,
            },
            KinshipGeneration {
                entity_id: 4,
                generation: 2,
            },
        ]
    );
}
