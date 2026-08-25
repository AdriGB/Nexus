use super::super::entity::Entity;
use super::super::spatial::EntitySnapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::simulation) enum CloseRelationshipRole {
    CurrentPartner,
    ParentChild,
    Sibling,
    Other,
}

#[derive(Clone, Copy)]
pub(in crate::simulation) struct RelationshipIdentity {
    pub id: u32,
    pub partner_id: Option<u32>,
    pub mother_id: Option<u32>,
    pub father_id: Option<u32>,
}

impl RelationshipIdentity {
    pub(in crate::simulation) fn from_entity(entity: &Entity) -> Self {
        Self {
            id: entity.id,
            partner_id: entity.partner_id,
            mother_id: entity.mother_id,
            father_id: entity.father_id,
        }
    }
}

pub(in crate::simulation) fn close_relationship_role(
    actor: RelationshipIdentity,
    target: &EntitySnapshot,
) -> CloseRelationshipRole {
    close_relationship_role_between(
        actor,
        RelationshipIdentity {
            id: target.id,
            partner_id: target.partner_id,
            mother_id: target.mother_id,
            father_id: target.father_id,
        },
    )
}

pub(in crate::simulation) fn close_relationship_role_between(
    actor: RelationshipIdentity,
    target: RelationshipIdentity,
) -> CloseRelationshipRole {
    if actor.partner_id == Some(target.id) {
        return CloseRelationshipRole::CurrentPartner;
    }
    if actor.mother_id == Some(target.id)
        || actor.father_id == Some(target.id)
        || target.mother_id == Some(actor.id)
        || target.father_id == Some(actor.id)
    {
        return CloseRelationshipRole::ParentChild;
    }
    if actor.mother_id.is_some() && actor.mother_id == target.mother_id
        || actor.father_id.is_some() && actor.father_id == target.father_id
    {
        return CloseRelationshipRole::Sibling;
    }
    CloseRelationshipRole::Other
}
